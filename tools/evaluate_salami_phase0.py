#!/usr/bin/env python3
"""Phase 0 Evaluation Contract: reproducible SALAMI evaluation.

Implements the frozen roadmap contract:
- dual-mode execution (legacy full-track vs corrected central-window eval)
- track-dependent crop offset alignment
- mir_eval P/R/F1 boundary metrics and pairwise clustering regression guard
- bootstrap confidence intervals and paired Wilcoxon tests
- legacy golden-number regression checks

Legacy mode exists to reproduce archived full-track numbers. Windowed mode is the
correct Phase 1 anchor for central-90s cached onset/chroma features; it recomputes
baseline/oracle/human ceilings on the same central crop.
"""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
import subprocess
import sys
import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Literal, TypedDict

import mir_eval
import numpy as np
from scipy.stats import wilcoxon

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
SESSION_DIR = REPO_ROOT / "doc/collab/sessions/2026-06-07-salami-eval-design"
DEFAULT_VALIDATION_SPLIT = SESSION_DIR / "validation_tracks.json"
DEFAULT_HOLDOUT_SPLIT = SESSION_DIR / "holdout_tracks.json"
DEFAULT_DB_PATH = Path(os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"))

sys.path.insert(0, str(SCRIPT_DIR))
from evaluate_salami_boundaries import (  # noqa: E402
    JAMS_DIR,
    evaluate_pairwise_clustering,
    load_onset_map,
    parse_jams_boundaries_and_labels,
    project_jams_to_16_bins,
)
from refine_salami_boundaries import (  # noqa: E402
    augment_with_peaks,
    baseline_boundaries,
    ranked_novelty_peaks,
)

warnings.filterwarnings("ignore", category=UserWarning)

Tolerances = tuple[float, ...]
EvalMode = Literal["legacy", "windowed"]
Variant = Literal["baseline", "refined", "oracle", "human"]


class BoundaryScores(TypedDict):
    precision: float
    recall: float
    f1: float


class TrackLoadError(RuntimeError):
    """Raised when a split entry cannot be evaluated."""


@dataclass(frozen=True)
class SplitEntry:
    db_id: int
    salami_id: int
    path: str | None = None


def calculate_crop_offset(duration: float, window: float = 90.0) -> float:
    """Return max(0, duration/2 - window/2), matching the centre crop in dsp.rs."""
    if duration <= 0:
        return 0.0
    if window <= 0:
        raise ValueError("window must be positive")
    return max(0.0, float(duration) / 2.0 - float(window) / 2.0)


def to_absolute_time(times_crop: list[float], offset: float) -> list[float]:
    """Add offset to timestamps from a crop-relative coordinate system."""
    return [float(t) + float(offset) for t in times_crop]


def filter_to_window(times_abs: list[float], start: float, end: float) -> list[float]:
    """Keep boundaries inside [start, end]."""
    lo, hi = float(start), float(end)
    if hi < lo:
        raise ValueError("window end must be >= start")
    return [float(t) for t in times_abs if lo <= float(t) <= hi]


def _intervals(bounds: list[float], duration: float) -> np.ndarray:
    clean = sorted(set([0.0] + [float(x) for x in bounds if 0.0 < float(x) < duration] + [float(duration)]))
    return mir_eval.util.boundaries_to_intervals(np.array(clean, dtype=float))


def _score_boundaries(ref_b: list[float], est_b: list[float], duration: float, tolerance: float) -> BoundaryScores:
    if duration <= 0:
        return {"precision": 0.0, "recall": 0.0, "f1": 0.0}
    try:
        p, r, f = mir_eval.segment.detection(
            _intervals(ref_b, duration),
            _intervals(est_b, duration),
            window=float(tolerance),
            trim=True,
        )
    except Exception:
        p, r, f = 0.0, 0.0, 0.0
    return {"precision": float(p), "recall": float(r), "f1": float(f)}


def score_mireval(
    pred_abs: list[float],
    gt_abs: list[float],
    tolerances: tuple[float, float] = (0.5, 3.0),
) -> dict[str, BoundaryScores]:
    """Run mir_eval.segment.detection for ±0.5s and ±3.0s.

    This public helper infers a duration just beyond the largest boundary for
    simple unit tests. The full evaluator uses the private scorer with the real
    track duration/window.
    """
    duration = max([0.0] + [float(x) for x in pred_abs] + [float(x) for x in gt_abs]) + 1.0
    return {str(t): _score_boundaries(gt_abs, pred_abs, duration, t) for t in tolerances}


def bootstrap_ci(
    scores: np.ndarray,
    n_resamples: int = 2000,
    alpha: float = 0.05,
) -> tuple[float, float, float]:
    """Return (mean, lower, upper) CI by resampling tracks with replacement."""
    arr = np.asarray(scores, dtype=float)
    arr = arr[np.isfinite(arr)]
    if arr.size == 0:
        return 0.0, 0.0, 0.0
    if n_resamples <= 0:
        m = float(np.mean(arr))
        return m, m, m

    rng = np.random.default_rng(0)
    draws = rng.choice(arr, size=(int(n_resamples), arr.size), replace=True).mean(axis=1)
    lower = float(np.quantile(draws, alpha / 2.0))
    upper = float(np.quantile(draws, 1.0 - alpha / 2.0))
    return float(np.mean(arr)), lower, upper


def paired_wilcoxon(a: np.ndarray, b: np.ndarray) -> dict:
    """Wilcoxon signed-rank test on paired per-track scores."""
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


def _parse_split_entry(entry: Any) -> SplitEntry:
    if isinstance(entry, dict):
        return SplitEntry(int(entry["db_id"]), int(entry["salami_id"]), entry.get("path"))
    if isinstance(entry, str) and ":" in entry:
        db_id, salami_id = entry.split(":", 1)
        return SplitEntry(int(db_id), int(salami_id))
    raise ValueError("split entries must be dicts with db_id/salami_id or 'db_id:salami_id' strings")


def _load_boundaries_json(raw: str | None) -> list[float]:
    if not raw:
        return []
    try:
        data = json.loads(raw)
    except Exception:
        return []
    if isinstance(data, dict):
        data = data.get("times", [])
    if not isinstance(data, list):
        return []
    out = []
    for item in data:
        try:
            out.append(float(item))
        except (TypeError, ValueError):
            continue
    return sorted(out)


def _connect(db_path: Path) -> sqlite3.Connection:
    path = Path(db_path).expanduser()
    if not path.exists():
        raise FileNotFoundError(f"database not found: {path}")
    return sqlite3.connect(path)


def load_track(track_id: str, db_path: Path) -> dict:
    """Load one track by DB id.

    Returns a lightweight dictionary with duration and crop-relative/full-track
    model predictions. Full evaluation also attaches JAMS annotations from the
    split entry's salami_id.
    """
    db_id = int(str(track_id).split(":", 1)[0])
    with _connect(db_path) as con:
        row = con.execute(
            """
            SELECT duration_seconds, waveform_data, sax_alignment_segments, sax_alignment_boundaries
            FROM tracks
            WHERE id = ?
            """,
            (db_id,),
        ).fetchone()
    if not row:
        raise TrackLoadError(f"track {db_id} not found in DB")
    duration, waveform_json, sax_segments, refined_json = row
    if not duration:
        raise TrackLoadError(f"track {db_id} has no duration_seconds")
    labels = sax_segments.split(",") if sax_segments else []
    if len(labels) != 16:
        raise TrackLoadError(f"track {db_id} has no 16-bin sax_alignment_segments")
    base = baseline_boundaries(labels, float(duration))
    refined = _load_boundaries_json(refined_json)
    if not refined:
        if not waveform_json:
            raise TrackLoadError(f"track {db_id} has no sax_alignment_boundaries or waveform_data")
        refined = augment_with_peaks(base, ranked_novelty_peaks(waveform_json, float(duration)), 8, 5.0)
    return {
        "db_id": db_id,
        "duration": float(duration),
        "labels": labels,
        "baseline_boundaries": base,
        "refined_boundaries": refined,
        "pred_crop": refined,
    }


def _load_track_for_entry(entry: SplitEntry, db_path: Path, onset_map: dict[int, float]) -> dict:
    track = load_track(str(entry.db_id), db_path)
    track["salami_id"] = entry.salami_id
    track["path"] = entry.path
    passes = parse_jams_boundaries_and_labels(
        JAMS_DIR / f"SALAMI_{entry.salami_id}.jams",
        onset_map.get(entry.salami_id, 0.0),
        track["duration"],
    )
    if not passes:
        raise TrackLoadError(f"SALAMI_{entry.salami_id}.jams missing or has no segment annotations")
    track["passes"] = passes
    return track


def _clip_segments_to_window(segments: list[dict], start: float, end: float) -> list[dict]:
    clipped = []
    for seg in segments:
        s = max(float(seg["start"]), start)
        e = min(float(seg["end"]), end)
        if e > s:
            clipped.append({"start": s - start, "end": e - start, "label": seg["label"]})
    return clipped


def _prepare_pass_for_mode(pass_info: dict, mode: EvalMode, duration: float, window: float) -> tuple[dict, float, float]:
    if mode == "legacy":
        return pass_info, float(duration), 0.0

    offset = calculate_crop_offset(duration, window)
    end = min(float(duration), offset + window)
    clipped_segments = _clip_segments_to_window(pass_info["segments"], offset, end)
    clipped_bounds = [b - offset for b in filter_to_window(pass_info["boundaries"], offset, end) if offset < b < end]
    prepared = {"segments": clipped_segments, "boundaries": clipped_bounds}
    return prepared, end - offset, offset


def _shift_predictions_for_mode(bounds: list[float], mode: EvalMode, duration: float, window: float) -> list[float]:
    if mode == "legacy":
        return list(bounds)
    offset = calculate_crop_offset(duration, window)
    end = min(float(duration), offset + window)
    # Current DB baseline/refined boundaries are full-track. Future crop-relative
    # SSM predictions should be shifted with to_absolute_time() before this filter.
    return [b - offset for b in filter_to_window(bounds, offset, end) if offset < b < end]


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


def _score_human(passes: list[dict], duration: float, tolerances: Tolerances) -> dict[str, BoundaryScores] | None:
    if len(passes) != 2:
        return None
    a, b = passes
    scores_ab = _score_variant_for_pass(a["boundaries"], b["boundaries"], duration, tolerances)
    scores_ba = _score_variant_for_pass(b["boundaries"], a["boundaries"], duration, tolerances)
    out: dict[str, BoundaryScores] = {}
    for tol in tolerances:
        key = str(tol)
        out[key] = {
            metric: float((scores_ab[key][metric] + scores_ba[key][metric]) / 2.0)
            for metric in ("precision", "recall", "f1")
        }
    return out


def _score_one_track(track: dict, mode: EvalMode, window: float, tolerances: Tolerances) -> dict:
    duration = float(track["duration"])
    prepared_passes = []
    eval_duration = duration
    offset = 0.0
    for pass_info in track["passes"]:
        prepared, eval_duration, offset = _prepare_pass_for_mode(pass_info, mode, duration, window)
        if prepared["segments"]:
            prepared_passes.append(prepared)
    if not prepared_passes:
        raise TrackLoadError(f"track {track['db_id']} has no annotations inside {mode} evaluation window")

    base = _shift_predictions_for_mode(track["baseline_boundaries"], mode, duration, window)
    refined = _shift_predictions_for_mode(track["refined_boundaries"], mode, duration, window)

    per_pass = {"baseline": [], "refined": [], "oracle": []}
    pairwise = []
    for prepared in prepared_passes:
        _, oracle_bounds = project_jams_to_16_bins(prepared["segments"], eval_duration)
        per_pass["baseline"].append(_score_variant_for_pass(prepared["boundaries"], base, eval_duration, tolerances))
        per_pass["refined"].append(_score_variant_for_pass(prepared["boundaries"], refined, eval_duration, tolerances))
        per_pass["oracle"].append(_score_variant_for_pass(prepared["boundaries"], oracle_bounds, eval_duration, tolerances))
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
            "baseline": len(base),
            "refined": len(refined),
            "gt_mean": float(np.mean([len(p["boundaries"]) for p in prepared_passes])),
        },
        "pairwise_label_f1": float(np.mean(pairwise)) if pairwise else 0.0,
        "scores": {},
    }
    for variant, variant_scores in per_pass.items():
        summary["scores"][variant] = {str(t): _mean_score(variant_scores, t) for t in tolerances}
    human = _score_human(prepared_passes, eval_duration, tolerances)
    if human is not None:
        summary["scores"]["human"] = human
    return summary


def _aggregate_tracks(tracks: list[dict], tolerances: Tolerances, n_bootstrap: int) -> dict:
    variants = ["baseline", "refined", "oracle", "human"]
    aggregates: dict[str, Any] = {}
    for variant in variants:
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


def _aligned_human_subset(tracks: list[dict]) -> list[dict]:
    """Return tracks that have human-ceiling scores for aligned decompositions."""
    return [t for t in tracks if "human" in t["scores"]]


def _significance(tracks: list[dict], tolerances: Tolerances) -> dict:
    out = {}
    for tol in tolerances:
        key = str(tol)
        refined = np.array([t["scores"]["refined"][key]["f1"] for t in tracks])
        baseline = np.array([t["scores"]["baseline"][key]["f1"] for t in tracks])
        oracle = np.array([t["scores"]["oracle"][key]["f1"] for t in tracks])
        out[key] = {
            "refined_vs_baseline": paired_wilcoxon(refined, baseline),
            "refined_vs_oracle": paired_wilcoxon(refined, oracle),
        }
    return out


def _git_hash() -> str | None:
    try:
        return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True).strip()
    except Exception:
        return None


def evaluate_split(
    track_ids: list[str],
    db_path: Path,
    mode: Literal["legacy", "windowed"] = "windowed",
    window: float = 90.0,
) -> dict:
    """Evaluate split entries in legacy or central-window mode."""
    entries = [_parse_split_entry(item) for item in track_ids]
    onset_map = load_onset_map()
    evaluated = []
    skipped = []
    with _connect(db_path):
        pass
    for entry in entries:
        try:
            track = _load_track_for_entry(entry, db_path, onset_map)
            evaluated.append(_score_one_track(track, mode, window, (0.5, 3.0)))
        except Exception as exc:
            skipped.append({"db_id": entry.db_id, "salami_id": entry.salami_id, "reason": str(exc)})

    if not evaluated:
        raise RuntimeError("no tracks could be evaluated; analysis may still be incomplete")

    return {
        "mode": mode,
        "window": window,
        "n_requested": len(entries),
        "n_evaluated": len(evaluated),
        "n_skipped": len(skipped),
        "skipped": skipped,
        "per_track": evaluated,
        "aggregates": _aggregate_tracks(evaluated, (0.5, 3.0), n_bootstrap=0),
        "significance": _significance(evaluated, (0.5, 3.0)),
    }


GOLDEN_LEGACY_DUAL_ANNOTATOR = {
    "baseline": {"3.0": 0.2182},
    "refined": {"3.0": 0.3326},
    "human": {"3.0": 0.7153},
}


def _assert_golden_numbers(result: dict, tolerance_abs: float = 0.005) -> None:
    if result["mode"] != "legacy":
        return
    errors = []
    ag = result.get("aligned_human_subset", {}).get("aggregates") or result["aggregates"]
    for variant, by_tol in GOLDEN_LEGACY_DUAL_ANNOTATOR.items():
        for tol, expected in by_tol.items():
            if variant not in ag:
                errors.append(f"missing aggregate for {variant}")
                continue
            actual = ag[variant][tol]["f1"]["mean"]
            if abs(actual - expected) > tolerance_abs:
                errors.append(
                    f"{variant} F1@{tol}s expected {expected * 100:.2f}% ±{tolerance_abs * 100:.2f}%, "
                    f"got {actual * 100:.2f}%"
                )
    if errors:
        raise AssertionError("golden-number regression failed:\n" + "\n".join(errors))


def _reject_holdout(split_json: Path, allow_holdout: bool) -> None:
    if allow_holdout:
        return
    if Path(split_json).resolve() == DEFAULT_HOLDOUT_SPLIT.resolve() or "holdout" in Path(split_json).name.lower():
        raise ValueError("holdout split is protected; pass --allow-holdout only for the custodian's frozen run")


def run_phase0(
    split_json: Path,
    db_path: Path,
    mode: Literal["legacy", "windowed"] = "windowed",
    n_bootstrap: int = 2000,
    *,
    allow_holdout: bool = False,
    golden: bool = False,
    window: float = 90.0,
) -> dict:
    """Run Phase 0 on a fixed split.

    The default refuses holdout files so normal prototype/eval paths cannot
    accidentally peek at protected data.
    """
    split_json = Path(split_json)
    _reject_holdout(split_json, allow_holdout)
    with open(split_json, "r", encoding="utf-8") as f:
        split_entries = json.load(f)

    result = evaluate_split(split_entries, db_path, mode=mode, window=window)
    result["aggregates"] = _aggregate_tracks(result["per_track"], (0.5, 3.0), n_bootstrap)
    aligned = _aligned_human_subset(result["per_track"])
    result["aligned_human_subset"] = {
        "n": len(aligned),
        "aggregates": _aggregate_tracks(aligned, (0.5, 3.0), n_bootstrap) if aligned else {},
        "significance": _significance(aligned, (0.5, 3.0)) if aligned else {},
    }
    result["manifest"] = {
        "code_hash": _git_hash(),
        "db_path": str(Path(db_path).expanduser()),
        "split_json": str(split_json),
        "mode": mode,
        "window": window,
        "n_bootstrap": n_bootstrap,
        "label": "validation result" if not allow_holdout else "held-out result",
        "notes": [
            "legacy mode reproduces archived full-track anchors",
            "windowed mode recomputes central-window anchors and is not comparable to archived full-track numbers",
        ],
    }
    if golden:
        _assert_golden_numbers(result)
        result["golden_check"] = {"passed": True, "tolerance_abs": 0.005}
    return result


def _print_summary(result: dict) -> None:
    print(f"\nPhase 0 SALAMI evaluation  mode={result['mode']}  N={result['n_evaluated']}  skipped={result['n_skipped']}")
    print("-" * 86)
    aggregates = result["aligned_human_subset"]["aggregates"] or result["aggregates"]
    if result["aligned_human_subset"]["aggregates"]:
        print(f"aligned dual-annotator subset N={result['aligned_human_subset']['n']}")
    for variant in ("baseline", "refined", "oracle", "human"):
        if variant not in aggregates:
            continue
        print(variant)
        for tol in ("0.5", "3.0"):
            f1 = aggregates[variant][tol]["f1"]
            p = aggregates[variant][tol]["precision"]["mean"]
            r = aggregates[variant][tol]["recall"]["mean"]
            print(
                f"  @{tol}s  P={p * 100:6.2f}%  R={r * 100:6.2f}%  "
                f"F1={f1['mean'] * 100:6.2f}%  CI=[{f1['ci95'][0] * 100:5.2f}, {f1['ci95'][1] * 100:5.2f}]"
            )
    pw = aggregates["pairwise_label_f1"]
    print(f"pairwise label F1: {pw['mean'] * 100:.2f}%  CI=[{pw['ci95'][0] * 100:.2f}, {pw['ci95'][1] * 100:.2f}]")
    significance = result["aligned_human_subset"]["significance"] or result["significance"]
    print("significance (F1 refined vs oracle @3s):", significance["3.0"]["refined_vs_oracle"])
    if result.get("golden_check", {}).get("passed"):
        print("golden-number regression: PASSED")


def main() -> int:
    parser = argparse.ArgumentParser(description="Run Phase 0 SALAMI boundary evaluation.")
    parser.add_argument("--split-json", type=Path, default=DEFAULT_VALIDATION_SPLIT)
    parser.add_argument("--db-path", type=Path, default=DEFAULT_DB_PATH)
    parser.add_argument("--mode", choices=["legacy", "windowed"], default="legacy")
    parser.add_argument("--window", type=float, default=90.0)
    parser.add_argument("--n-bootstrap", type=int, default=2000)
    parser.add_argument("--golden", action="store_true", help="assert archived legacy golden numbers")
    parser.add_argument("--allow-holdout", action="store_true", help="custodian-only: allow holdout split")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    try:
        result = run_phase0(
            args.split_json,
            args.db_path,
            mode=args.mode,
            n_bootstrap=args.n_bootstrap,
            allow_holdout=args.allow_holdout,
            golden=args.golden,
            window=args.window,
        )
    except Exception as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    _print_summary(result)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"wrote {args.json_out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
