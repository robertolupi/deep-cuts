#!/usr/bin/env python3
"""
Diagnostic tool for CLAP audio embedding quality.

Loads all embeddings from the Deep Cuts SQLite database and reports:
  1. Count & norm check   — all embeddings should have norm ≈ 1.0
  2. Centroid norm        — near zero means embeddings are well-spread
  3. Pairwise distance    — random sample; healthy distribution peaks ~0.9–1.2
  4. Nearest-neighbour    — per-track closest match; too low → collapsed space

Run from the repo root with the tools venv active:
  source tools/.venv/bin/activate
  python tools/check_embeddings.py [--db PATH]
"""

import argparse
import struct
import sqlite3
import sys
import numpy as np
import sqlite_vec

DEFAULT_DB = os.path.expanduser("~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db")

EMBEDDING_DIM = 512
PAIRWISE_SAMPLE = 2000   # max tracks used for pairwise distance sampling
NN_WARN_THRESHOLD = 0.10  # nearest-neighbour distance below this is suspicious


def load_embeddings(db_path: str) -> tuple[np.ndarray, list[int]]:
    conn = sqlite3.connect(db_path)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)
    rows = conn.execute(
        "SELECT track_id, embedding FROM audio_embeddings ORDER BY track_id"
    ).fetchall()
    conn.close()

    if not rows:
        print("No embeddings found in the database.")
        sys.exit(1)

    track_ids = []
    vectors = []
    bad = 0
    for track_id, blob in rows:
        if len(blob) != EMBEDDING_DIM * 4:
            bad += 1
            continue
        vec = np.array(struct.unpack(f"{EMBEDDING_DIM}f", blob), dtype=np.float32)
        track_ids.append(track_id)
        vectors.append(vec)

    if bad:
        print(f"  ⚠  Skipped {bad} row(s) with unexpected blob size.")

    return np.stack(vectors), track_ids


def section(title: str):
    print(f"\n{'─' * 60}")
    print(f"  {title}")
    print(f"{'─' * 60}")


def check_norms(E: np.ndarray):
    section("1. Embedding norms  (should all be ≈ 1.0)")
    norms = np.linalg.norm(E, axis=1)
    print(f"  count : {len(norms)}")
    print(f"  min   : {norms.min():.6f}")
    print(f"  max   : {norms.max():.6f}")
    print(f"  mean  : {norms.mean():.6f}")
    print(f"  std   : {norms.std():.6f}")
    bad = np.sum(np.abs(norms - 1.0) > 0.05)
    if bad == 0:
        print("  ✓  All norms within ±5% of 1.0")
    else:
        print(f"  ✗  {bad} embeddings have norm outside ±5% of 1.0 — possible encode error")


def check_centroid(E: np.ndarray):
    section("2. Centroid norm  (should be near 0 for a well-spread space)")
    centroid = E.mean(axis=0)
    norm = np.linalg.norm(centroid)
    print(f"  centroid norm : {norm:.6f}")
    if norm < 0.1:
        print("  ✓  Centroid close to origin — embeddings are well-spread")
    elif norm < 0.3:
        print("  ⚠  Moderate centroid bias — slight directional skew")
    else:
        print("  ✗  High centroid norm — embeddings may be collapsed or biased")


def check_pairwise(E: np.ndarray):
    section(f"3. Pairwise L2 distance  (random sample ≤ {PAIRWISE_SAMPLE} tracks)")
    n = min(len(E), PAIRWISE_SAMPLE)
    rng = np.random.default_rng(42)
    idx = rng.choice(len(E), size=n, replace=False)
    S = E[idx]

    # Compute pairwise L2 via ||a-b||² = ||a||² + ||b||² - 2·aᵀb
    # For unit vectors: ||a-b||² = 2 - 2·aᵀb  →  distance = sqrt(2 - 2·cosine)
    gram = S @ S.T
    gram = np.clip(gram, -1.0, 1.0)
    dist_sq = 2.0 - 2.0 * gram
    np.fill_diagonal(dist_sq, np.nan)
    dists = np.sqrt(np.clip(dist_sq, 0, None)).flatten()
    dists = dists[~np.isnan(dists)]

    p = np.percentile(dists, [5, 25, 50, 75, 95])
    print(f"  pairs sampled : {len(dists):,}")
    print(f"  p5  / p25     : {p[0]:.4f} / {p[1]:.4f}")
    print(f"  median        : {p[2]:.4f}")
    print(f"  p75 / p95     : {p[3]:.4f} / {p[4]:.4f}")

    near_zero = np.sum(dists < 0.05) / len(dists) * 100
    if near_zero > 5:
        print(f"  ✗  {near_zero:.1f}% of pairs have distance < 0.05 — space may be collapsed")
    else:
        print(f"  ✓  Only {near_zero:.1f}% of pairs within distance 0.05")

    # ASCII histogram
    bins = np.arange(0, 2.1, 0.1)
    counts, _ = np.histogram(dists, bins=bins)
    peak = counts.max()
    print("\n  Distribution (L2 distance 0 → 2):")
    for i, c in enumerate(counts):
        bar = "█" * int(c / peak * 40)
        print(f"  {bins[i]:.1f}–{bins[i+1]:.1f}  {bar} {c:,}")


def check_nearest_neighbour(E: np.ndarray, track_ids: list[int]):
    section("4. Nearest-neighbour distances  (excluding self)")
    # For large libraries compute in batches to avoid OOM
    batch = 512
    nn_dists = []
    for start in range(0, len(E), batch):
        S = E[start:start + batch]
        gram = S @ E.T
        gram = np.clip(gram, -1.0, 1.0)
        dist_sq = 2.0 - 2.0 * gram
        # Mask self-matches
        for i in range(len(S)):
            dist_sq[i, start + i] = np.inf
        nn_dists.append(np.sqrt(np.clip(dist_sq.min(axis=1), 0, None)))

    nn = np.concatenate(nn_dists)
    p = np.percentile(nn, [5, 25, 50, 75, 95])
    print(f"  p5  / p25     : {p[0]:.4f} / {p[1]:.4f}")
    print(f"  median        : {p[2]:.4f}")
    print(f"  p75 / p95     : {p[3]:.4f} / {p[4]:.4f}")

    collapsed = np.sum(nn < NN_WARN_THRESHOLD)
    if collapsed > 0:
        print(f"  ✗  {collapsed} track(s) have a nearest neighbour within {NN_WARN_THRESHOLD} "
              f"— likely duplicates or embedding collapse")
    else:
        print(f"  ✓  No tracks with nearest neighbour below {NN_WARN_THRESHOLD}")

    # Show the 5 most isolated and 5 most duplicated
    order = np.argsort(nn)
    print("\n  5 most similar pairs (lowest NN distance):")
    for i in order[:5]:
        print(f"    track_id={track_ids[i]}  nn_dist={nn[i]:.4f}")
    print("\n  5 most isolated tracks (highest NN distance):")
    for i in order[-5:][::-1]:
        print(f"    track_id={track_ids[i]}  nn_dist={nn[i]:.4f}")


def main():
    parser = argparse.ArgumentParser(description="Check Deep Cuts CLAP embedding quality")
    parser.add_argument("--db", default=DEFAULT_DB, help="Path to deep_cuts.db")
    args = parser.parse_args()

    print(f"Loading embeddings from:\n  {args.db}")
    E, track_ids = load_embeddings(args.db)
    print(f"Loaded {len(E)} embeddings ({EMBEDDING_DIM}-d each)")

    check_norms(E)
    check_centroid(E)
    check_pairwise(E)
    check_nearest_neighbour(E, track_ids)
    print(f"\n{'─' * 60}\n  Done.\n{'─' * 60}\n")


if __name__ == "__main__":
    main()
