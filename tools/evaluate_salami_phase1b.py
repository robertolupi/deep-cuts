#!/usr/bin/env python3
"""Phase 1b Evaluation: Fused Dense SSM Prototype.

Computes cosine similarity Self-Similarity Matrices (SSM) from cached chroma,
correlates them with Gaussian checkerboard kernels to produce novelty curves,
snaps peaks to onsets, and performs SciPy-based hyperparameter optimization (HPO)
on the validation split.
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

# Import Phase 0 helpers
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
from refine_salami_boundaries import (
    augment_with_peaks,
    snap,
)
from evaluate_salami_phase1a import (
    compute_chroma_novelty,
    load_high_res_features,
    paired_wilcoxon_phase1a,
)

warnings.filterwarnings("ignore", category=UserWarning)

VARIANTS = ["baseline", "refined", "ssm_fused", "oracle", "human"]

def _aggregate_tracks_phase1b(tracks: list[dict], tolerances: Tolerances, n_bootstrap: int) -> dict:
    aggregates: dict[str, Any] = {}
    for variant in VARIANTS:
        if not any(variant in t["scores"] for t in tracks):
            continue
        aggregates[variant] = {}
        for tol in tolerances:
            key = str(tol)
            aggregates[variant][key] = {}
            for metric in ("precision", "recall", "f1"):
                values = np.array([t["scores"][variant][key][metric] for t in tracks if variant in t["scores"]])
                mean, lo, hi = bootstrap_ci(values, n_bootstrap)
                aggregates[variant][key][metric] = {
                    "mean": mean,
                    "ci95": [lo, hi],
                    "n": int(values.size),
                }

    aggregates["pairwise_label_f1"] = {}
    pairwise = np.array([t["pairwise_label_f1"] for t in tracks], dtype=float)
    mean, lo, hi = bootstrap_ci(pairwise, n_bootstrap)
    aggregates["pairwise_label_f1"]["mean"] = mean
    aggregates["pairwise_label_f1"]["ci95"] = [lo, hi]
    aggregates["pairwise_label_f1"]["n"] = int(pairwise.size)
    return aggregates



def make_checkerboard_kernel(L: int, sigma: float) -> np.ndarray:
    """Create a symmetric L x L Gaussian checkerboard kernel."""
    if L < 2:
        return np.ones((1, 1))
    half = L / 2.0
    x = np.linspace(-half + 0.5, half - 0.5, L)
    X, Y = np.meshgrid(x, x)
    
    g = np.exp(-(X**2 + Y**2) / (2.0 * sigma**2))
    s = np.sign(X) * np.sign(Y)
    kernel = g * s
    
    pos_sum = np.sum(kernel[kernel > 0])
    if pos_sum > 0:
        kernel /= pos_sum
    return kernel


def compute_ssm_novelty(track: dict, kernel: np.ndarray) -> np.ndarray:
    """Compute cosine distance SSM and diagonal checkerboard novelty curve, caching SSM."""
    chroma_series = track["chroma_series"]
    if not chroma_series or len(chroma_series) < kernel.shape[0]:
        return np.zeros(len(chroma_series))
        
    if "ssm" not in track:
        X = np.array(chroma_series)
        norms = np.linalg.norm(X, axis=1, keepdims=True)
        norms[norms < 1e-8] = 1.0
        X_norm = X / norms
        track["ssm"] = X_norm @ X_norm.T
        
    S = track["ssm"]
    N = S.shape[0]
    L = kernel.shape[0]
    if N < L:
        return np.zeros(N)
    half = L // 2
    novelty = np.zeros(N)
    
    for t in range(half, N - half):
        sub_S = S[t - half : t + half, t - half : t + half]
        novelty[t] = np.sum(sub_S * kernel)
        
    return novelty


def run_ssm_segmentation(
    track: dict,
    mode: Literal["legacy", "windowed"],
    window: float,
    tolerances: Tolerances,
    # Hyperparameters to evaluate
    kernel_size: int,
    kernel_sigma: float,
    min_prominence: float,
    min_distance_sec: float,
    onset_snap_window: float,
    strategy: Literal["augment", "replace"],
    n_add_or_replace: int,
) -> list[float]:
    """Runs the SSM + checkerboard boundary detection pipeline for one track."""
    duration = float(track["duration"])
    offset = calculate_crop_offset(duration, window)
    
    chroma_series = track["chroma_series"]
    chroma_times = track["chroma_times"]
    
    if not chroma_series or len(chroma_series) < kernel_size:
        return list(track["baseline_boundaries"])
        
    # 1. Checkerboard Novelty (uses cached SSM on track)
    kernel = make_checkerboard_kernel(kernel_size, kernel_sigma)
    novelty = compute_ssm_novelty(track, kernel)
    
    # 2. Peak Picking
    # Convert min_distance_sec to frames (chroma_time_step is ~0.2s)
    min_dist_frames = max(1, int(round(min_distance_sec / 0.2)))
    novelty_std = np.std(novelty)
    prom = min_prominence * (novelty_std if novelty_std > 1e-8 else 1.0)
    peaks, properties = find_peaks(novelty, prominence=prom, distance=min_dist_frames)
    
    if len(peaks) == 0:
        return list(track["baseline_boundaries"])
        
    # Sort picked peaks by prominence
    prominences = properties["prominences"]
    peak_times_crop = [chroma_times[p] for p in peaks]
    ranked_peaks_crop = sorted(zip(peak_times_crop, prominences), key=lambda x: -x[1])
    
    # Shift to absolute time
    peaks_abs = to_absolute_time([p[0] for p in ranked_peaks_crop], offset)
    
    # 3. Onset Snapping (if window > 0)
    onset_times = track["onsets"].get("times", [])
    onset_peaks_abs = to_absolute_time(onset_times, offset)
    
    if onset_snap_window > 0.0 and onset_peaks_abs:
        peaks_final = snap(peaks_abs, onset_peaks_abs, window=onset_snap_window)
    else:
        peaks_final = peaks_abs
        
    # 4. Strategy: Augment Baseline vs Replace completely
    if strategy == "augment":
        ranked_peaks_final = [(t, 1.0) for t in peaks_final]
        pred_full = augment_with_peaks(track["baseline_boundaries"], ranked_peaks_final, n_add_or_replace, min_gap=5.0)
    else:
        # Purely predict from top-N SSM peaks
        pred_full = sorted(peaks_final[:n_add_or_replace])
        
    return _shift_predictions_for_mode(pred_full, mode, duration, window)


def evaluate_config(
    tracks: list[dict],
    mode: Literal["legacy", "windowed"],
    window: float,
    tolerances: Tolerances,
    # Hyperparams
    kernel_size: int,
    kernel_sigma: float,
    min_prominence: float,
    min_distance_sec: float,
    onset_snap_window: float,
    strategy: Literal["augment", "replace"],
    n_add_or_replace: int,
    objective_type: Literal["joint", "0.5", "3.0"] = "joint",
) -> float:
    """Return mean F1 objective score across all tracks for this parameter config."""
    scores_list = []
    for track in tracks:
        try:
            pred = run_ssm_segmentation(
                track, mode, window, tolerances,
                kernel_size, kernel_sigma, min_prominence,
                min_distance_sec, onset_snap_window, strategy, n_add_or_replace
            )
            
            # Score
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


def optimize_hyperparameters(
    tracks: list[dict], 
    mode: str, 
    window: float, 
    objective_type: Literal["joint", "0.5", "3.0"] = "joint"
) -> tuple[dict[str, Any], int]:
    """Run a pre-registered optimization sweep over the validation subset."""
    print(f"\nRunning Hyperparameter Optimization (SciPy Nelder-Mead) using objective={objective_type}...")
    
    best_score = 0.0
    best_params = {}
    n_configs_evaluated = 0
    
    # Pre-registered HPO parameter bounds/grids:
    # kernel_size (must be even): 10, 20, 30, 40, 50
    # kernel_sigma: 0.1 to 1.0
    # min_prominence: 0.05 to 2.0 (standard deviation multiplier)
    # min_distance_sec: 1.0 to 10.0
    # onset_snap_window: 0.0 (no snap), 0.25, 0.5
    # strategy: "augment" vs "replace"
    # n_add_or_replace: 2 to 12
    
    # Let's perform a fast random search of 100 trials first to find the best region
    rng = np.random.default_rng(42)
    trials = 100
    
    for trial in range(trials):
        k_size = int(rng.choice([10, 20, 30, 40, 50, 60]))
        sigma = float(rng.uniform(0.1, 1.2))
        prom = float(rng.uniform(0.05, 1.5))
        dist_sec = float(rng.uniform(2.0, 10.0))
        snap_win = float(rng.choice([0.0, 0.25, 0.5, 0.75]))
        strat = str(rng.choice(["augment", "replace"]))
        n_add = int(rng.integers(3, 12))
        
        score = evaluate_config(
            tracks, mode, window, (0.5, 3.0),
            k_size, sigma, prom, dist_sec, snap_win, strat, n_add,
            objective_type=objective_type
        )
        n_configs_evaluated += 1
        
        if score > best_score:
            best_score = score
            best_params = {
                "kernel_size": k_size,
                "kernel_sigma": sigma,
                "min_prominence": prom,
                "min_distance_sec": dist_sec,
                "onset_snap_window": snap_win,
                "strategy": strat,
                "n_add_or_replace": n_add
            }
            
    print(f"Random Search Best Score: {best_score*100:.2f}%")
    print("Best params:", best_params)
    
    # Nelder-Mead local polish on continuous params: sigma, prominence (std mult), distance
    # Nelder-Mead objective function
    def objective(x):
        nonlocal n_configs_evaluated
        sigma_val, prom_val, dist_val = x
        sigma_val = clip(sigma_val, 0.05, 2.0)
        prom_val = clip(prom_val, 0.001, 5.0)
        dist_val = clip(dist_val, 1.0, 15.0)
        
        score = evaluate_config(
            tracks, mode, window, (0.5, 3.0),
            best_params["kernel_size"], sigma_val, prom_val, dist_val,
            best_params["onset_snap_window"], best_params["strategy"],
            best_params["n_add_or_replace"],
            objective_type=objective_type
        )
        n_configs_evaluated += 1
        return -score  # minimize negative score
        
    def clip(val, lo, hi):
        return max(lo, min(hi, val))
        
    from scipy.optimize import minimize
    initial_guess = [best_params["kernel_sigma"], best_params["min_prominence"], best_params["min_distance_sec"]]
    
    res = minimize(
        objective,
        initial_guess,
        method="Nelder-Mead",
        options={"maxiter": 30, "disp": False}
    )
    
    # Extract optimized params
    opt_sigma, opt_prom, opt_dist = res.x
    opt_sigma = clip(opt_sigma, 0.05, 2.0)
    opt_prom = clip(opt_prom, 0.001, 5.0)
    opt_dist = clip(opt_dist, 1.0, 15.0)
    
    opt_score = evaluate_config(
        tracks, mode, window, (0.5, 3.0),
        best_params["kernel_size"], opt_sigma, opt_prom, opt_dist,
        best_params["onset_snap_window"], best_params["strategy"],
        best_params["n_add_or_replace"],
        objective_type=objective_type
    )
    n_configs_evaluated += 1
    
    if opt_score > best_score:
        best_score = opt_score
        best_params["kernel_sigma"] = opt_sigma
        best_params["min_prominence"] = opt_prom
        best_params["min_distance_sec"] = opt_dist
        
    print(f"Nelder-Mead Polished Score: {best_score*100:.2f}%")
    print("Final Optimized Parameters:")
    for k, v in best_params.items():
        print(f"  {k}: {v}")
        
    return best_params, n_configs_evaluated


def run_evaluation_split(
    tracks: list[dict],
    mode: str,
    window: float,
    best_params: dict[str, Any],
    n_bootstrap: int,
) -> dict[str, Any]:
    """Runs final evaluation and computes statistics for a given track subset."""
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
                
        # Generate predictions
        pred = run_ssm_segmentation(
            track, mode, window, (0.5, 3.0),
            best_params["kernel_size"], best_params["kernel_sigma"],
            best_params["min_prominence"], best_params["min_distance_sec"],
            best_params["onset_snap_window"], best_params["strategy"],
            best_params["n_add_or_replace"]
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


def print_report(name: str, eval_results: dict[str, Any]) -> None:
    """Print standard validation report for a split."""
    print(f"\n==========================================")
    print(f" {name} Report (N={eval_results['n_evaluated']})")
    print(f"==========================================")
    print("-" * 110)
    print(f"{'variant':<22}{'F1@0.5s':>18}{'F1@3.0s':>18}{'avg #bnd':>12}")
    print("-" * 110)
    
    active_ag = eval_results["aligned_human_subset"]["aggregates"] or eval_results["aggregates"]
    variants_list = ["baseline", "refined", "ssm_fused", "oracle", "human"]
    for variant in variants_list:
        if variant not in active_ag:
            continue
        f05 = active_ag[variant]["0.5"]["f1"]
        f30 = active_ag[variant]["3.0"]["f1"]
        counts = [t["boundary_counts"].get(variant, 0) for t in eval_results["per_track"]]
        avg_count = np.mean(counts) if counts else 0.0
        
        print(
            f"{variant:<22}"
            f"{f05['mean'] * 100:6.2f}% [{f05['ci95'][0] * 100:5.2f}, {f05['ci95'][1] * 100:5.2f}]  "
            f"{f30['mean'] * 100:6.2f}% [{f30['ci95'][0] * 100:5.2f}, {f30['ci95'][1] * 100:5.2f}]  "
            f"{avg_count:9.2f}"
        )
    
    pw = active_ag["pairwise_label_f1"]
    print("-" * 110)
    print(f"pairwise label F1: {pw['mean'] * 100:.2f}%  CI=[{pw['ci95'][0] * 100:.2f}, {pw['ci95'][1] * 100:.2f}]")
    
    print("\nSignificance of SSM vs Refined (F1@0.5s):")
    sig = eval_results["significance"]["0.5"]["ssm_vs_refined"]
    print(f"  p={sig['p']:.2e}  mean_diff={sig['mean_diff']*100:+.2f}% (N={sig['n']})")
    
    print("\nSignificance of SSM vs Refined (F1@3.0s):")
    sig = eval_results["significance"]["3.0"]["ssm_vs_refined"]
    print(f"  p={sig['p']:.2e}  mean_diff={sig['mean_diff']*100:+.2f}% (N={sig['n']})")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 1b SALAMI boundary evaluation.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="windowed")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=2000)
    parser.add_argument("--hpo", action="store_true", help="run hyperparameter optimization")
    parser.add_argument("--objective", choices=["joint", "0.5", "3.0"], default="joint", help="HPO objective metric")
    parser.add_argument("--allow-holdout", action="store_true", help="custodian-only: allow holdout split")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    # Reject holdout split check (structural check)
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
    print(f"  Inner Dev Fold: {len(dev_tracks)} tracks (for HPO parameter tuning)")
    print(f"  Held-back Fold: {len(heldback_tracks)} tracks (unbiased validation generalization)")

    # 2. Hyperparameter Optimization on dev_tracks
    n_configs_evaluated = 0
    if args.hpo:
        best_params, n_configs_evaluated = optimize_hyperparameters(
            dev_tracks, args.mode, args.window, objective_type=args.objective
        )
    else:
        # Default starting/optimized params
        best_params = {
            "kernel_size": 10,
            "kernel_sigma": 0.2274,
            "min_prominence": 0.5, # 0.5 * std
            "min_distance_sec": 2.82,
            "onset_snap_window": 0.75,
            "strategy": "augment",
            "n_add_or_replace": 8
        }

    # 3. Final Evaluation Run with Best Params on both splits
    print("\nRunning final evaluation on Held-back Fold...")
    heldback_results = run_evaluation_split(
        heldback_tracks, args.mode, args.window, best_params, args.n_bootstrap
    )
    
    print("\nRunning final evaluation on Inner Dev Fold...")
    dev_results = run_evaluation_split(
        dev_tracks, args.mode, args.window, best_params, args.n_bootstrap
    )

    # 4. Print Reports
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
