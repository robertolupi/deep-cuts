---
status: proposed
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# Waveform Envelope Analysis

This document outlines structural and mathematical features utilizing the 128-point low-resolution `waveform_data` (RMS/energy envelope) stored in the Deep Cuts database.

---

## 1. Two-Stage Structural Similarity Search (KNN + DTW)

To query the library for tracks sharing similar energy arrangement shapes (e.g. build-ups, breakdowns, climax positions) regardless of absolute tempo/duration differences:

### Stage 1: Fast $k$-NN Pruning (`sqlite-vec`)
- The 128-point $L_2$-normalized envelopes are stored in a `vec0` virtual table in SQLite.
- A fast L2 distance query is executed to retrieve the top 100 shape candidates.
- *Limitation*: Raw Euclidean/Cosine calculations are sensitive to time-shifts and compression/expansion of the shape.

### Stage 2: Shape Refinement (Rust DTW)
- The Rust backend runs Dynamic Time Warping (DTW) on the 100 candidate envelopes against the seed track's envelope.
- DTW accommodates non-linear time stretching and compression, yielding a tempo-independent similarity ranking.

---

## 2. Pairwise Matrix Similarity ($M M^T$)

If the $L_2$-normalized envelopes of all $N$ tracks in the library are placed in an $N \times 128$ matrix $M$, computing $M M^T$ yields an $N \times N$ matrix $S$.

$$S_{i, j} = \text{Cosine Similarity between Track } i \text{ and Track } j$$

For $N = 2,000$ tracks, this matrix takes up ~16 MB and can be calculated in milliseconds using SIMD or BLAS routines.

### A. Discrete Graph Clustering
Treating $S$ as a weighted graph adjacency matrix allows us to run algorithms like Louvain community detection to automatically categorize the library into discrete structural groups (e.g. static loops, classical builds).

### B. Stable Spectral Map Projections
By taking the top eigenvectors of the graph Laplacian derived from $S$ (**Laplacian Eigenmaps**), we can project tracks onto a 2D map layout (**Arrangement Space**). This projection is stable and deterministic; adding new tracks will not warp or rotate the existing canvas.

### C. Structural Standardness & Obscurity Metrics
- **Structural Centroid**: Summing the rows of $S$ reveals which track is structurally most similar to all other tracks in the library.
- **Structural Obscurity**: The track with the lowest row sum is identified as the most unconventional or structurally unique song in the library.
