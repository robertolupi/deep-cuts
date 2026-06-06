"""
Compute pairwise Levenshtein distances over waveform_sax strings and plot a histogram.
"""
import sqlite3
import time
from pathlib import Path

import numpy as np
from rapidfuzz import process
from rapidfuzz.distance import Levenshtein

DB = Path.home() / "Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"


def main():
    con = sqlite3.connect(DB)
    rows = con.execute(
        "SELECT waveform_sax FROM tracks WHERE waveform_sax IS NOT NULL"
    ).fetchall()
    con.close()

    strings = [r[0] for r in rows]
    print(f"Loaded {len(strings)} SAX strings (len={len(strings[0])})")

    t0 = time.perf_counter()
    dm = process.cdist(strings, strings, scorer=Levenshtein.distance, workers=-1)
    elapsed = time.perf_counter() - t0
    print(f"cdist completed in {elapsed:.2f}s  shape={dm.shape}")

    # Upper triangle only (exclude diagonal)
    upper = dm[np.triu_indices_from(dm, k=1)]
    print(f"Pairs: {len(upper):,}")
    print(f"min={upper.min()}  max={upper.max()}  mean={upper.mean():.2f}  median={np.median(upper):.1f}")

    try:
        import matplotlib.pyplot as plt
        fig, ax = plt.subplots(figsize=(10, 5))
        ax.hist(upper, bins=range(0, int(upper.max()) + 2), edgecolor="black", color="steelblue")
        ax.set_xlabel("Levenshtein distance")
        ax.set_ylabel("Pair count")
        ax.set_title(f"Pairwise SAX Levenshtein — {len(strings):,} tracks, {len(upper):,} pairs")
        plt.tight_layout()
        out = Path(__file__).parent / "sax_levenshtein_histogram.png"
        plt.savefig(out, dpi=150)
        print(f"Saved histogram → {out}")
    except ImportError:
        print("matplotlib not available — skipping plot")
        # Print a simple text histogram
        counts = np.bincount(upper)
        for d, c in enumerate(counts):
            if c:
                print(f"  d={d:2d}  {'#' * min(c // 1000, 60)}  {c:,}")


if __name__ == "__main__":
    main()
