#!/usr/bin/env python3
"""
Structure Map — UMAP on pairwise Levenshtein distance matrix of sax_alignment strings.

Length/position invariant: IVPCVPCO and IIVVPCVVPCCCO share the same skeleton
so they'll land close together regardless of repeat counts.
"""

import sqlite3
import numpy as np
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
from rapidfuzz.process import cdist
from rapidfuzz.distance import Levenshtein
import umap
import hdbscan

DB = "/Users/rlupi/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db"

# ── Load ──────────────────────────────────────────────────────────────────────
conn = sqlite3.connect(DB)
rows = conn.execute(
    "SELECT id, artist, title, sax_alignment FROM tracks "
    "WHERE sax_alignment IS NOT NULL "
    "  AND length(sax_alignment) >= 4 "
    "  AND sax_alignment NOT LIKE '%➔%'"  # skip old-format rows
).fetchall()
conn.close()

ids, artists, titles, alignments = zip(*rows)
n = len(alignments)
print(f"Loaded {n} tracks")
print(f"Alignment lengths: min={min(len(a) for a in alignments)} "
      f"max={max(len(a) for a in alignments)} "
      f"median={int(np.median([len(a) for a in alignments]))}")

# ── Skeleton helper ───────────────────────────────────────────────────────────
def skeleton(s):
    """Collapse runs: IIVVVVPCCCCO → IVPCO"""
    if not s:
        return ''
    out = [s[0]]
    for c in s[1:]:
        if c != out[-1]:
            out.append(c)
    return ''.join(out)

skeletons = [skeleton(a) for a in alignments]

# ── Pairwise Levenshtein distance matrix ──────────────────────────────────────
print("Computing pairwise Levenshtein distances…")
dist = cdist(alignments, alignments, scorer=Levenshtein.distance, dtype=np.float32)

print(f"Distance matrix: {dist.shape}, "
      f"mean={dist.mean():.2f}, max={dist.max():.0f}")

# ── UMAP on precomputed distances ─────────────────────────────────────────────
print("Running UMAP…")
reducer = umap.UMAP(
    n_neighbors=20,
    min_dist=0.05,
    metric='precomputed',
    random_state=42,
)
embedding = reducer.fit_transform(dist)

# ── Color scheme ──────────────────────────────────────────────────────────────
def classify(a, sk):
    if 'B' in sk:            return ('has bridge',  '#b07fd4')
    if not a.startswith('I') and not a.startswith('II'):
                             return ('no intro',    '#e8a020')
    if 'O' not in sk:        return ('no outro',    '#5b8dd9')
    if 'P' not in sk:        return ('no pre-cho',  '#4db89a')
    return                          ('standard',    '#ffffff')

groups = [classify(a, sk) for a, sk in zip(alignments, skeletons)]
colors   = [c for _, c in groups]
glabels  = {label: color for label, color in groups}

# Also compute normalized distance for size (tracks with many close neighbours = denser)
# Use inverse mean distance to 10 nearest as "centrality"
nearest_mean = np.sort(dist, axis=1)[:, 1:11].mean(axis=1)
sizes = 4 + 12 * (1 - (nearest_mean - nearest_mean.min()) / (nearest_mean.max() - nearest_mean.min()))

# ── Plot ──────────────────────────────────────────────────────────────────────
# ── Skeleton distance matrix + UMAP ──────────────────────────────────────────
print("Computing skeleton Levenshtein distances…")
skel_dist = cdist(skeletons, skeletons, scorer=Levenshtein.distance, dtype=np.float32)
print(f"Skeleton distance matrix: mean={skel_dist.mean():.2f}, max={skel_dist.max():.0f}")

print("Running skeleton UMAP…")
skel_embedding = umap.UMAP(
    n_neighbors=20, min_dist=0.05, metric='precomputed', random_state=42
).fit_transform(skel_dist)

# ── HDBSCAN clustering on skeleton UMAP embedding ────────────────────────────
print("Clustering skeleton embedding…")
clusterer = hdbscan.HDBSCAN(min_cluster_size=40, min_samples=5)
cluster_labels = clusterer.fit_predict(skel_embedding)

n_clusters = cluster_labels.max() + 1
n_noise    = (cluster_labels == -1).sum()
print(f"Found {n_clusters} clusters, {n_noise} noise points ({100*n_noise/len(cluster_labels):.1f}%)")

# Assign a distinct color per cluster; noise → grey
cmap = plt.cm.get_cmap('tab20', n_clusters)
def cluster_color(label):
    if label == -1:
        return '#333340'
    return cmap(label % 20)

cluster_colors = [cluster_color(l) for l in cluster_labels]

# Compute cluster centroids in skeleton UMAP space
centroids = {}
for cid in range(n_clusters):
    mask = cluster_labels == cid
    centroids[cid] = skel_embedding[mask].mean(axis=0)

# ── Plot ──────────────────────────────────────────────────────────────────────
fig, axes = plt.subplots(1, 4, figsize=(30, 8))
fig.patch.set_facecolor('#111116')

# Left: structural groups
ax = axes[0]
ax.set_facecolor('#111116')
ax.scatter(embedding[:, 0], embedding[:, 1],
           c=colors, s=sizes, alpha=0.7, linewidths=0)
patches = [mpatches.Patch(color=c, label=l) for l, c in sorted(glabels.items())]
ax.legend(handles=patches, loc='lower right', framealpha=0.3,
          labelcolor='white', facecolor='#222228', edgecolor='#444', fontsize=9)
ax.set_title('Structural groups', color='white', fontsize=12, pad=10)
ax.tick_params(colors='#555')
for sp in ax.spines.values(): sp.set_edgecolor('#333')

# Right: alignment length
ax = axes[1]
ax.set_facecolor('#111116')
lengths = np.array([len(a) for a in alignments])
sc = ax.scatter(embedding[:, 0], embedding[:, 1],
                c=lengths, cmap='plasma', s=sizes, alpha=0.7, linewidths=0)
cbar = fig.colorbar(sc, ax=ax, fraction=0.03, pad=0.02)
cbar.set_label('alignment length', color='white', fontsize=9)
cbar.ax.yaxis.set_tick_params(color='white')
plt.setp(cbar.ax.yaxis.get_ticklabels(), color='white')
ax.set_title('Alignment length', color='white', fontsize=12, pad=10)
ax.tick_params(colors='#555')
for sp in ax.spines.values(): sp.set_edgecolor('#333')

# Third panel: skeleton UMAP
ax = axes[2]
ax.set_facecolor('#111116')
skel_nearest = np.sort(skel_dist, axis=1)[:, 1:11].mean(axis=1)
skel_sizes = 4 + 12 * (1 - (skel_nearest - skel_nearest.min()) / (skel_nearest.max() - skel_nearest.min() + 1e-9))
ax.scatter(skel_embedding[:, 0], skel_embedding[:, 1],
           c=colors, s=skel_sizes, alpha=0.7, linewidths=0)
ax.legend(handles=patches, loc='lower right', framealpha=0.3,
          labelcolor='white', facecolor='#222228', edgecolor='#444', fontsize=9)
ax.set_title('Skeleton (collapsed runs)', color='white', fontsize=12, pad=10)
ax.tick_params(colors='#555')
for sp in ax.spines.values(): sp.set_edgecolor('#333')

# Fourth panel: cluster colors on raw alignment map (cross-view preview)
ax = axes[3]
ax.set_facecolor('#111116')
ax.scatter(embedding[:, 0], embedding[:, 1],
           c=cluster_colors, s=sizes, alpha=0.7, linewidths=0)
# Mark centroids on skeleton coords mapped back — just label the skeleton panel instead
ax.set_title('Structure clusters on acoustic layout\n(skeleton clusters → raw alignment map)',
             color='white', fontsize=11, pad=10)
ax.tick_params(colors='#555')
for sp in ax.spines.values(): sp.set_edgecolor('#333')

# Also overlay cluster labels on skeleton panel
ax_skel = axes[2]
ax_skel.scatter(skel_embedding[:, 0], skel_embedding[:, 1],
                c=cluster_colors, s=skel_sizes, alpha=0.8, linewidths=0)
for cid, (cx, cy) in centroids.items():
    ax_skel.text(cx, cy, str(cid), fontsize=6, color='white', ha='center', va='center',
                 fontweight='bold', alpha=0.8)
ax_skel.set_title('Skeleton clusters (HDBSCAN)', color='white', fontsize=12, pad=10)
ax_skel.tick_params(colors='#555')
for sp in ax_skel.spines.values(): sp.set_edgecolor('#333')

fig.suptitle('Structure Map — UMAP on Levenshtein distance matrix',
             color='white', fontsize=14, y=1.01)
plt.tight_layout()

out = "tools/structure_map.png"
plt.savefig(out, dpi=150, bbox_inches='tight', facecolor='#111116')
print(f"Saved → {out}")

# ── Stats ─────────────────────────────────────────────────────────────────────
from collections import Counter
print(f"\nGroup breakdown:")
for label, count in Counter(l for l, _ in groups).most_common():
    print(f"  {label:20s} {count:4d} ({100*count/n:.1f}%)")

print(f"\nSkeleton diversity: {len(set(skeletons))} unique skeletons")
print("Top 10 skeletons:")
for sk, cnt in Counter(skeletons).most_common(10):
    print(f"  {sk:30s} {cnt:4d}")

plt.show()

# ── Write cluster IDs back to the database ────────────────────────────────────
print("\nWriting structure_cluster_id to database…")
conn = sqlite3.connect(DB)
# -1 (noise) → NULL
updated = 0
for track_id, label in zip(ids, cluster_labels):
    cluster_val = int(label) if label >= 0 else None
    conn.execute(
        "UPDATE tracks SET structure_cluster_id = ? WHERE id = ?",
        (cluster_val, int(track_id)),
    )
    updated += 1
conn.commit()
conn.close()
print(f"Updated {updated} tracks with structure cluster IDs ({n_clusters} clusters + NULL for noise)")
