#!/usr/bin/env python3
"""Independent verification of agy's Phase 1e hybrid RF+DP @0.5s claim.

Question: is the reported F1@0.5s win (refined 5.37 -> hybrid 9.08, p=0.0194) robust,
or an artifact of the single seed-42 held-back fold that was inspected across 1b..1e?

Method: import agy's EXACT pipeline functions (isolating the split as the only variable),
FREEZE agy's reported best_params (removing the HPO forking-path), and evaluate under:
  (1) a reproduction of agy's seed-42 80/20 held-back fold (sanity: should ~match 9.08/5.37), and
  (2) honest K-fold CV (RF retrained per fold, every track tested exactly once) across many seeds.
If the @0.5 win survives (2), it is robust; if it only appears in (1), contamination is confirmed.

This script lives in Claude's worktree; it reads agy's read-only worktree only to reuse identical code.
"""
from __future__ import annotations
import sys, json
from pathlib import Path
import numpy as np
from scipy.stats import wilcoxon

AGY_TOOLS = "/Users/rlupi/src/deep-cuts-agy/tools"
sys.path.insert(0, AGY_TOOLS)
from sklearn.ensemble import RandomForestClassifier
from evaluate_salami_phase0 import (
    DEFAULT_DB_PATH, DEFAULT_VALIDATION_SPLIT,
    _prepare_pass_for_mode, _score_boundaries, _shift_predictions_for_mode, load_track,
)
from evaluate_salami_boundaries import JAMS_DIR, load_onset_map, parse_jams_boundaries_and_labels
from evaluate_salami_phase1a import load_high_res_features
from evaluate_salami_phase1c import extract_track_candidates_and_features
from evaluate_salami_phase1e import run_hybrid_dp_decoder

# agy's reported Phase 1e best_params (tuned on the seed-42 dev fold) — frozen here.
FROZEN = dict(lambda_val=-3.4228, target_dur=15.28, dur_sigma=0.4446, weight_dur=0.3498, min_gap=5.0)
MODE, WINDOW = "windowed", 90.0
RF_KW = dict(n_estimators=100, max_depth=6, min_samples_leaf=4, random_state=42)


def load_all_tracks():
    entries = json.load(open(DEFAULT_VALIDATION_SPLIT))
    onset_map = load_onset_map()
    tracks = []
    for e in entries:
        try:
            sid = e["salami_id"]
            t = load_track(str(e["db_id"]), DEFAULT_DB_PATH)
            t["salami_id"] = sid; t["path"] = e.get("path")
            passes = parse_jams_boundaries_and_labels(
                JAMS_DIR / f"SALAMI_{sid}.jams", onset_map.get(sid, 0.0), t["duration"])
            if not passes:
                continue
            t["passes"] = passes
            t = load_high_res_features(t, DEFAULT_DB_PATH)
            cands, feats, targets = extract_track_candidates_and_features(t, MODE, WINDOW)
            t["candidates"], t["features"], t["targets"] = cands, feats, targets
            t["n_passes"] = len(passes)
            tracks.append(t)
        except Exception:
            continue
    return tracks


def fit_rf(train_tracks):
    X, y = [], []
    for t in train_tracks:
        if t["features"].size > 0:
            X.append(t["features"]); y.append(t["targets"])
    clf = RandomForestClassifier(**RF_KW)
    clf.fit(np.vstack(X), np.concatenate(y))
    return clf


def score_track(track, clf, tol):
    """Return (hybrid_f1, refined_f1) averaged over the track's annotator passes, or None."""
    if track["candidates"] and track["features"].size > 0:
        track["probs"] = clf.predict_proba(track["features"])[:, 1]
    else:
        track["probs"] = np.array([])
    duration = float(track["duration"])
    pred = run_hybrid_dp_decoder(track, track["candidates"], track["probs"],
                                 FROZEN["lambda_val"], FROZEN["target_dur"], FROZEN["dur_sigma"],
                                 FROZEN["weight_dur"], FROZEN["min_gap"], MODE, WINDOW)
    refined = _shift_predictions_for_mode(track["refined_boundaries"], MODE, duration, WINDOW)
    hs, rs = [], []
    for pass_info in track["passes"]:
        prepared, eval_dur, _ = _prepare_pass_for_mode(pass_info, MODE, duration, WINDOW)
        if prepared["segments"]:
            hs.append(_score_boundaries(prepared["boundaries"], pred, eval_dur, tol)["f1"])
            rs.append(_score_boundaries(prepared["boundaries"], refined, eval_dur, tol)["f1"])
    if hs:
        return float(np.mean(hs)), float(np.mean(rs))
    return None


def paired(diffs):
    diffs = np.asarray(diffs, float)
    nz = diffs[np.abs(diffs) > 1e-12]
    if nz.size == 0:
        return 1.0
    return float(wilcoxon(diffs)[1])


def summarize(name, H, R, tol):
    H, R = np.array(H), np.array(R)
    d = H - R
    print(f"  [{name}] N={len(H):3d}  refined={R.mean()*100:5.2f}%  hybrid={H.mean()*100:5.2f}%  "
          f"mean_diff={d.mean()*100:+5.2f}%  win_rate={(d>0).mean()*100:4.1f}%  p={paired(d):.4f}")
    return d.mean()


def repro_seed42(tracks, tol):
    # agy's exact split: np.random.default_rng(42).shuffle, 80/20
    rng = np.random.default_rng(42)
    sh = list(tracks); rng.shuffle(sh)
    k = int(len(sh) * 0.8)
    dev, held = sh[:k], sh[k:]
    clf = fit_rf(dev)
    H, R = [], []
    for t in held:
        r = score_track(t, clf, tol)
        if r:
            H.append(r[0]); R.append(r[1])
    return H, R


def kfold(tracks, kfolds, seed, tol):
    rng = np.random.default_rng(seed)
    idx = np.arange(len(tracks)); rng.shuffle(idx)
    folds = np.array_split(idx, kfolds)
    H, R = [], []
    for fi in range(kfolds):
        test_ix = set(folds[fi].tolist())
        train = [tracks[i] for i in range(len(tracks)) if i not in test_ix]
        clf = fit_rf(train)
        for i in folds[fi]:
            r = score_track(tracks[i], clf, tol)
            if r:
                H.append(r[0]); R.append(r[1])
    return H, R


def main():
    print("Loading tracks (agy's exact pipeline, features computed once)...")
    tracks = load_all_tracks()
    print(f"Loaded {len(tracks)} evaluable tracks "
          f"({sum(t['n_passes'] >= 2 for t in tracks)} dual-annotator).\n")

    for tol in (0.5, 3.0):
        print(f"===== tolerance +/-{tol}s =====")
        # (1) Sanity: reproduce agy's seed-42 held-back fold with frozen params
        H, R = repro_seed42(tracks, tol)
        summarize("repro seed-42 held-back (agy's fold)", H, R, tol)
        # (2) Honest 5-fold CV across seeds
        means = []
        for seed in range(8):
            H, R = kfold(tracks, 5, seed, tol)
            means.append(summarize(f"5-fold CV seed={seed}", H, R, tol))
        print(f"  --> CV mean_diff across 8 seeds: {np.mean(means)*100:+.2f}%  "
              f"(min {np.min(means)*100:+.2f}, max {np.max(means)*100:+.2f})\n")


if __name__ == "__main__":
    main()
