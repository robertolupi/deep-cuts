#!/usr/bin/env python3
"""Phase 1a Evaluation: Novelty Source Swap.

Evaluates boundary refinement using high-resolution cached features:
1. Onset-only: augment 16-bin baseline with onset strengths.
2. Chroma-only: augment 16-bin baseline with chroma frame-to-frame cosine novelty.
3. Fused (Onset-snapped Chroma): snap chroma novelty peaks to nearest onsets.

Runs in windowed mode on the validation split, reporting P/R/F1, bootstrap CIs,
and Wilcoxon significance versus the Phase 0 refined baseline.
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

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
sys.path.insert(0, str(SCRIPT_DIR))

# Import helpers from Phase 0
from evaluate_salami_phase0 import (
    DEFAULT_DB_PATH,
    DEFAULT_VALIDATION_SPLIT,
    Tolerances,
    _aggregate_tracks,
    _aligned_human_subset,
    _connect,
    _intervals,
    _load_boundaries_json,
    _parse_split_entry,
    _prepare_pass_for_mode,
    _score_boundaries,
    _score_human,
    _shift_predictions_for_mode,
    _significance,
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

warnings.filterwarnings("ignore", category=UserWarning)

# Define our new variant keys
VARIANTS = ["baseline", "refined", "onset_only", "chroma_only", "snapped", "oracle", "human"]


def load_high_res_features(track: dict, db_path: Path) -> dict:
    """Load high-resolution onsets and chroma series from DB and sidecar."""
    db_id = track["db_id"]
    duration = track["duration"]
    path = track.get("path")

    # 1. Load Onsets (try sidecar first, then fall back to DB)
    onsets_data = None
    sidecar_path = Path(path + ".dc.json") if path else None
    
    if sidecar_path and sidecar_path.exists():
        try:
            with open(sidecar_path, "r", encoding="utf-8") as f:
                sidecar_data = json.load(f)
            if "onsets" in sidecar_data:
                onsets_data = json.loads(sidecar_data["onsets"])
        except Exception:
            pass

    if not onsets_data:
        # DB fallback
        with _connect(db_path) as con:
            row = con.execute("SELECT onsets FROM tracks WHERE id = ?", (db_id,)).fetchone()
        if row and row[0]:
            try:
                onsets_data = json.loads(row[0])
            except Exception:
                pass

    # 2. Load Chroma Series from sidecar
    chroma_series = []
    chroma_times = []
    if sidecar_path and sidecar_path.exists():
        try:
            with open(sidecar_path, "r", encoding="utf-8") as f:
                sidecar_data = json.load(f)
            dsp = sidecar_data.get("dsp_features", {})
            chroma_series = dsp.get("chroma_series", [])
            chroma_times = dsp.get("chroma_times", [])
        except Exception:
            pass

    track["onsets"] = onsets_data or {"times": [], "strengths": []}
    track["chroma_series"] = chroma_series
    track["chroma_times"] = chroma_times
    return track


def compute_chroma_novelty(chroma_series: list[list[float]]) -> list[float]:
    """Compute frame-to-frame cosine distance novelty curve."""
    if not chroma_series or len(chroma_series) < 2:
        return []
    X = np.array(chroma_series)
    # L2 normalize rows
    norms = np.linalg.norm(X, axis=1, keepdims=True)
    norms[norms < 1e-8] = 1.0
    X_norm = X / norms
    
    # Cosine similarities
    sims = np.sum(X_norm[:-1] * X_norm[1:], axis=1)
    dist = 1.0 - sims
    return [0.0] + dist.tolist()


def get_chroma_peaks(times: list[float], novelty: list[float]) -> list[tuple[float, float]]:
    """Simple local maximum peak picker over chroma novelty."""
    if not novelty or len(novelty) != len(times):
        return []
    peaks = []
    for i in range(1, len(novelty) - 1):
        if novelty[i] >= novelty[i - 1] and novelty[i] > novelty[i + 1]:
            peaks.append((times[i], novelty[i]))
    # Sort by strength descending
    peaks.sort(key=lambda x: -x[1])
    return peaks


def _score_one_track_phase1a(
    track: dict,
    mode: Literal["legacy", "windowed"],
    window: float,
    tolerances: Tolerances,
    n_add: int = 8,
    min_gap: float = 5.0,
) -> dict:
    duration = float(track["duration"])
    offset = calculate_crop_offset(duration, window)
    
    prepared_passes = []
    eval_duration = duration
    for pass_info in track["passes"]:
        prepared, eval_duration, offset = _prepare_pass_for_mode(pass_info, mode, duration, window)
        if prepared["segments"]:
            prepared_passes.append(prepared)
    if not prepared_passes:
        raise RuntimeError("no annotations inside window")

    # Shifted/filtered baseline and refined (128-pt) boundaries
    base = _shift_predictions_for_mode(track["baseline_boundaries"], mode, duration, window)
    refined = _shift_predictions_for_mode(track["refined_boundaries"], mode, duration, window)

    # Compute high-resolution candidates
    # Onsets
    onset_times = track["onsets"].get("times", [])
    onset_strengths = track["onsets"].get("strengths", [])
    onset_peaks_abs = to_absolute_time(onset_times, offset)
    ranked_onset_peaks = sorted(zip(onset_peaks_abs, onset_strengths), key=lambda x: -x[1])
    onset_only_full = augment_with_peaks(track["baseline_boundaries"], ranked_onset_peaks, n_add, min_gap)
    onset_only = _shift_predictions_for_mode(onset_only_full, mode, duration, window)

    # Chroma
    chroma_series = track["chroma_series"]
    chroma_times = track["chroma_times"]
    chroma_novelty = compute_chroma_novelty(chroma_series)
    chroma_peaks_crop = get_chroma_peaks(chroma_times, chroma_novelty)
    chroma_peaks_abs = to_absolute_time([p[0] for p in chroma_peaks_crop], offset)
    ranked_chroma_peaks = sorted(zip(chroma_peaks_abs, [p[1] for p in chroma_peaks_crop]), key=lambda x: -x[1])
    chroma_only_full = augment_with_peaks(track["baseline_boundaries"], ranked_chroma_peaks, n_add, min_gap)
    chroma_only = _shift_predictions_for_mode(chroma_only_full, mode, duration, window)

    # Snapped (Chroma novelty peaks snapped to nearest onset peaks)
    # Perform snapping in absolute time
    snapped_peaks_abs = []
    if onset_peaks_abs:
        snapped_peaks_abs = snap(chroma_peaks_abs, onset_peaks_abs, window=0.5)
    else:
        snapped_peaks_abs = chroma_peaks_abs
    ranked_snapped_peaks = [(t, 1.0) for t in snapped_peaks_abs]
    snapped_full = augment_with_peaks(track["baseline_boundaries"], ranked_snapped_peaks, n_add, min_gap)
    snapped = _shift_predictions_for_mode(snapped_full, mode, duration, window)

    # Build candidates dict for scoring
    cand = {
        "baseline": base,
        "refined": refined,
        "onset_only": onset_only,
        "chroma_only": chroma_only,
        "snapped": snapped,
    }

    per_pass = {v: [] for v in VARIANTS if v not in ("oracle", "human")}
    pairwise = []
    for prepared in prepared_passes:
        _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
        cand["oracle"] = oracle_bounds

        for variant in per_pass.keys():
            per_pass[variant].append(_score_variant_for_pass(prepared["boundaries"], cand[variant], eval_duration, tolerances))
            
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
        summary["scores"][variant] = {str(t): _mean_score(variant_scores, t) for t in tolerances}
    
    # Oracle scoring
    oracle_scores = []
    for prepared in prepared_passes:
        _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
        oracle_scores.append(_score_variant_for_pass(prepared["boundaries"], oracle_bounds, eval_duration, tolerances))
    summary["scores"]["oracle"] = {str(t): _mean_score(oracle_scores, t) for t in tolerances}

    human = _score_human(prepared_passes, eval_duration, tolerances)
    if human is not None:
        summary["scores"]["human"] = human
    return summary


def _score_variant_for_pass(
    ref_boundaries: list[float],
    est_boundaries: list[float],
    duration: float,
    tolerances: Tolerances,
) -> dict[str, BoundaryScores]:
    return {str(t): _score_boundaries(ref_boundaries, est_boundaries, duration, t) for t in tolerances}


def _mean_score(scores: list[dict[str, BoundaryScores]], tolerance: float) -> BoundaryScores:
    key = str(tolerance)
    if not scores:
        return {"precision": 0.0, "recall": 0.0, "f1": 0.0}
    return {
        metric: float(np.mean([s[key][metric] for s in scores]))
        for metric in ("precision", "recall", "f1")
    }


def _aggregate_tracks_phase1a(tracks: list[dict], tolerances: Tolerances, n_bootstrap: int) -> dict:
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


def _significance_phase1a(tracks: list[dict], tolerances: Tolerances) -> dict:
    out = {}
    for tol in tolerances:
        key = str(tol)
        
        # Test each variant against the refined baseline
        refined = np.array([t["scores"]["refined"][key]["f1"] for t in tracks])
        
        out[key] = {}
        for var in ("onset_only", "chroma_only", "snapped"):
            scores = np.array([t["scores"][var][key]["f1"] for t in tracks])
            out[key][f"{var}_vs_refined"] = paired_wilcoxon_phase1a(scores, refined)
            
            oracle = np.array([t["scores"]["oracle"][key]["f1"] for t in tracks])
            out[key][f"{var}_vs_oracle"] = paired_wilcoxon_phase1a(scores, oracle)
            
    return out


def paired_wilcoxon_phase1a(a: np.ndarray, b: np.ndarray) -> dict:
    """Wilcoxon signed-rank test on paired differences."""
    from scipy.stats import wilcoxon
    aa = np.asarray(a, dtype=float)
    bb = np.asarray(b, dtype=float)
    mask = np.isfinite(aa) & np.isfinite(bb)
    aa, bb = aa[mask], bb[mask]
    diff = aa - bb
    nonzero = diff[np.abs(diff) > 1e-12]
    if aa.size == 0:
        return {"stat": None, "p": None, "mean_diff": 0.0, "n": 0}
    if nonzero.size == 0:
        return {"stat": 0.0, "p": 1.0, "mean_diff": 0.0, "n": int(aa.size)}
    res = wilcoxon(aa, bb, zero_method="wilcox", alternative="two-sided")
    return {
        "stat": float(res.statistic),
        "p": float(res.pvalue),
        "mean_diff": float(np.mean(diff)),
        "n": int(aa.size),
    }


def evaluate_split_phase1a(
    track_ids: list[str],
    db_path: Path,
    mode: Literal["legacy", "windowed"] = "windowed",
    window: float = 90.0,
    n_add: int = 8,
    min_gap: float = 5.0,
) -> dict:
    entries = [_parse_split_entry(item) for item in track_ids]
    onset_map = load_onset_map()
    evaluated = []
    skipped = []
    
    for entry in entries:
        try:
            track = load_track(str(entry.db_id), db_path)
            track["salami_id"] = entry.salami_id
            track["path"] = entry.path
            
            # Load GT
            jams_path = JAMS_DIR / f"SALAMI_{entry.salami_id}.jams"
            offset = onset_map.get(entry.salami_id, 0.0)
            passes = parse_jams_boundaries_and_labels(jams_path, offset, track["duration"])
            if not passes:
                skipped.append({"db_id": entry.db_id, "salami_id": entry.salami_id, "reason": "no JAMS"})
                continue
            track["passes"] = passes
            
            # Load Onsets and Chroma
            track = load_high_res_features(track, db_path)
            
            # Score
            res = _score_one_track_phase1a(track, mode, window, (0.5, 3.0), n_add, min_gap)
            evaluated.append(res)
        except Exception as exc:
            skipped.append({"db_id": entry.db_id, "salami_id": entry.salami_id, "reason": str(exc)})

    if not evaluated:
        raise RuntimeError("no tracks could be evaluated")

    return {
        "mode": mode,
        "window": window,
        "n_requested": len(entries),
        "n_evaluated": len(evaluated),
        "n_skipped": len(skipped),
        "skipped": skipped,
        "per_track": evaluated,
        "aggregates": _aggregate_tracks_phase1a(evaluated, (0.5, 3.0), n_bootstrap=0),
        "significance": _significance_phase1a(evaluated, (0.5, 3.0)),
    }


def _print_summary_phase1a(result: dict) -> None:
    print(f"\nPhase 1a SALAMI evaluation  mode={result['mode']}  N={result['n_evaluated']}  skipped={result['n_skipped']}")
    print("-" * 110)
    aggregates = result["aligned_human_subset"]["aggregates"] or result["aggregates"]
    if result["aligned_human_subset"]["aggregates"]:
        print(f"aligned dual-annotator subset N={result['aligned_human_subset']['n']}")
    
    print(f"{'variant':<22}{'F1@0.5s':>18}{'F1@3.0s':>18}{'avg #bnd':>12}")
    print("-" * 110)
    
    for variant in VARIANTS:
        if variant not in aggregates:
            continue
        f05 = aggregates[variant]["0.5"]["f1"]
        f30 = aggregates[variant]["3.0"]["f1"]
        
        # Calculate avg boundary count
        counts = [t["boundary_counts"].get(variant, 0) for t in result["per_track"]]
        avg_count = np.mean(counts) if counts else 0.0
        
        print(
            f"{variant:<22}"
            f"{f05['mean'] * 100:6.2f}% [{f05['ci95'][0] * 100:5.2f}, {f05['ci95'][1] * 100:5.2f}]  "
            f"{f30['mean'] * 100:6.2f}% [{f30['ci95'][0] * 100:5.2f}, {f30['ci95'][1] * 100:5.2f}]  "
            f"{avg_count:9.2f}"
        )
    
    pw = aggregates["pairwise_label_f1"]
    print("-" * 110)
    print(f"pairwise label F1: {pw['mean'] * 100:.2f}%  CI=[{pw['ci95'][0] * 100:.2f}, {pw['ci95'][1] * 100:.2f}]")
    
    significance = result["aligned_human_subset"]["significance"] or result["significance"]
    print("\nSignificance vs Refined F1@0.5s:")
    for var in ("onset_only", "chroma_only", "snapped"):
        sig = significance["0.5"][f"{var}_vs_refined"]
        print(f"  {var:<12} vs refined: p={sig['p']:.2e}  mean_diff={sig['mean_diff']*100:+.2f}%")
        
    print("\nSignificance vs Refined F1@3.0s:")
    for var in ("onset_only", "chroma_only", "snapped"):
        sig = significance["3.0"][f"{var}_vs_refined"]
        print(f"  {var:<12} vs refined: p={sig['p']:.2e}  mean_diff={sig['mean_diff']*100:+.2f}%")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 1a SALAMI boundary evaluation.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="windowed")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=2000)
    parser.add_argument("--n-add", type=int, default=8, help="number of peaks to add")
    parser.add_argument("--min-gap", type=float, default=5.0, help="min gap between added peaks")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    # Reject holdout split check
    if Path(args.split_json).resolve() == DEFAULT_VALIDATION_SPLIT.parent.joinpath("holdout_tracks.json").resolve():
        print("error: holdout split is protected; evaluation rejected", file=sys.stderr)
        return 2

    with open(args.split_json, "r", encoding="utf-8") as f:
        split_entries = json.load(f)

    try:
        result = evaluate_split_phase1a(
            split_entries,
            args.db_path,
            mode=args.mode,
            window=args.window,
            n_add=args.n_add,
            min_gap=args.min_gap,
        )
        
        # Apply bootstrap aggregation
        result["aggregates"] = _aggregate_tracks_phase1a(result["per_track"], (0.5, 3.0), args.n_bootstrap)
        aligned = _aligned_human_subset(result["per_track"])
        result["aligned_human_subset"] = {
            "n": len(aligned),
            "aggregates": _aggregate_tracks_phase1a(aligned, (0.5, 3.0), args.n_bootstrap) if aligned else {},
            "significance": _significance_phase1a(aligned, (0.5, 3.0)) if aligned else {},
        }
        
    except Exception as exc:
        import traceback
        traceback.print_exc()
        print(f"error: {exc}", file=sys.stderr)
        return 2
        
    _print_summary_phase1a(result)
    
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"wrote {args.json_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
