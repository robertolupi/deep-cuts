#!/usr/bin/env python3
"""Phase 1c Evaluation: Supervised Peak Classifier.

Generates candidate boundary points from baseline, refined, and SSM novelty peaks,
extracts a rich 11-dimensional feature vector for each candidate (including harmonic 
chroma differences, prominence, distance to baseline, etc.), trains a Random Forest 
classifier on the Inner Dev fold (80%), tunes prediction threshold and spacing constraints,
and evaluates generalization on the Held-back fold (20%).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import warnings
from pathlib import Path
from typing import Any, Literal

import numpy as np
from scipy.signal import find_peaks
from sklearn.ensemble import RandomForestClassifier

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
sys.path.insert(0, str(SCRIPT_DIR))

# Import Helpers
from evaluate_salami_phase0 import (
    DEFAULT_DB_PATH,
    DEFAULT_VALIDATION_SPLIT,
    Tolerances,
    _aggregate_tracks,
    _aligned_human_subset,
    _parse_split_entry,
    _prepare_pass_for_mode,
    _score_boundaries,
    _score_human,
    _score_variant_for_pass,
    _mean_score,
    _shift_predictions_for_mode,
    _significance,
    _reject_holdout,
    bootstrap_ci,
    calculate_crop_offset,
    load_track,
    to_absolute_time,
)
from evaluate_salami_boundaries import (
    JAMS_DIR,
    evaluate_pairwise_clustering,
    load_onset_map,
    parse_jams_boundaries_and_labels,
    project_jams_to_16_bins,
)
from refine_salami_boundaries import snap
from evaluate_salami_phase1a import (
    load_high_res_features,
    paired_wilcoxon_phase1a,
)
from evaluate_salami_phase1b import (
    make_checkerboard_kernel,
    compute_ssm_novelty,
    _aggregate_tracks_phase1b,
    print_report,
)

warnings.filterwarnings("ignore", category=UserWarning)

# Fixed SSM novelty calculation parameters (optimized from Phase 1b HPO)
SSM_KERNEL_SIZE = 60
SSM_KERNEL_SIGMA = 1.0753
SSM_MIN_PROMINENCE = 0.2063
SSM_MIN_DISTANCE_SEC = 2.62
SSM_SNAP_WINDOW = 0.5


def compute_local_chroma_diff(chroma_series: np.ndarray, chroma_times: np.ndarray, t: float, W: float) -> float:
    """Compute cosine distance between mean chroma vectors in left and right windows of size W."""
    left_mask = (chroma_times >= t - W) & (chroma_times <= t)
    right_mask = (chroma_times >= t) & (chroma_times <= t + W)
    
    if not np.any(left_mask) or not np.any(right_mask):
        return 0.0
        
    left_mean = np.mean(chroma_series[left_mask], axis=0)
    right_mean = np.mean(chroma_series[right_mask], axis=0)
    
    norm_l = np.linalg.norm(left_mean)
    norm_r = np.linalg.norm(right_mean)
    if norm_l < 1e-8 or norm_r < 1e-8:
        return 0.0
        
    cosine_sim = np.dot(left_mean, right_mean) / (norm_l * norm_r)
    return float(1.0 - np.clip(cosine_sim, -1.0, 1.0))


def compute_onset_density(onset_times: np.ndarray, t: float, W: float) -> float:
    if onset_times.size == 0:
        return 0.0
    return float(np.sum((onset_times >= t - W) & (onset_times <= t + W)))


def compute_onset_strength_sum(onset_times: np.ndarray, onset_strengths: np.ndarray, t: float, W: float) -> float:
    if onset_times.size == 0 or onset_strengths.size == 0:
        return 0.0
    mask = (onset_times >= t - W) & (onset_times <= t + W)
    return float(np.sum(onset_strengths[mask]))


def compute_local_chroma_var_diff(chroma_series: np.ndarray, chroma_times: np.ndarray, t: float, W: float) -> float:
    left_mask = (chroma_times >= t - W) & (chroma_times <= t)
    right_mask = (chroma_times >= t) & (chroma_times <= t + W)
    if not np.any(left_mask) or not np.any(right_mask):
        return 0.0
    left_var = np.var(chroma_series[left_mask], axis=0)
    right_var = np.var(chroma_series[right_mask], axis=0)
    return float(np.linalg.norm(left_var - right_var))


def compute_novelty_contrast(novelty: np.ndarray, chroma_times: np.ndarray, t: float, W: float) -> float:
    mask = (chroma_times >= t - W) & (chroma_times <= t + W)
    if not np.any(mask):
        return 0.0
    local_mean = np.mean(novelty[mask])
    val = np.interp(t, chroma_times, novelty)
    return float(val - local_mean)


def extract_track_candidates_and_features(track: dict, mode: str, window: float) -> tuple[list[float], np.ndarray, np.ndarray]:
    """Generates candidate boundary times and extracts their 11-dimensional feature vectors and binary targets."""
    duration = float(track["duration"])
    offset = calculate_crop_offset(duration, window)
    
    # 1. Generate SSM Novelty
    kernel = make_checkerboard_kernel(SSM_KERNEL_SIZE, SSM_KERNEL_SIGMA)
    novelty = compute_ssm_novelty(track, kernel)
    
    # Peak picking
    min_dist_frames = max(1, int(round(SSM_MIN_DISTANCE_SEC / 0.2)))
    novelty_std = np.std(novelty)
    prom = SSM_MIN_PROMINENCE * (novelty_std if novelty_std > 1e-8 else 1.0)
    peaks, properties = find_peaks(novelty, prominence=prom, distance=min_dist_frames)
    
    chroma_times = np.array(track["chroma_times"])
    peaks_abs = []
    if len(peaks) > 0:
        peaks_abs = to_absolute_time([chroma_times[p] for p in peaks], offset)
        
    # Snap peaks to onsets
    onset_times = np.array(track["onsets"].get("times", []))
    onset_peaks_abs = to_absolute_time(onset_times, offset)
    if SSM_SNAP_WINDOW > 0.0 and len(onset_peaks_abs) > 0 and len(peaks_abs) > 0:
        peaks_abs = snap(peaks_abs, onset_peaks_abs, window=SSM_SNAP_WINDOW)
        
    # Gather candidates from baseline, refined, and SSM peaks
    raw_candidates = []
    raw_candidates.extend(track["baseline_boundaries"])
    raw_candidates.extend(track["refined_boundaries"])
    raw_candidates.extend(peaks_abs)
    
    # Deduplicate candidates within 0.5 seconds of each other
    raw_candidates = sorted(list(set(raw_candidates)))
    candidates = []
    for c in raw_candidates:
        if not candidates or (c - candidates[-1]) >= 0.5:
            candidates.append(c)
            
    if not candidates:
        return [], np.empty((0, 17)), np.empty((0,))
        
    # Extract features for each candidate
    features_list = []
    targets_list = []
    chroma_series_np = np.array(track["chroma_series"])
    
    # Gather all ground-truth boundaries from annotator passes
    gt_boundaries = []
    for pass_info in track["passes"]:
        prepared, _, _ = _prepare_pass_for_mode(pass_info, mode, duration, window)
        if prepared["segments"]:
            gt_boundaries.extend(prepared["boundaries"])
    gt_boundaries = np.array(sorted(list(set(gt_boundaries))))
    
    for c in candidates:
        c_crop = c - offset
        
        # 1. ssm_novelty value (interpolated)
        ssm_novelty_val = 0.0
        if chroma_times.size > 0:
            ssm_novelty_val = float(np.interp(c_crop, chroma_times, novelty))
            
        # 2. is_ssm_peak
        is_ssm_peak = 1.0 if (len(peaks_abs) > 0 and any(abs(c - p) < 0.25 for p in peaks_abs)) else 0.0
        
        # 3. ssm_prominence
        ssm_prom = 0.0
        if len(peaks_abs) > 0:
            closest_p_idx = np.argmin(np.abs(np.array(peaks_abs) - c))
            if abs(c - peaks_abs[closest_p_idx]) < 0.25:
                ssm_prom = float(properties["prominences"][closest_p_idx])
                
        # 4. is_baseline
        is_baseline = 1.0 if any(abs(c - b) < 0.1 for b in track["baseline_boundaries"]) else 0.0
        
        # 5. is_refined
        is_refined = 1.0 if any(abs(c - r) < 0.1 for r in track["refined_boundaries"]) else 0.0
        
        # 6. dist_to_baseline
        dist_to_baseline = min([abs(c - b) for b in track["baseline_boundaries"]]) if track["baseline_boundaries"] else 99.0
        
        # 7. dist_to_refined
        dist_to_refined = min([abs(c - r) for r in track["refined_boundaries"]]) if track["refined_boundaries"] else 99.0
        
        # 8. chroma_diff_5s
        chroma_diff_5s = compute_local_chroma_diff(chroma_series_np, chroma_times, c_crop, 5.0)
        
        # 9. chroma_diff_10s
        chroma_diff_10s = compute_local_chroma_diff(chroma_series_np, chroma_times, c_crop, 10.0)
        
        # 10. onset_strength
        onset_strengths = np.array(track["onsets"].get("strengths", []))
        onset_str = 0.0
        if onset_times.size > 0 and onset_strengths.size > 0:
            closest_o_idx = np.argmin(np.abs(onset_times - c_crop))
            if abs(onset_times[closest_o_idx] - c_crop) < 0.25:
                onset_str = float(onset_strengths[closest_o_idx])
                
        # 11. time_ratio
        time_ratio = c / duration
        
        # 12. onset_density_2s
        onset_density_2s = compute_onset_density(onset_times, c_crop, 1.0)
        
        # 13. onset_density_5s
        onset_density_5s = compute_onset_density(onset_times, c_crop, 2.5)
        
        # 14. onset_strength_sum_2s
        onset_str_sum_2s = compute_onset_strength_sum(onset_times, onset_strengths, c_crop, 1.0)
        
        # 15. chroma_var_diff_5s
        chroma_var_diff_5s = compute_local_chroma_var_diff(chroma_series_np, chroma_times, c_crop, 5.0)
        
        # 16. chroma_var_diff_10s
        chroma_var_diff_10s = compute_local_chroma_var_diff(chroma_series_np, chroma_times, c_crop, 10.0)
        
        # 17. novelty_contrast_5s
        novelty_contrast_5s = compute_novelty_contrast(novelty, chroma_times, c_crop, 5.0)
        
        feats = np.array([
            ssm_novelty_val, is_ssm_peak, ssm_prom,
            is_baseline, is_refined, dist_to_baseline, dist_to_refined,
            chroma_diff_5s, chroma_diff_10s, onset_str, time_ratio,
            onset_density_2s, onset_density_5s, onset_str_sum_2s,
            chroma_var_diff_5s, chroma_var_diff_10s, novelty_contrast_5s
        ])
        features_list.append(feats)
        
        # Target: 1.0 if within 2.0s of ANY ground-truth boundary, else 0.0
        is_gt = 0.0
        if gt_boundaries.size > 0:
            min_gt_dist = np.min(np.abs(gt_boundaries - c))
            if min_gt_dist <= 2.0:
                is_gt = 1.0
        targets_list.append(is_gt)
        
    return candidates, np.array(features_list), np.array(targets_list)


def predict_boundaries(
    track: dict,
    candidates: list[float],
    features: np.ndarray,
    clf: RandomForestClassifier,
    threshold: float,
    min_gap: float,
    max_boundaries: int,
    mode: str,
    window: float,
) -> list[float]:
    """Applies the trained classifier on candidate boundaries and decodes the final set."""
    if not candidates or features.size == 0:
        return list(track["baseline_boundaries"])
        
    # Get predicted probabilities for boundary class (class 1)
    probs = clf.predict_proba(features)[:, 1]
    
    # Sort candidates by probability descending
    ranked_candidates = sorted(zip(candidates, probs), key=lambda x: -x[1])
    
    # Greedy decoding with min_gap constraint
    selected = []
    for cand, prob in ranked_candidates:
        if prob < threshold:
            continue
        # Check gap with already selected boundaries
        if all(abs(cand - s) >= min_gap for s in selected):
            selected.append(cand)
        if len(selected) >= max_boundaries:
            break
            
    # Always sort selected boundaries chronologically
    pred_full = sorted(selected)
    
    # Fallback to refined boundaries if we predict nothing
    if not pred_full:
        pred_full = list(track["refined_boundaries"])
        
    duration = float(track["duration"])
    return _shift_predictions_for_mode(pred_full, mode, duration, window)


def run_ml_evaluation_split(
    tracks: list[dict],
    clf: RandomForestClassifier,
    threshold: float,
    min_gap: float,
    max_boundaries: int,
    mode: str,
    window: float,
    n_bootstrap: int,
) -> dict[str, Any]:
    """Runs final evaluation and computes statistics for a given track subset using the ML model."""
    evaluated = []
    for track in tracks:
        duration = float(track["duration"])
        offset = calculate_crop_offset(duration, window)
        
        prepared_passes = []
        eval_duration = duration
        for pass_info in track["passes"]:
            prepared, eval_duration, offset = _prepare_pass_for_mode(pass_info, mode, duration, window)
            if prepared["segments"]:
                prepared_passes.append(prepared)
                
        # Generate predictions using ML model
        cands = track["candidates"]
        feats = track["features"]
        
        pred = predict_boundaries(
            track, cands, feats, clf, threshold, min_gap, max_boundaries, mode, window
        )
        
        # Standard legacy reference bounds
        base = _shift_predictions_for_mode(track["baseline_boundaries"], mode, duration, window)
        refined = _shift_predictions_for_mode(track["refined_boundaries"], mode, duration, window)
        
        cand = {
            "baseline": base,
            "refined": refined,
            "ssm_fused": pred
        }
        
        per_pass = {k: [] for k in cand.keys()}
        pairwise = []
        for prepared in prepared_passes:
            _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
            cand["oracle"] = oracle_bounds

            for variant in per_pass.keys():
                per_pass[variant].append(_score_variant_for_pass(prepared["boundaries"], cand[variant], eval_duration, (0.5, 3.0)))
                
            _, _, pw_f1 = evaluate_pairwise_clustering(track["labels"], prepared["segments"], eval_duration)
            pairwise.append(float(pw_f1))
            
        summary = {
            "db_id": track["db_id"],
            "salami_id": track["salami_id"],
            "duration": duration,
            "mode_duration": eval_duration,
            "crop_offset": offset,
            "n_annotators": len(prepared_passes),
            "boundary_counts": {
                k: len(v) for k, v in cand.items()
            },
            "pairwise_label_f1": float(np.mean(pairwise)) if pairwise else 0.0,
            "scores": {},
        }
        summary["boundary_counts"]["gt_mean"] = float(np.mean([len(p["boundaries"]) for p in prepared_passes]))
        
        for variant, variant_scores in per_pass.items():
            summary["scores"][variant] = {str(t): _mean_score(variant_scores, t) for t in (0.5, 3.0)}
            
        # Oracle
        oracle_scores = []
        for prepared in prepared_passes:
            _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
            oracle_scores.append(_score_variant_for_pass(prepared["boundaries"], oracle_bounds, eval_duration, (0.5, 3.0)))
        summary["scores"]["oracle"] = {str(t): _mean_score(oracle_scores, t) for t in (0.5, 3.0)}

        human = _score_human(prepared_passes, eval_duration, (0.5, 3.0))
        if human is not None:
            summary["scores"]["human"] = human
            
        evaluated.append(summary)

    aggregates = _aggregate_tracks_phase1b(evaluated, (0.5, 3.0), n_bootstrap)
    aligned = _aligned_human_subset(evaluated)
    
    # Compute significance
    sig_out = {}
    for tol in (0.5, 3.0):
        key = str(tol)
        refined_scores = np.array([t["scores"]["refined"][key]["f1"] for t in evaluated])
        ssm_scores = np.array([t["scores"]["ssm_fused"][key]["f1"] for t in evaluated])
        oracle_scores = np.array([t["scores"]["oracle"][key]["f1"] for t in evaluated])
        sig_out[key] = {
            "ssm_vs_refined": paired_wilcoxon_phase1a(ssm_scores, refined_scores),
            "ssm_vs_oracle": paired_wilcoxon_phase1a(ssm_scores, oracle_scores)
        }
        
    return {
        "n_evaluated": len(evaluated),
        "per_track": evaluated,
        "aggregates": aggregates,
        "significance": sig_out,
        "aligned_human_subset": {
            "n": len(aligned),
            "aggregates": _aggregate_tracks_phase1b(aligned, (0.5, 3.0), n_bootstrap) if aligned else {},
            "significance": sig_out,
        }
    }


def evaluate_ml_config(
    tracks: list[dict],
    clf: RandomForestClassifier,
    threshold: float,
    min_gap: float,
    max_boundaries: int,
    mode: str,
    window: float,
    objective_type: str,
) -> float:
    """Return mean F1 objective score across tracks for a given decoder configuration."""
    scores_list = []
    for track in tracks:
        try:
            pred = predict_boundaries(
                track, track["candidates"], track["features"], clf,
                threshold, min_gap, max_boundaries, mode, window
            )
            
            duration = float(track["duration"])
            eval_duration = duration
            track_f1s_05 = []
            track_f1s_30 = []
            for pass_info in track["passes"]:
                prepared, eval_duration, _ = _prepare_pass_for_mode(pass_info, mode, duration, window)
                if prepared["segments"]:
                    s05 = _score_boundaries(prepared["boundaries"], pred, eval_duration, 0.5)
                    track_f1s_05.append(s05["f1"])
                    s30 = _score_boundaries(prepared["boundaries"], pred, eval_duration, 3.0)
                    track_f1s_30.append(s30["f1"])
            
            if track_f1s_05 and track_f1s_30:
                mean_f1_05 = np.mean(track_f1s_05)
                mean_f1_30 = np.mean(track_f1s_30)
                if objective_type == "joint":
                    scores_list.append(0.5 * mean_f1_05 + 0.5 * mean_f1_30)
                elif objective_type == "0.5":
                    scores_list.append(mean_f1_05)
                else:
                    scores_list.append(mean_f1_30)
        except Exception:
            continue
    return float(np.mean(scores_list)) if scores_list else 0.0


def tune_ml_decoder(
    tracks: list[dict],
    clf: RandomForestClassifier,
    mode: str,
    window: float,
    objective_type: str,
) -> tuple[dict[str, Any], int]:
    """Performs a grid search over prediction threshold, min gap, and max boundary count."""
    print(f"\nTuning ML Decoder hyperparameters on Inner Dev Fold using objective={objective_type}...")
    best_score = 0.0
    best_params = {}
    
    thresholds = [0.2, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6]
    min_gaps = [3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    max_bnds = [6, 7, 8, 9, 10, 11, 12]
    
    n_configs = 0
    for threshold in thresholds:
        for min_gap in min_gaps:
            for max_bnd in max_bnds:
                score = evaluate_ml_config(
                    tracks, clf, threshold, min_gap, max_bnd, mode, window, objective_type
                )
                n_configs += 1
                if score > best_score:
                    best_score = score
                    best_params = {
                        "threshold": threshold,
                        "min_gap": min_gap,
                        "max_boundaries": max_bnd
                    }
                    
    print(f"Decoder Tuning Best Score: {best_score*100:.2f}%")
    print("Best Decoder Params:", best_params)
    return best_params, n_configs


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 1c Supervised Peak Classifier evaluation.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="windowed")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=200)
    parser.add_argument("--objective", choices=["joint", "0.5", "3.0"], default="joint", help="HPO objective metric")
    parser.add_argument("--allow-holdout", action="store_true", help="custodian-only: allow holdout split")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    # Reject holdout split check
    _reject_holdout(args.split_json, args.allow_holdout)

    with open(args.split_json, "r", encoding="utf-8") as f:
        split_entries = json.load(f)

    # 1. Load all high-resolution track features in memory
    print("Loading track features and annotations...")
    tracks = []
    skipped = []
    onset_map = load_onset_map()
    
    for entry in split_entries:
        try:
            db_id = entry["db_id"]
            salami_id = entry["salami_id"]
            path = entry.get("path")
            
            track = load_track(str(db_id), args.db_path)
            track["salami_id"] = salami_id
            track["path"] = path
            
            jams_path = JAMS_DIR / f"SALAMI_{salami_id}.jams"
            offset = onset_map.get(salami_id, 0.0)
            passes = parse_jams_boundaries_and_labels(jams_path, offset, track["duration"])
            if not passes:
                skipped.append({"db_id": db_id, "salami_id": salami_id, "reason": "no JAMS"})
                continue
            track["passes"] = passes
            track = load_high_res_features(track, args.db_path)
            tracks.append(track)
        except Exception as exc:
            skipped.append({"db_id": entry.get("db_id"), "salami_id": entry.get("salami_id"), "reason": str(exc)})

    print(f"Loaded {len(tracks)} tracks. Skipped {len(skipped)} tracks.")

    if not tracks:
        print("error: no tracks could be evaluated", file=sys.stderr)
        return 2

    # Deterministic split: seed random number generator and shuffle tracks
    # to form the inner dev fold (80%) and held-back eval fold (20%)
    rng = np.random.default_rng(42)
    shuffled_tracks = list(tracks)
    rng.shuffle(shuffled_tracks)
    split_idx = int(len(shuffled_tracks) * 0.8)
    dev_tracks = shuffled_tracks[:split_idx]
    heldback_tracks = shuffled_tracks[split_idx:]
    
    print(f"\nNested validation split created:")
    print(f"  Inner Dev Fold: {len(dev_tracks)} tracks (for classifier training and decoding tuning)")
    print(f"  Held-back Fold: {len(heldback_tracks)} tracks (unbiased validation generalization)")

    # 2. Candidate Generation & Feature Extraction
    print("\nExtracting candidate boundaries and feature vectors...")
    
    X_train_list = []
    y_train_list = []
    
    # We extract features for all tracks in memory and cache them on the track dict
    for track in tracks:
        cands, feats, targets = extract_track_candidates_and_features(track, args.mode, args.window)
        track["candidates"] = cands
        track["features"] = feats
        track["targets"] = targets
        
        # Only add to training set if in dev_tracks
        if any(t["db_id"] == track["db_id"] for t in dev_tracks) and feats.size > 0:
            X_train_list.append(feats)
            y_train_list.append(targets)
            
    X_train = np.vstack(X_train_list)
    y_train = np.concatenate(y_train_list)
    
    print(f"Training dataset size: {X_train.shape[0]} candidates, {int(np.sum(y_train))} positive cases.")

    # 3. Train Classifier
    print("\nTraining Random Forest Classifier on Inner Dev Fold...")
    clf = RandomForestClassifier(n_estimators=100, max_depth=6, min_samples_leaf=4, random_state=42)
    clf.fit(X_train, y_train)
    
    # Feature Importance Report
    feature_names = [
        "ssm_novelty", "is_ssm_peak", "ssm_prominence",
        "is_baseline", "is_refined", "dist_to_baseline", "dist_to_refined",
        "chroma_diff_5s", "chroma_diff_10s", "onset_strength", "time_ratio"
    ]
    importances = clf.feature_importances_
    print("Feature Importances:")
    for name, imp in sorted(zip(feature_names, importances), key=lambda x: -x[1]):
        print(f"  {name:<18}: {imp*100:.2f}%")

    # 4. Tune Decoder Parameters on Inner Dev Fold
    best_decoder_params, n_configs_evaluated = tune_ml_decoder(
        dev_tracks, clf, args.mode, args.window, args.objective
    )

    # 5. Final Evaluation Run with Best Params on both splits
    print("\nRunning final evaluation on Held-back Fold using trained model...")
    heldback_results = run_ml_evaluation_split(
        heldback_tracks, clf, 
        best_decoder_params["threshold"], 
        best_decoder_params["min_gap"], 
        best_decoder_params["max_boundaries"], 
        args.mode, args.window, args.n_bootstrap
    )
    
    print("\nRunning final evaluation on Inner Dev Fold using trained model...")
    dev_results = run_ml_evaluation_split(
        dev_tracks, clf, 
        best_decoder_params["threshold"], 
        best_decoder_params["min_gap"], 
        best_decoder_params["max_boundaries"], 
        args.mode, args.window, args.n_bootstrap
    )

    # 6. Print Reports
    print_report("Inner Dev Fold (Tuning/Train)", dev_results)
    print_report("Held-back Fold (Generalization/Test)", heldback_results)

    result = {
        "mode": args.mode,
        "window": args.window,
        "objective": args.objective,
        "n_configs_evaluated": n_configs_evaluated,
        "best_decoder_params": best_decoder_params,
        "feature_importances": {name: float(imp) for name, imp in zip(feature_names, importances)},
        "inner_dev": {
            "n_evaluated": len(dev_tracks),
            "aggregates": dev_results["aggregates"],
            "significance": dev_results["significance"],
            "aligned_human_subset": {
                "n": dev_results["aligned_human_subset"]["n"],
                "aggregates": dev_results["aligned_human_subset"]["aggregates"],
            }
        },
        "held_back": {
            "n_evaluated": len(heldback_tracks),
            "aggregates": heldback_results["aggregates"],
            "significance": heldback_results["significance"],
            "aligned_human_subset": {
                "n": heldback_results["aligned_human_subset"]["n"],
                "aggregates": heldback_results["aligned_human_subset"]["aggregates"],
            }
        }
    }

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"\nwrote {args.json_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
