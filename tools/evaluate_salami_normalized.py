#!/usr/bin/env python3
import os
import sys
import json
import sqlite3

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from evaluate_salami_boundaries import (  # noqa: E402
    load_onset_map, parse_jams_boundaries_and_labels, evaluate_boundaries,
    project_jams_to_16_bins, DB_PATH, VAL_TRACKS_PATH, JAMS_DIR,
)
from refine_salami_boundaries import (  # noqa: E402
    baseline_boundaries, ranked_novelty_peaks, augment_with_peaks,
)

TOL = 3.0

def sym_boundary_f1(a, b, dur, tol):
    """Order-independent boundary F1 (mean of both directions)."""
    _, _, f_ab = evaluate_boundaries(a, b, dur, tol)
    _, _, f_ba = evaluate_boundaries(b, a, dur, tol)
    return 0.5 * (f_ab + f_ba)

def main():
    val = json.load(open(VAL_TRACKS_PATH))
    onset = load_onset_map()
    con = sqlite3.connect(DB_PATH)
    cur = con.cursor()

    base_s, ref_s, grid_s, human_s, norm = [], [], [], [], []
    n_dual = 0
    n_zero_human = 0

    for t in val:
        cur.execute(
            "SELECT duration_seconds, waveform_data, sax_alignment_segments "
            "FROM tracks WHERE id=?", (t["db_id"],))
        r = cur.fetchone()
        if not r or not r[0] or not r[2]:
            continue
        dur, wf, segs = r
        labels = segs.split(",")
        if len(labels) != 16:
            continue
        passes = parse_jams_boundaries_and_labels(
            JAMS_DIR / f"SALAMI_{t['salami_id']}.jams", onset.get(t["salami_id"], 0.0), dur)
        if len(passes) != 2:  # dual-annotator subset only
            continue
        n_dual += 1

        base = baseline_boundaries(labels, dur)
        refined = augment_with_peaks(base, ranked_novelty_peaks(wf, dur), 8, 5.0)

        mf, bf, gf = [], [], []
        for p in passes:
            _, _, f = evaluate_boundaries(p["boundaries"], refined, dur, TOL); mf.append(f)
            _, _, fb = evaluate_boundaries(p["boundaries"], base, dur, TOL); bf.append(fb)
            _, proj_b = project_jams_to_16_bins(p["segments"], dur)
            _, _, fg = evaluate_boundaries(p["boundaries"], proj_b, dur, TOL); gf.append(fg)
        m = sum(mf) / len(mf)
        h = sym_boundary_f1(passes[0]["boundaries"], passes[1]["boundaries"], dur, TOL)

        ref_s.append(m)
        base_s.append(sum(bf) / len(bf))
        grid_s.append(sum(gf) / len(gf))
        human_s.append(h)
        if h > 0.05:
            norm.append(min(m / h, 1.5))
        else:
            n_zero_human += 1
    con.close()

    def mean(x):
        return 100 * sum(x) / len(x)

    print(f"\nDual-annotator validation subset  N={n_dual}  (tolerance ±{TOL}s)")
    print("-" * 60)
    print(f"  Baseline model F1        : {mean(base_s):6.2f}%")
    print(f"  Refined  model F1        : {mean(ref_s):6.2f}%")
    print(f"  16-bin GRID ceiling      : {mean(grid_s):6.2f}%")
    print(f"  Human consensus ceiling  : {mean(human_s):6.2f}%")
    print("-" * 60)
    print(f"  Refined / GRID ceiling   : {100*sum(ref_s)/sum(grid_s):5.1f}%  (detector saturates the grid)")
    print(f"  GRID / HUMAN ceiling     : {100*sum(grid_s)/sum(human_s):5.1f}%  (pure quantization loss)")
    print(f"  Refined / HUMAN ceiling  : {100*sum(ref_s)/sum(human_s):5.1f}%  (ratio of means)")
    print(f"  Per-track normalized mean: {100*sum(norm)/len(norm):5.1f}%  (mean model/human, cap 1.5, n={len(norm)})")
    print(f"  Tracks with ~0 human agreement (excluded from norm): {n_zero_human}")

if __name__ == "__main__":
    main()
