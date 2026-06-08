#!/usr/bin/env python3
"""Phase 1 Diagnostics: Candidate Ceiling & Stock Foote Baseline.

1. Candidate-Ceiling Test: Evaluates the maximum possible F1 score achievable 
   by selecting an oracle subset of the generated candidate pool (baseline + refined + SSM peaks).
2. Stock Foote Baseline: Evaluates a standard, unoptimized Foote novelty detector 
   (using Librosa/SciPy stock parameters) to establish the off-the-shelf reference point.
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


def compute_candidate_oracle_boundaries(ref_boundaries: list[float], candidates: list[float], tolerance: float) -> list[float]:
    """Selects the subset of candidates that match the reference boundaries within tolerance."""
    matched_candidates = []
    for ref in ref_boundaries:
        candidates_in_tol = [c for c in candidates if abs(c - ref) <= tolerance]
        if candidates_in_tol:
            # Pick the closest one
            best_c = min(candidates_in_tol, key=lambda x: abs(x - ref))
            matched_candidates.append(best_c)
    return sorted(list(set(matched_candidates)))


def run_stock_foote(track: dict, mode: str, window: float) -> list[float]:
    """Runs a standard, stock Foote detector (unoptimized parameters)."""
    duration = float(track["duration"])
    offset = calculate_crop_offset(duration, window)
    
    # Standard stock settings: kernel size 30 (6 seconds), sigma 0.5, min prominence 0.1
    kernel = make_checkerboard_kernel(30, 0.5)
    novelty = compute_ssm_novelty(track, kernel)
    
    min_dist_frames = max(1, int(round(5.0 / 0.2)))  # 5s min distance
    peaks, _ = find_peaks(novelty, prominence=0.1, distance=min_dist_frames)
    
    chroma_times = np.array(track["chroma_times"])
    if len(peaks) == 0:
        return list(track["baseline_boundaries"])
        
    peaks_abs = to_absolute_time([chroma_times[p] for p in peaks], offset)
    
    # Snap to onsets with 0.5s window
    onset_times = np.array(track["onsets"].get("times", []))
    onset_peaks_abs = to_absolute_time(onset_times, offset)
    if len(onset_peaks_abs) > 0:
        peaks_final = snap(peaks_abs, onset_peaks_abs, window=0.5)
    else:
        peaks_final = peaks_abs
        
    # Augment baseline with stock peaks (up to 8)
    from refine_salami_boundaries import augment_with_peaks
    ranked_peaks = [(t, 1.0) for t in peaks_final]
    pred_full = augment_with_peaks(track["baseline_boundaries"], ranked_peaks, 8, min_gap=5.0)
    
    return _shift_predictions_for_mode(pred_full, mode, duration, window)


def run_diagnostics(
    tracks: list[dict],
    mode: str,
    window: float,
    n_bootstrap: int,
) -> dict[str, Any]:
    """Runs candidate-ceiling and stock Foote evaluations on the tracks."""
    evaluated = []
    
    # Phase 1b optimized params for candidate generation
    opt_kernel = make_checkerboard_kernel(60, 1.0753)
    
    for track in tracks:
        duration = float(track["duration"])
        offset = calculate_crop_offset(duration, window)
        
        prepared_passes = []
        eval_duration = duration
        for pass_info in track["passes"]:
            prepared, eval_duration, offset = _prepare_pass_for_mode(pass_info, mode, duration, window)
            if prepared["segments"]:
                prepared_passes.append(prepared)
                
        # 1. Generate Candidate Pool for this track
        # Baseline + Refined + SSM peaks
        novelty = compute_ssm_novelty(track, opt_kernel)
        min_dist_frames = max(1, int(round(2.62 / 0.2)))
        novelty_std = np.std(novelty)
        prom = 0.2063 * (novelty_std if novelty_std > 1e-8 else 1.0)
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
        
        # 2. Evaluate Stock Foote novelty
        stock_foote_pred = run_stock_foote(track, mode, window)
        
        # 3. Evaluate Candidate Oracle (ceiling) for each tolerance separately
        # We need to compute predictions *per tolerance* because the oracle selection depends on tolerance
        cand = {
            "baseline": _shift_predictions_for_mode(track["baseline_boundaries"], mode, duration, window),
            "refined": _shift_predictions_for_mode(track["refined_boundaries"], mode, duration, window),
            "stock_foote": stock_foote_pred,
        }
        
        per_pass = {k: [] for k in cand.keys()}
        per_pass["cand_ceiling"] = []
        
        pairwise = []
        for prepared in prepared_passes:
            _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
            cand["oracle"] = oracle_bounds

            # For candidate ceiling, compute the oracle boundaries for 0.5s and 3.0s separately
            pred_ceiling_05 = compute_candidate_oracle_boundaries(prepared["boundaries"], candidates, 0.5)
            pred_ceiling_30 = compute_candidate_oracle_boundaries(prepared["boundaries"], candidates, 3.0)
            
            # Score baseline, refined, stock_foote
            for variant in ("baseline", "refined", "stock_foote"):
                per_pass[variant].append(_score_variant_for_pass(prepared["boundaries"], cand[variant], eval_duration, (0.5, 3.0)))
                
            # Score ceiling
            score_05 = _score_boundaries(prepared["boundaries"], pred_ceiling_05, eval_duration, 0.5)
            score_30 = _score_boundaries(prepared["boundaries"], pred_ceiling_30, eval_duration, 3.0)
            per_pass["cand_ceiling"].append({
                "0.5": score_05,
                "3.0": score_30
            })
            
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
        # Add candidate count to boundary_counts
        summary["boundary_counts"]["candidates"] = len(candidates)
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

    # Aggregate results
    variants_list = ["baseline", "refined", "stock_foote", "cand_ceiling", "oracle", "human"]
    aggregates = {}
    for variant in variants_list:
        aggregates[variant] = {}
        for tol in (0.5, 3.0):
            key = str(tol)
            aggregates[variant][key] = {}
            for metric in ("precision", "recall", "f1"):
                values = np.array([t["scores"][variant][key][metric] for t in evaluated if variant in t["scores"]])
                mean, lo, hi = bootstrap_ci(values, n_bootstrap)
                aggregates[variant][key][metric] = {
                    "mean": mean,
                    "ci95": [lo, hi],
                    "n": int(values.size),
                }
    aggregates["pairwise_label_f1"] = {}
    pairwise = np.array([t["pairwise_label_f1"] for t in evaluated], dtype=float)
    mean, lo, hi = bootstrap_ci(pairwise, n_bootstrap)
    aggregates["pairwise_label_f1"]["mean"] = mean
    aggregates["pairwise_label_f1"]["ci95"] = [lo, hi]
    aggregates["pairwise_label_f1"]["n"] = int(pairwise.size)
    
    aligned = _aligned_human_subset(evaluated)
    
    # Compute significance of Stock Foote and Ceiling vs Refined
    sig_out = {}
    for tol in (0.5, 3.0):
        key = str(tol)
        refined_scores = np.array([t["scores"]["refined"][key]["f1"] for t in evaluated])
        stock_scores = np.array([t["scores"]["stock_foote"][key]["f1"] for t in evaluated])
        ceil_scores = np.array([t["scores"]["cand_ceiling"][key]["f1"] for t in evaluated])
        oracle_scores = np.array([t["scores"]["oracle"][key]["f1"] for t in evaluated])
        
        sig_out[key] = {
            "stock_vs_refined": paired_wilcoxon_phase1a(stock_scores, refined_scores),
            "ceil_vs_refined": paired_wilcoxon_phase1a(ceil_scores, refined_scores),
            "ceil_vs_oracle": paired_wilcoxon_phase1a(ceil_scores, oracle_scores)
        }
        
    return {
        "n_evaluated": len(evaluated),
        "per_track": evaluated,
        "aggregates": aggregates,
        "significance": sig_out,
        "aligned_human_subset": {
            "n": len(aligned),
            "aggregates": run_diagnostics_aligned_subset(aligned, n_bootstrap) if aligned else {},
        }
    }


def run_diagnostics_aligned_subset(aligned_tracks: list[dict], n_bootstrap: int) -> dict[str, Any]:
    """Helper to aggregate diagnostics for human aligned subset."""
    variants_list = ["baseline", "refined", "stock_foote", "cand_ceiling", "oracle", "human"]
    aggregates = {}
    for variant in variants_list:
        aggregates[variant] = {}
        for tol in (0.5, 3.0):
            key = str(tol)
            aggregates[variant][key] = {}
            for metric in ("precision", "recall", "f1"):
                values = np.array([t["scores"][variant][key][metric] for t in aligned_tracks if variant in t["scores"]])
                mean, lo, hi = bootstrap_ci(values, n_bootstrap)
                aggregates[variant][key][metric] = {
                    "mean": mean,
                    "ci95": [lo, hi],
                    "n": int(values.size),
                }
    aggregates["pairwise_label_f1"] = {}
    pairwise = np.array([t["pairwise_label_f1"] for t in aligned_tracks], dtype=float)
    mean, lo, hi = bootstrap_ci(pairwise, n_bootstrap)
    aggregates["pairwise_label_f1"]["mean"] = mean
    aggregates["pairwise_label_f1"]["ci95"] = [lo, hi]
    aggregates["pairwise_label_f1"]["n"] = int(pairwise.size)
    return aggregates


def print_diag_report(name: str, eval_results: dict[str, Any]) -> None:
    """Print diagnostics report."""
    print(f"\n==========================================")
    print(f" {name} Diagnostic Report (N={eval_results['n_evaluated']})")
    print(f"==========================================")
    print("-" * 110)
    print(f"{'variant':<22}{'F1@0.5s':>18}{'F1@3.0s':>18}{'avg #bnd':>12}")
    print("-" * 110)
    
    active_ag = eval_results["aligned_human_subset"]["aggregates"] or eval_results["aggregates"]
    variants_list = ["baseline", "refined", "stock_foote", "cand_ceiling", "oracle", "human"]
    for variant in variants_list:
        if variant not in active_ag:
            continue
        f05 = active_ag[variant]["0.5"]["f1"]
        f30 = active_ag[variant]["3.0"]["f1"]
        counts = [t["boundary_counts"].get(variant, 0) for t in eval_results["per_track"]]
        avg_count = np.mean(counts) if counts else 0.0
        
        # Override candidates count print
        if variant == "cand_ceiling":
            counts = [t["boundary_counts"].get("candidates", 0) for t in eval_results["per_track"]]
            avg_count = np.mean(counts) if counts else 0.0
            variant_name = "cand_ceiling (pool)"
        else:
            variant_name = variant
            
        print(
            f"{variant_name:<22}"
            f"{f05['mean'] * 100:6.2f}% [{f05['ci95'][0] * 100:5.2f}, {f05['ci95'][1] * 100:5.2f}]  "
            f"{f30['mean'] * 100:6.2f}% [{f30['ci95'][0] * 100:5.2f}, {f30['ci95'][1] * 100:5.2f}]  "
            f"{avg_count:9.2f}"
        )
        
    print("-" * 110)
    print("\nCeiling statistical potential:")
    sig = eval_results["significance"]["0.5"]["ceil_vs_refined"]
    print(f"  Ceiling vs Refined @0.5s: mean_diff={sig['mean_diff']*100:+.2f}%  p={sig['p']:.2e}")
    sig = eval_results["significance"]["3.0"]["ceil_vs_refined"]
    print(f"  Ceiling vs Refined @3.0s: mean_diff={sig['mean_diff']*100:+.2f}%  p={sig['p']:.2e}")
    
    print("\nStock Foote performance:")
    sig = eval_results["significance"]["3.0"]["stock_vs_refined"]
    print(f"  Stock Foote vs Refined @3.0s: mean_diff={sig['mean_diff']*100:+.2f}%  p={sig['p']:.2e}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 1 Diagnostic Checks.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="windowed")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=200)
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

    # 2. Run Diagnostics on both splits
    print("\nRunning diagnostics on Held-back Fold...")
    heldback_results = run_diagnostics(heldback_tracks, args.mode, args.window, args.n_bootstrap)
    
    print("\nRunning diagnostics on Inner Dev Fold...")
    dev_results = run_diagnostics(dev_tracks, args.mode, args.window, args.n_bootstrap)

    # 3. Print Reports
    print_diag_report("Inner Dev Fold (Tuning/Train)", dev_results)
    print_diag_report("Held-back Fold (Generalization/Test)", heldback_results)

    result = {
        "mode": args.mode,
        "window": args.window,
        "inner_dev": {
            "n_evaluated": len(dev_tracks),
            "aggregates": dev_results["aggregates"],
            "significance": dev_results["significance"],
        },
        "held_back": {
            "n_evaluated": len(heldback_tracks),
            "aggregates": heldback_results["aggregates"],
            "significance": heldback_results["significance"],
        }
    }

    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"\nwrote {args.json_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
