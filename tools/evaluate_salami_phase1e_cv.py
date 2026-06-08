#!/usr/bin/env python3
"""Cross-Validation for Phase 1e: Hybrid RF+DP Decoder.

Performs an honest 5-fold cross-validation over the 229 validation tracks,
re-training the Random Forest classifier on each training split, predicting
on the test fold, and evaluating overall cross-validated metrics vs the refined baseline.
"""

from __future__ import annotations

import argparse
import json
import sys
import warnings
from pathlib import Path

import numpy as np
from sklearn.ensemble import RandomForestClassifier

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
sys.path.insert(0, str(SCRIPT_DIR))

# Import Helpers
from evaluate_salami_phase0 import (
    DEFAULT_DB_PATH,
    DEFAULT_VALIDATION_SPLIT,
    _aligned_human_subset,
    _prepare_pass_for_mode,
    _score_boundaries,
    _score_human,
    _score_variant_for_pass,
    _mean_score,
    _shift_predictions_for_mode,
    _reject_holdout,
    bootstrap_ci,
    calculate_crop_offset,
    load_track,
)
from evaluate_salami_boundaries import (
    JAMS_DIR,
    evaluate_pairwise_clustering,
    load_onset_map,
    parse_jams_boundaries_and_labels,
    project_jams_to_16_bins,
)
from evaluate_salami_phase1a import load_high_res_features, paired_wilcoxon_phase1a
from evaluate_salami_phase1b import _aggregate_tracks_phase1b, print_report
from evaluate_salami_phase1c import extract_track_candidates_and_features
from evaluate_salami_phase1e import run_hybrid_dp_decoder

warnings.filterwarnings("ignore", category=UserWarning)

# Best HPO parameters from seed 42 run
BEST_PARAMS = {
    "lambda_val": -3.4227534583703303,
    "target_dur": 15.284694006411751,
    "dur_sigma": 0.4445606546359455,
    "weight_dur": 0.34977771728307283,
    "min_gap": 5.0,
}


def main():
    parser = argparse.ArgumentParser(description="Cross-validate Phase 1e Hybrid RF+DP.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="windowed")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=200)
    parser.add_argument("--seed", type=int, default=42, help="Seed for split shuffling")
    parser.add_argument("--allow-holdout", action="store_true")
    args = parser.parse_args()

    _reject_holdout(args.split_json, args.allow_holdout)

    with open(args.split_json, "r", encoding="utf-8") as f:
        split_entries = json.load(f)

    # 1. Load tracks
    print("Loading track features and annotations...")
    tracks = []
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
                continue
            track["passes"] = passes
            track = load_high_res_features(track, args.db_path)
            tracks.append(track)
        except Exception:
            pass

    print(f"Loaded {len(tracks)} tracks.")

    # 2. Extract features for all tracks
    print("Extracting candidates and feature vectors...")
    for track in tracks:
        cands, feats, targets = extract_track_candidates_and_features(track, args.mode, args.window)
        track["candidates"] = cands
        track["features"] = feats
        track["targets"] = targets
        track["probs"] = np.array([])  # to be filled during CV

    # 3. Perform 5-Fold Cross Validation
    rng = np.random.default_rng(args.seed)
    shuffled_tracks = list(tracks)
    rng.shuffle(shuffled_tracks)
    
    folds = np.array_split(shuffled_tracks, 5)
    print(f"\nRunning 5-fold cross-validation with seed={args.seed}...")

    for f_idx in range(5):
        test_tracks = list(folds[f_idx])
        train_tracks = []
        for i in range(5):
            if i != f_idx:
                train_tracks.extend(list(folds[i]))
                
        # Gather training features
        X_train_list = [t["features"] for t in train_tracks if t["features"].size > 0]
        y_train_list = [t["targets"] for t in train_tracks if t["features"].size > 0]
        
        X_train = np.vstack(X_train_list)
        y_train = np.concatenate(y_train_list)
        
        # Train model
        clf = RandomForestClassifier(n_estimators=100, max_depth=6, min_samples_leaf=4, random_state=args.seed)
        clf.fit(X_train, y_train)
        
        # Predict on test fold
        for track in test_tracks:
            if track["candidates"] and track["features"].size > 0:
                track["probs"] = clf.predict_proba(track["features"])[:, 1]
            else:
                track["probs"] = np.array([])

    # 4. Evaluate overall predictions (all tracks evaluated once under CV)
    print("\nRunning evaluation on CV predictions...")
    evaluated = []
    for track in tracks:
        duration = float(track["duration"])
        offset = calculate_crop_offset(duration, args.window)
        
        prepared_passes = []
        eval_duration = duration
        for pass_info in track["passes"]:
            prepared, eval_duration, offset = _prepare_pass_for_mode(pass_info, args.mode, duration, args.window)
            if prepared["segments"]:
                prepared_passes.append(prepared)
                
        # Generate DP predictions using CV probabilities
        pred = run_hybrid_dp_decoder(
            track, track["candidates"], track["probs"],
            BEST_PARAMS["lambda_val"], BEST_PARAMS["target_dur"],
            BEST_PARAMS["dur_sigma"], BEST_PARAMS["weight_dur"],
            BEST_PARAMS["min_gap"], args.mode, args.window
        )
        
        base = _shift_predictions_for_mode(track["baseline_boundaries"], args.mode, duration, args.window)
        refined = _shift_predictions_for_mode(track["refined_boundaries"], args.mode, duration, args.window)
        
        cand = {
            "baseline": base,
            "refined": refined,
            "ssm_fused": pred,
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
            "boundary_counts": {k: len(v) for k, v in cand.items()},
            "pairwise_label_f1": float(np.mean(pairwise)) if pairwise else 0.0,
            "scores": {},
        }
        summary["boundary_counts"]["candidates"] = len(track["candidates"])
        summary["boundary_counts"]["gt_mean"] = float(np.mean([len(p["boundaries"]) for p in prepared_passes]))
        
        for variant, variant_scores in per_pass.items():
            summary["scores"][variant] = {str(t): _mean_score(variant_scores, t) for t in (0.5, 3.0)}
            
        oracle_scores = []
        for prepared in prepared_passes:
            _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
            oracle_scores.append(_score_variant_for_pass(prepared["boundaries"], oracle_bounds, eval_duration, (0.5, 3.0)))
        summary["scores"]["oracle"] = {str(t): _mean_score(oracle_scores, t) for t in (0.5, 3.0)}

        human = _score_human(prepared_passes, eval_duration, (0.5, 3.0))
        if human is not None:
            summary["scores"]["human"] = human
            
        evaluated.append(summary)

    aggregates = _aggregate_tracks_phase1b(evaluated, (0.5, 3.0), args.n_bootstrap)
    aligned = _aligned_human_subset(evaluated)
    
    # Compute significance
    sig_out = {}
    for tol in (0.5, 3.0):
        key = str(tol)
        refined_scores = np.array([t["scores"]["refined"][key]["f1"] for t in evaluated])
        ssm_scores = np.array([t["scores"]["ssm_fused"][key]["f1"] for t in evaluated])
        sig_out[key] = {
            "ssm_vs_refined": paired_wilcoxon_phase1a(ssm_scores, refined_scores),
        }
        
    print_report(f"Overall 5-Fold Cross-Validation (Seed {args.seed})", {
        "n_evaluated": len(evaluated),
        "per_track": evaluated,
        "aggregates": aggregates,
        "significance": sig_out,
        "aligned_human_subset": {
            "n": len(aligned),
            "aggregates": _aggregate_tracks_phase1b(aligned, (0.5, 3.0), args.n_bootstrap) if aligned else {},
            "significance": sig_out,
        }
    })


if __name__ == "__main__":
    main()
