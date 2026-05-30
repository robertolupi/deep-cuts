#!/usr/bin/env python3
"""
Compare 2D projection methods for the Deep Cuts music library.

Loads CLAP + description embeddings from the SQLite database and renders
six side-by-side scatter plots:

  1. Current UMAP coordinates (stored in track_coords)
  2. PCA (top 2 principal components)
  3. Truncated SVD / LSA (same as PCA but without mean-centering, better for sparse embeddings)
  4. Spectral Embedding / Laplacian Eigenmaps (preserves local graph structure)
  5. MDS (metric multidimensional scaling, preserves global distances)
  6. UMAP re-run on raw embeddings via scikit-learn (reproducible, tunable)

Dots are coloured by BPM range when available, otherwise by library index.

Usage:
  source tools/.venv/bin/activate
  python tools/visualize_projections.py [--db PATH] [--out PATH]
"""

import argparse
import struct
import sqlite3
import sys
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import matplotlib.colors as mcolors
import sqlite_vec
from sklearn.decomposition import PCA, TruncatedSVD
from sklearn.manifold import SpectralEmbedding, MDS
from sklearn.preprocessing import StandardScaler

DEFAULT_DB = "/Users/rlupi/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"
DEFAULT_OUT = "tools/projection_comparison.png"
CLAP_DIM = 512
DESC_DIM = 384


def load_data(db_path: str):
    conn = sqlite3.connect(db_path)
    conn.enable_load_extension(True)
    sqlite_vec.load(conn)
    conn.enable_load_extension(False)

    # Stored UMAP coords
    coords_rows = conn.execute(
        "SELECT tc.track_id, tc.x, tc.y, t.bpm "
        "FROM track_coords tc JOIN tracks t ON t.id = tc.track_id "
        "ORDER BY tc.track_id"
    ).fetchall()

    # CLAP embeddings (only tracks that have coords)
    stored_ids = {r[0] for r in coords_rows}
    emb_rows = conn.execute(
        "SELECT ae.track_id, ae.embedding, de.embedding "
        "FROM audio_embeddings ae "
        "LEFT JOIN description_embeddings de ON de.track_id = ae.track_id "
        "ORDER BY ae.track_id"
    ).fetchall()
    conn.close()

    # Build aligned arrays
    id_to_coord = {r[0]: (r[1], r[2], r[3]) for r in coords_rows}
    track_ids, clap_vecs, bpms = [], [], []
    for track_id, clap_blob, desc_blob in emb_rows:
        if track_id not in id_to_coord:
            continue
        if len(clap_blob) != CLAP_DIM * 4:
            continue
        clap = np.array(struct.unpack(f"{CLAP_DIM}f", clap_blob), dtype=np.float32)
        clap = clap / (np.linalg.norm(clap) + 1e-9)

        if desc_blob and len(desc_blob) == DESC_DIM * 4:
            desc = np.array(struct.unpack(f"{DESC_DIM}f", desc_blob), dtype=np.float32)
            desc = desc / (np.linalg.norm(desc) + 1e-9)
            blended = np.concatenate([clap * 0.5, desc * 0.5])
        else:
            blended = clap

        track_ids.append(track_id)
        clap_vecs.append(blended)
        bpms.append(id_to_coord[track_id][2])

    E = np.stack(clap_vecs)
    umap_xy = np.array([[id_to_coord[tid][0], id_to_coord[tid][1]] for tid in track_ids])

    # BPM colour: map to [0,1]; NaN → grey
    bpm_arr = np.array([b if b else np.nan for b in bpms], dtype=float)
    valid_bpm = bpm_arr[~np.isnan(bpm_arr)]
    p5, p95 = (np.percentile(valid_bpm, 5), np.percentile(valid_bpm, 95)) if len(valid_bpm) else (60, 180)
    colours = np.where(
        np.isnan(bpm_arr),
        0.5,
        np.clip((bpm_arr - p5) / (p95 - p5 + 1e-9), 0, 1),
    )

    return E, umap_xy, colours, len(track_ids)


def normalise(xy: np.ndarray) -> np.ndarray:
    lo, hi = xy.min(axis=0), xy.max(axis=0)
    rng = hi - lo
    rng[rng == 0] = 1.0
    return (xy - lo) / rng * 100.0


def scatter(ax, xy, colours, title, n, cmap="plasma"):
    ax.scatter(
        xy[:, 0], xy[:, 1],
        c=colours, cmap=cmap,
        s=4, alpha=0.6, linewidths=0,
    )
    ax.set_title(title, fontsize=9, color="white", pad=4)
    ax.set_xticks([])
    ax.set_yticks([])
    ax.set_facecolor("#0a0b10")
    for spine in ax.spines.values():
        spine.set_edgecolor("#333")
    ax.text(0.02, 0.02, f"n={n:,}", transform=ax.transAxes,
            fontsize=7, color="#666", va="bottom")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--db", default=DEFAULT_DB)
    parser.add_argument("--out", default=DEFAULT_OUT)
    args = parser.parse_args()

    print(f"Loading data from {args.db} …")
    E, umap_xy, colours, n = load_data(args.db)
    print(f"  {n} tracks loaded  |  embedding dim = {E.shape[1]}")

    # --- Compute projections ---
    print("Running PCA …")
    pca_xy = normalise(PCA(n_components=2, random_state=42).fit_transform(E))

    print("Running Truncated SVD (no mean-centring) …")
    svd_xy = normalise(TruncatedSVD(n_components=2, random_state=42).fit_transform(E))

    print("Running Spectral Embedding (Laplacian Eigenmaps) …")
    spec_xy = normalise(
        SpectralEmbedding(n_components=2, n_neighbors=15, random_state=42).fit_transform(E)
    )

    print("Running MDS …")
    # MDS is O(n²) — subsample for large libraries
    MAX_MDS = 1000
    if n > MAX_MDS:
        print(f"  MDS: subsampling to {MAX_MDS} tracks for speed")
        rng = np.random.default_rng(42)
        idx = rng.choice(n, MAX_MDS, replace=False)
        mds_xy_sub = normalise(MDS(n_components=2, random_state=42, normalized_stress="auto").fit_transform(E[idx]))
        mds_xy = np.full((n, 2), np.nan)
        mds_xy[idx] = mds_xy_sub
        mds_colours = colours.copy()
        mask = np.zeros(n, dtype=bool)
        mask[idx] = True
    else:
        mds_xy = normalise(MDS(n_components=2, random_state=42, normalized_stress="auto").fit_transform(E))
        mds_colours = colours
        mask = np.ones(n, dtype=bool)

    # UMAP via scikit-learn-compatible wrapper if available, else skip
    try:
        import umap as umap_lib
        print("Running UMAP (sklearn API) …")
        umap_xy2 = normalise(
            umap_lib.UMAP(n_components=2, n_neighbors=15, min_dist=0.1, random_state=42).fit_transform(E)
        )
        has_umap = True
    except ImportError:
        has_umap = False
        umap_xy2 = None
        print("  umap-learn not installed — skipping sklearn UMAP panel")

    # --- Plot ---
    fig, axes = plt.subplots(2, 3, figsize=(15, 10))
    fig.patch.set_facecolor("#0a0b10")
    fig.suptitle(
        "Deep Cuts — 2D Projection Method Comparison  (colour = BPM, grey = unknown)",
        fontsize=11, color="#ccc", y=0.98,
    )

    scatter(axes[0, 0], umap_xy,   colours,        f"Current UMAP (stored coords)", n)
    scatter(axes[0, 1], pca_xy,    colours,        "PCA (2 components)", n)
    scatter(axes[0, 2], svd_xy,    colours,        "Truncated SVD (no mean-centring)", n)
    scatter(axes[1, 0], spec_xy,   colours,        "Spectral Embedding / Laplacian Eigenmaps\n(n_neighbors=15)", n)

    if mds_xy is not None and not np.all(np.isnan(mds_xy)):
        valid = ~np.isnan(mds_xy[:, 0])
        scatter(axes[1, 1], mds_xy[valid], mds_colours[valid],
                f"MDS (metric, n={valid.sum():,})", valid.sum())
    else:
        axes[1, 1].set_visible(False)

    if has_umap and umap_xy2 is not None:
        scatter(axes[1, 2], umap_xy2, colours,
                "UMAP via umap-learn\n(n_neighbors=15, min_dist=0.1)", n)
    else:
        axes[1, 2].set_visible(False)

    plt.tight_layout(rect=[0, 0, 1, 0.97])
    plt.savefig(args.out, dpi=150, bbox_inches="tight", facecolor="#0a0b10")
    print(f"\nSaved to {args.out}")


if __name__ == "__main__":
    main()
