#!/usr/bin/env python3
import os
import json
import sqlite3
from pathlib import Path
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from evaluate_salami_boundaries import (
    load_onset_map,
    parse_jams_boundaries_and_labels,
    evaluate_boundaries,
    evaluate_pairwise_clustering,
    sym_boundary_f1,
    DB_PATH,
    VAL_TRACKS_PATH,
    JAMS_DIR,
)

def main():
    if not VAL_TRACKS_PATH.exists():
        print(f"Error: Validation tracks list not found at {VAL_TRACKS_PATH}. Run prepare_eval_splits.py first.")
        return

    with open(VAL_TRACKS_PATH, "r") as f:
        val_tracks = json.load(f)

    onset_map = load_onset_map()

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    results = []

    for track in val_tracks:
        db_id = track["db_id"]
        salami_id = track["salami_id"]
        
        # Load metadata
        cursor.execute("SELECT duration_seconds, title, artist, genre FROM tracks WHERE id = ?", (db_id,))
        row = cursor.fetchone()
        if not row or not row[0]:
            continue
        duration, title, artist, genre = row

        # Parse JAMS ground truth
        jams_path = JAMS_DIR / f"SALAMI_{salami_id}.jams"
        onset_offset = onset_map.get(salami_id, 0.0)
        gt_passes = parse_jams_boundaries_and_labels(jams_path, onset_offset, duration)

        # We can only compute agreement if we have exactly 2 annotators
        if len(gt_passes) != 2:
            continue

        pass1, pass2 = gt_passes[0], gt_passes[1]

        # 1. Symmetric Boundary Agreement at 0.5s and 3.0s using mir_eval
        f1_0_5 = sym_boundary_f1(pass1["boundaries"], pass2["boundaries"], duration, 0.5)
        f1_3_0 = sym_boundary_f1(pass1["boundaries"], pass2["boundaries"], duration, 3.0)

        # 2. Pairwise Clustering Agreement
        # Map segments from pass1 as "prediction labels" for comparison
        times = []
        t = 0.0
        step = 0.5
        while t <= duration:
            times.append(t)
            t += step

        pass1_labels_series = []
        for t in times:
            lbl = "silence"
            for seg in pass1["segments"]:
                if seg["start"] <= t <= seg["end"]:
                    lbl = seg["label"]
                    break
            pass1_labels_series.append(lbl)

        _, _, pw_f1 = evaluate_pairwise_clustering(pass1_labels_series, pass2["segments"], duration, step=0.5)

        results.append({
            "salami_id": salami_id,
            "title": title or "Unknown",
            "artist": artist or "Unknown",
            "genre": genre or "Unknown",
            "f1_0_5": f1_0_5 * 100,
            "f1_3_0": f1_3_0 * 100,
            "pw_f1": pw_f1 * 100,
            "n_boundaries_a1": len(pass1["boundaries"]),
            "n_boundaries_a2": len(pass2["boundaries"]),
        })

    conn.close()

    if not results:
        print("No tracks with dual annotations found in the validation set.")
        return

    # Sort by 3.0s boundary agreement (ascending for hardest, descending for easiest)
    results.sort(key=lambda x: x["f1_3_0"])

    print(f"\nTotal tracks with dual annotators evaluated: {len(results)}")
    
    print("\n" + "=" * 90)
    print(" TOP 10 HARDEST TRACKS (Lowest Human Agreement at ±3s)")
    print("=" * 90)
    print(f"{'ID':<6}{'Title':<28}{'Artist':<20}{'Genre':<15}{'F1@3s':>8}{'F1@0.5s':>9}{'PW F1':>8}")
    print("-" * 90)
    for r in results[:10]:
        print(f"{r['salami_id']:<6}{r['title'][:26]:<28}{r['artist'][:18]:<20}{r['genre'][:13]:<15}{r['f1_3_0']:>7.1f}%{r['f1_0_5']:>8.1f}%{r['pw_f1']:>7.1f}%")

    print("\n" + "=" * 90)
    print(" TOP 10 EASIEST TRACKS (Highest Human Agreement at ±3s)")
    print("=" * 90)
    print(f"{'ID':<6}{'Title':<28}{'Artist':<20}{'Genre':<15}{'F1@3s':>8}{'F1@0.5s':>9}{'PW F1':>8}")
    print("-" * 90)
    for r in reversed(results[-10:]):
        print(f"{r['salami_id']:<6}{r['title'][:26]:<28}{r['artist'][:18]:<20}{r['genre'][:13]:<15}{r['f1_3_0']:>7.1f}%{r['f1_0_5']:>8.1f}%{r['pw_f1']:>7.1f}%")

    # Average human agreement across all dual-annotator tracks
    avg_f1_3_0 = sum(r["f1_3_0"] for r in results) / len(results)
    avg_f1_0_5 = sum(r["f1_0_5"] for r in results) / len(results)
    avg_pw_f1 = sum(r["pw_f1"] for r in results) / len(results)

    print("\n" + "=" * 90)
    print(f"AVERAGE HUMAN AGREEMENT OVERALL (N={len(results)}):")
    print(f"  Boundary F1 (±3.0s): {avg_f1_3_0:.2f}%")
    print(f"  Boundary F1 (±0.5s): {avg_f1_0_5:.2f}%")
    print(f"  Pairwise F1:         {avg_pw_f1:.2f}%")
    print("=" * 90)

if __name__ == "__main__":
    main()
