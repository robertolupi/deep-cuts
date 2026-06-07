#!/usr/bin/env python3
import os
import sys
import json
import sqlite3
from pathlib import Path

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from evaluate_salami_boundaries import (  # noqa: E402
    load_onset_map,
    parse_jams_boundaries_and_labels,
    evaluate_boundaries,
    DB_PATH,
    VAL_TRACKS_PATH,
    JAMS_DIR,
)

def beat_grid(duration, bpm, beats_per_unit=1):
    if not bpm or bpm <= 0:
        return []
    period = (60.0 / bpm) * beats_per_unit
    if period <= 0:
        return []
    grid = []
    t = period
    while t < duration:
        grid.append(t)
        t += period
    return grid

def novelty_peaks(waveform_json, duration, min_prominence=0.0):
    if not waveform_json:
        return []
    try:
        env = json.loads(waveform_json)
    except Exception:
        return []
    n = len(env)
    if n < 3:
        return []
    nov = [abs(env[i] - env[i - 1]) for i in range(1, n)]
    peaks = []
    for i in range(1, len(nov) - 1):
        if nov[i] >= nov[i - 1] and nov[i] > nov[i + 1] and nov[i] > min_prominence:
            t = (i + 1) * (duration / n)
            peaks.append(t)
    return peaks

def snap(boundaries, candidates, window):
    if not candidates:
        return list(boundaries)
    out = []
    for b in boundaries:
        best = None
        best_d = window
        for c in candidates:
            d = abs(b - c)
            if d <= best_d:
                best = c
                best_d = d
        out.append(best if best is not None else b)
    return sorted(set(out))

def merge_short(labels, duration, min_len):
    n = len(labels)
    bin_dur = duration / n
    merged = list(labels)
    changed = True
    while changed:
        changed = False
        runs = []
        s = 0
        for i in range(1, n + 1):
            if i == n or merged[i] != merged[s]:
                runs.append([merged[s], s, i])
                s = i
        for ri, (lbl, a, b) in enumerate(runs):
            run_len = (b - a) * bin_dur
            if run_len < min_len and len(runs) > 1:
                if ri == 0:
                    new_lbl = runs[ri + 1][0]
                elif ri == len(runs) - 1:
                    new_lbl = runs[ri - 1][0]
                else:
                    prev_len = (runs[ri - 1][2] - runs[ri - 1][1])
                    next_len = (runs[ri + 1][2] - runs[ri + 1][1])
                    new_lbl = runs[ri - 1][0] if prev_len >= next_len else runs[ri + 1][0]
                for k in range(a, b):
                    merged[k] = new_lbl
                changed = True
                break
    bounds = []
    for i in range(1, n):
        if merged[i - 1] != merged[i]:
            bounds.append(i * bin_dur)
    return bounds

def baseline_boundaries(labels, duration):
    n = len(labels)
    bin_dur = duration / n
    return [i * bin_dur for i in range(1, n) if labels[i - 1] != labels[i]]

def ranked_novelty_peaks(waveform_json, duration):
    if not waveform_json:
        return []
    try:
        env = json.loads(waveform_json)
    except Exception:
        return []
    n = len(env)
    if n < 3:
        return []
    nov = [abs(env[i] - env[i - 1]) for i in range(1, n)]
    out = []
    for i in range(1, len(nov) - 1):
        if nov[i] >= nov[i - 1] and nov[i] > nov[i + 1]:
            out.append(((i + 1) * (duration / n), nov[i]))
    out.sort(key=lambda x: -x[1])
    return out

def augment_with_peaks(base, ranked_peaks, n_add, min_gap):
    bounds = list(base)
    added = 0
    for t, _ in ranked_peaks:
        if added >= n_add:
            break
        if all(abs(t - b) >= min_gap for b in bounds):
            bounds.append(t)
            added += 1
    return sorted(bounds)

def main():
    with open(VAL_TRACKS_PATH) as f:
        val_tracks = json.load(f)
    onset_map = load_onset_map()
    con = sqlite3.connect(DB_PATH)
    cur = con.cursor()

    variants = {}
    n_eval = 0
    gt_bnd_total = 0.0
    base_bnd_total = 0.0

    def acc(name, f05, f30, nb):
        v = variants.setdefault(name, {"f05": 0.0, "f30": 0.0, "nb": 0.0})
        v["f05"] += f05
        v["f30"] += f30
        v["nb"] += nb

    for track in val_tracks:
        cur.execute(
            "SELECT duration_seconds, bpm, waveform_data, sax_alignment_segments "
            "FROM tracks WHERE id = ?",
            (track["db_id"],),
        )
        row = cur.fetchone()
        if not row or not row[0] or not row[3]:
            continue
        duration, bpm, waveform_json, segs_str = row
        labels = segs_str.split(",")
        if len(labels) != 16:
            continue

        jams_path = JAMS_DIR / f"SALAMI_{track['salami_id']}.jams"
        offset = onset_map.get(track['salami_id'], 0.0)
        gt_passes = parse_jams_boundaries_and_labels(jams_path, offset, duration)
        if not gt_passes:
            continue

        base = baseline_boundaries(labels, duration)

        beats = beat_grid(duration, bpm, 1)
        bars = beat_grid(duration, bpm, 4)
        peaks = novelty_peaks(waveform_json, duration)
        ranked = ranked_novelty_peaks(waveform_json, duration)

        cand = {
            "baseline": base,
            "merge_8s": merge_short(labels, duration, 8.0),
            "merge_12s": merge_short(labels, duration, 12.0),
            "beat_snap_1.0": snap(base, beats, 1.0),
            "bar_snap_2.0": snap(base, bars, 2.0),
            "bar_snap_4.0": snap(base, bars, 4.0),
            "novelty_snap_3.0": snap(base, peaks, 3.0),
            "novelty_snap_5.0": snap(base, peaks, 5.0),
            "merge12+bar_snap_2.0": snap(merge_short(labels, duration, 12.0), bars, 2.0),
            "merge12+novelty_5.0": snap(merge_short(labels, duration, 12.0), peaks, 5.0),
            "augment+4peaks_8s": augment_with_peaks(base, ranked, 4, 8.0),
            "augment+8peaks_8s": augment_with_peaks(base, ranked, 8, 8.0),
            "augment+8peaks_5s": augment_with_peaks(base, ranked, 8, 5.0),
            "aug8+novelty_snap5": snap(augment_with_peaks(base, ranked, 8, 8.0), peaks, 5.0),
        }

        gt_n = sum(len(p["boundaries"]) for p in gt_passes) / len(gt_passes)
        gt_bnd_total += gt_n
        base_bnd_total += len(base)

        for name, bounds in cand.items():
            sp05 = sp30 = 0.0
            for p in gt_passes:
                _, _, f05 = evaluate_boundaries(p["boundaries"], bounds, duration, 0.5)
                _, _, f30 = evaluate_boundaries(p["boundaries"], bounds, duration, 3.0)
                sp05 += f05
                sp30 += f30
            k = len(gt_passes)
            acc(name, sp05 / k, sp30 / k, len(bounds))

        sp05 = sp30 = 0.0
        onb = 0.0
        for p in gt_passes:
            ob = snap(base, p["boundaries"], 5.0)
            _, _, f05 = evaluate_boundaries(p["boundaries"], ob, duration, 0.5)
            _, _, f30 = evaluate_boundaries(p["boundaries"], ob, duration, 3.0)
            sp05 += f05
            sp30 += f30
            onb += len(ob)
        k = len(gt_passes)
        acc("ORACLE_snap_gt_5s", sp05 / k, sp30 / k, onb / k)

        n_eval += 1

    con.close()

    if n_eval == 0:
        print("No tracks evaluated.")
        return

    print(f"\nValidation refinement experiment  (N = {n_eval} tracks)")
    print(f"Avg GT boundaries/track:       {gt_bnd_total / n_eval:.2f}")
    print(f"Avg baseline boundaries/track: {base_bnd_total / n_eval:.2f}")
    print("=" * 78)
    print(f"{'variant':<26}{'F1@0.5s':>10}{'F1@3.0s':>10}{'avg #bnd':>12}")
    print("-" * 78)
    order = [
        "baseline",
        "merge_8s",
        "merge_12s",
        "beat_snap_1.0",
        "bar_snap_2.0",
        "bar_snap_4.0",
        "novelty_snap_3.0",
        "novelty_snap_5.0",
        "merge12+bar_snap_2.0",
        "merge12+novelty_5.0",
        "augment+4peaks_8s",
        "augment+8peaks_8s",
        "augment+8peaks_5s",
        "aug8+novelty_snap5",
        "ORACLE_snap_gt_5s",
    ]
    for name in order:
        v = variants[name]
        print(f"{name:<26}{v['f05'] / n_eval * 100:>9.2f}%{v['f30'] / n_eval * 100:>9.2f}%"
              f"{v['nb'] / n_eval:>12.2f}")
    print("=" * 78)

if __name__ == "__main__":
    main()
