#!/usr/bin/env python3
"""Phase 1d Evaluation: Optimal Segment-Path DP Decoder.

Implements a global 1-D segment-path Dynamic Programming decoder over candidates 
(baseline + refined + SSM peaks) to optimize spacing, placement, and count jointly.
Tunes DP parameters (boundary penalty, target duration, variance, weight) on the 
Inner Dev fold (80%) and reports generalization metrics on the Held-back fold (20%).
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

# Fixed SSM novelty parameters (from Phase 1b optimized HPO)
SSM_KERNEL_SIZE = 60
SSM_KERNEL_SIGMA = 1.0753
SSM_MIN_PROMINENCE = 0.2063
SSM_MIN_DISTANCE_SEC = 2.62
SSM_SNAP_WINDOW = 0.5


def run_dp_decoder(
    track: dict,
    candidates: list[float],
    novelties: list[float],
    lambda_val: float,
    target_dur: float,
    dur_sigma: float,
    weight_dur: float,
    min_gap: float,
    mode: str,
    window: float,
) -> list[float]:
    """Runs segment-path DP over candidates and returns optimal boundaries."""
    duration = float(track["duration"])
    
    if not candidates:
        return list(track["baseline_boundaries"])
        
    K = len(candidates)
    # Augment candidates with dummy boundary at 0.0 and duration
    times = [0.0] + candidates + [duration]
    
    # dp[k] represents the max utility of a path ending at times[k]
    dp = np.full(K + 2, -1e9)
    dp[0] = 0.0
    prev = np.zeros(K + 2, dtype=int)
    
    mu = np.log(target_dur)
    
    for k in range(1, K + 2):
        for j in range(k):
            d = times[k] - times[j]
            if d < min_gap:
                continue
                
            # Log-normal duration prior/cost
            log_prob = -((np.log(d) - mu) ** 2) / (2.0 * (dur_sigma ** 2)) - np.log(d * dur_sigma * np.sqrt(2.0 * np.pi))
            dur_cost = -weight_dur * log_prob
            
            # Reward for placing boundary at times[k] (if not dummy end node)
            reward = 0.0
            if k < K + 1:
                reward = novelties[k - 1] - lambda_val
                
            utility = dp[j] + reward - dur_cost
            if utility > dp[k]:
                dp[k] = utility
                prev[k] = j
                
    # Reconstruct optimal boundaries
    selected_times = []
    curr = K + 1
    while curr > 0:
        curr = prev[curr]
        if curr > 0:
            selected_times.append(times[curr])
            
    selected_times.reverse()
    
    # Fallback to refined boundaries if DP fails to pick anything
    if not selected_times:
        selected_times = list(track["refined_boundaries"])
        
    return _shift_predictions_for_mode(selected_times, mode, duration, window)


def evaluate_dp_config(
    tracks: list[dict],
    lambda_val: float,
    target_dur: float,
    dur_sigma: float,
    weight_dur: float,
    min_gap: float,
    mode: str,
    window: float,
    objective_type: str,
) -> float:
    """Return mean F1 objective score across tracks for a given DP configuration."""
    scores_list = []
    for track in tracks:
        try:
            pred = run_dp_decoder(
                track, track["candidates"], track["novelties"],
                lambda_val, target_dur, dur_sigma, weight_dur, min_gap, mode, window
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


def optimize_dp_hyperparameters(
    tracks: list[dict],
    mode: str,
    window: float,
    objective_type: str,
) -> tuple[dict[str, Any], int]:
    """Performs a grid/random search + Nelder-Mead polish to optimize DP decoder parameters."""
    print(f"\nRunning DP Hyperparameter Optimization using objective={objective_type}...")
    
    best_score = 0.0
    best_params = {}
    n_configs = 0
    
    # Grid/Random Search space:
    # lambda_val: -1.0 to 1.5 (novelty scores are normalized std multipliers)
    # target_dur: 15.0s to 35.0s
    # dur_sigma: 0.3 to 1.2
    # weight_dur: 0.1 to 2.5
    # min_gap: 3.0 to 7.0s
    
    rng = np.random.default_rng(42)
    trials = 100
    
    for trial in range(trials):
        lam = float(rng.uniform(-0.5, 1.2))
        tgt_dur = float(rng.uniform(15.0, 35.0))
        sigma = float(rng.uniform(0.3, 1.0))
        w_dur = float(rng.uniform(0.1, 2.0))
        gap = float(rng.choice([3.0, 4.0, 5.0, 6.0]))
        
        score = evaluate_dp_config(
            tracks, lam, tgt_dur, sigma, w_dur, gap, mode, window, objective_type
        )
        n_configs += 1
        
        if score > best_score:
            best_score = score
            best_params = {
                "lambda_val": lam,
                "target_dur": tgt_dur,
                "dur_sigma": sigma,
                "weight_dur": w_dur,
                "min_gap": gap
            }
            
    print(f"Random Search Best Score: {best_score*100:.2f}%")
    print("Best parameters:", best_params)
    
    # Nelder-Mead polish on continuous variables: lambda_val, target_dur, dur_sigma, weight_dur
    def objective(x):
        nonlocal n_configs
        lam_val, tgt_val, sig_val, w_val = x
        lam_val = clip(lam_val, -1.0, 2.0)
        tgt_val = clip(tgt_val, 10.0, 45.0)
        sig_val = clip(sig_val, 0.1, 2.0)
        w_val = clip(w_val, 0.01, 5.0)
        
        score = evaluate_dp_config(
            tracks, lam_val, tgt_val, sig_val, w_val, best_params["min_gap"],
            mode, window, objective_type
        )
        n_configs += 1
        return -score
        
    def clip(val, lo, hi):
        return max(lo, min(hi, val))
        
    from scipy.optimize import minimize
    initial_guess = [best_params["lambda_val"], best_params["target_dur"], best_params["dur_sigma"], best_params["weight_dur"]]
    res = minimize(
        objective,
        initial_guess,
        method="Nelder-Mead",
        options={"maxiter": 30, "disp": False}
    )
    
    opt_lam, opt_tgt, opt_sig, opt_w = res.x
    opt_lam = clip(opt_lam, -1.0, 2.0)
    opt_tgt = clip(opt_tgt, 10.0, 45.0)
    opt_sig = clip(opt_sig, 0.1, 2.0)
    opt_w = clip(opt_w, 0.01, 5.0)
    
    opt_score = evaluate_dp_config(
        tracks, opt_lam, opt_tgt, opt_sig, opt_w, best_params["min_gap"],
        mode, window, objective_type
    )
    n_configs += 1
    
    if opt_score > best_score:
        best_score = opt_score
        best_params["lambda_val"] = opt_lam
        best_params["target_dur"] = opt_tgt
        best_params["dur_sigma"] = opt_sig
        best_params["weight_dur"] = opt_w
        
    print(f"Nelder-Mead Polished Score: {best_score*100:.2f}%")
    print("Final Optimized Parameters:")
    for k, v in best_params.items():
        print(f"  {k}: {v}")
        
    return best_params, n_configs


def run_dp_evaluation_split(
    tracks: list[dict],
    best_params: dict[str, Any],
    mode: str,
    window: float,
    n_bootstrap: int,
) -> dict[str, Any]:
    """Runs final evaluation and computes statistics for the DP model on a subset of tracks."""
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
                
        # Generate DP predictions
        pred = run_dp_decoder(
            track, track["candidates"], track["novelties"],
            best_params["lambda_val"], best_params["target_dur"],
            best_params["dur_sigma"], best_params["weight_dur"],
            best_params["min_gap"], mode, window
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
        summary["boundary_counts"]["candidates"] = len(track["candidates"])
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


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 1d DP Optimal Partition evaluation.")
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
    print(f"  Inner Dev Fold: {len(dev_tracks)} tracks")
    print(f"  Held-back Fold: {len(heldback_tracks)} tracks")

    # 2. Candidate Generation & Feature/Novelty Extraction
    print("\nExtracting candidates and novelty scores...")
    opt_kernel = make_checkerboard_kernel(SSM_KERNEL_SIZE, SSM_KERNEL_SIGMA)
    
    for track in tracks:
        duration = float(track["duration"])
        offset = calculate_crop_offset(duration, args.window)
        
        # Compute SSM Novelty
        novelty = compute_ssm_novelty(track, opt_kernel)
        min_dist_frames = max(1, int(round(SSM_MIN_DISTANCE_SEC / 0.2)))
        novelty_std = np.std(novelty)
        prom = SSM_MIN_PROMINENCE * (novelty_std if novelty_std > 1e-8 else 1.0)
        peaks, _ = find_peaks(novelty, prominence=prom, distance=min_dist_frames)
        
        chroma_times = np.array(track["chroma_times"])
        peaks_abs = []
        if len(peaks) > 0:
            peaks_abs = to_absolute_time([chroma_times[p] for p in peaks], offset)
        onset_times = np.array(track["onsets"].get("times", []))
        onset_peaks_abs = to_absolute_time(onset_times, offset)
        if len(onset_peaks_abs) > 0 and len(peaks_abs) > 0:
            peaks_abs = snap(peaks_abs, onset_peaks_abs, window=0.5)
            
        candidates = []
        candidates.extend(track["baseline_boundaries"])
        candidates.extend(track["refined_boundaries"])
        candidates.extend(peaks_abs)
        candidates = sorted(list(set(candidates)))
        
        # Novelties values at each candidate (crop-relative)
        novelties = []
        for c in candidates:
            c_crop = c - offset
            val = float(np.interp(c_crop, chroma_times, novelty)) if chroma_times.size > 0 else 0.0
            # Normalize novelty locally
            val = val / (novelty_std if novelty_std > 1e-8 else 1.0)
            novelties.append(val)
            
        track["candidates"] = candidates
        track["novelties"] = novelties

    # 3. Hyperparameter Optimization on dev_tracks
    best_params, n_configs_evaluated = optimize_dp_hyperparameters(
        dev_tracks, args.mode, args.window, args.objective
    )

    # 4. Final Evaluation Run with Best Params on both splits
    print("\nRunning final evaluation on Held-back Fold...")
    heldback_results = run_dp_evaluation_split(
        heldback_tracks, best_params, args.mode, args.window, args.n_bootstrap
    )
    
    print("\nRunning final evaluation on Inner Dev Fold...")
    dev_results = run_dp_evaluation_split(
        dev_tracks, best_params, args.mode, args.window, args.n_bootstrap
    )

    # 5. Print Reports
    print_report("Inner Dev Fold (Tuning/Train)", dev_results)
    print_report("Held-back Fold (Generalization/Test)", heldback_results)

    result = {
        "mode": args.mode,
        "window": args.window,
        "objective": args.objective,
        "n_configs_evaluated": n_configs_evaluated,
        "best_params": best_params,
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
