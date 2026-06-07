#!/usr/bin/env python3
"""
Benchmark-standard boundary eval with mir_eval (SALAMI validation, dual-annotator).

Our homegrown evaluate_boundaries() uses greedy nearest-match; every SALAMI paper
reports mir_eval.segment.detection (optimal bipartite matching). To make any
cross-paper / SoTA claim, scores MUST come from mir_eval on the standard protocol.

This recomputes baseline, refined (augment+8peaks_5s), and the human consensus
ceiling under both our metric and mir_eval, at ±0.5s and ±3s, on the same
dual-annotator subset. trim=True drops the trivial track-start/end boundaries
(matches our 0<t<dur exclusion and MIREX convention).

Validation only. Holdout untouched.
"""
import os
import sys
import json
import sqlite3
import warnings
import numpy as np
import mir_eval

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from evaluate_salami_boundaries import (  # noqa: E402
    load_onset_map, parse_jams_boundaries_and_labels, evaluate_boundaries,
    DB_PATH, VAL_TRACKS_PATH, JAMS_DIR,
)
from refine_salami_boundaries import (  # noqa: E402
    baseline_boundaries, ranked_novelty_peaks, augment_with_peaks,
)

warnings.filterwarnings("ignore", category=UserWarning)


def intervals(bounds, dur):
    b = sorted(set([0.0] + [x for x in bounds if 0 < x < dur] + [float(dur)]))
    return mir_eval.util.boundaries_to_intervals(np.array(b))


def mir_f(ref_b, est_b, dur, win):
    _, _, f = mir_eval.segment.detection(
        intervals(ref_b, dur), intervals(est_b, dur), window=win, trim=True)
    return f


def main():
    val = json.load(open(VAL_TRACKS_PATH))
    onset = load_onset_map()
    con = sqlite3.connect(DB_PATH)
    cur = con.cursor()
    A = {k: [] for k in ("base_our", "base_mir", "ref_our", "ref_mir",
                         "hum_our", "hum_mir", "ref_mir5", "hum_mir5")}
    n = 0
    for t in val:
        cur.execute("SELECT duration_seconds, waveform_data, sax_alignment_segments "
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
        if len(passes) != 2:
            continue
        n += 1
        base = baseline_boundaries(labels, dur)
        refined = augment_with_peaks(base, ranked_novelty_peaks(wf, dur), 8, 5.0)
        for tag, est in (("base", base), ("ref", refined)):
            our, mir, mir5 = [], [], []
            for p in passes:
                _, _, f = evaluate_boundaries(est, p["boundaries"], 3.0)
                our.append(f)
                mir.append(mir_f(p["boundaries"], est, dur, 3.0))
                if tag == "ref":
                    mir5.append(mir_f(p["boundaries"], est, dur, 0.5))
            A[f"{tag}_our"].append(np.mean(our))
            A[f"{tag}_mir"].append(np.mean(mir))
            if tag == "ref":
                A["ref_mir5"].append(np.mean(mir5))
        _, _, f = evaluate_boundaries(passes[0]["boundaries"], passes[1]["boundaries"], 3.0)
        A["hum_our"].append(f)
        A["hum_mir"].append(mir_f(passes[0]["boundaries"], passes[1]["boundaries"], dur, 3.0))
        A["hum_mir5"].append(mir_f(passes[0]["boundaries"], passes[1]["boundaries"], dur, 0.5))
    con.close()

    def m(x):
        return 100 * np.mean(x)

    print(f"\nDual-annotator validation subset  N={n}")
    print("-" * 56)
    print(f"{'@3s':<18}{'our greedy':>12}{'mir_eval':>12}")
    print(f"{'  baseline':<18}{m(A['base_our']):>11.2f}%{m(A['base_mir']):>11.2f}%")
    print(f"{'  refined':<18}{m(A['ref_our']):>11.2f}%{m(A['ref_mir']):>11.2f}%")
    print(f"{'  human ceiling':<18}{m(A['hum_our']):>11.2f}%{m(A['hum_mir']):>11.2f}%")
    print("-" * 56)
    print(f"{'@0.5s (mir_eval)':<18}{'refined':>12}{'human':>12}")
    print(f"{'':<18}{m(A['ref_mir5']):>11.2f}%{m(A['hum_mir5']):>11.2f}%")
    print("-" * 56)
    print(f"Refined / Human (mir_eval @3s): {m(A['ref_mir'])/m(A['hum_mir'])*100:.1f}%")


if __name__ == "__main__":
    main()
