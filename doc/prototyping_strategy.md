# Prototyping Strategy

When deciding whether to prototype an idea in Python/Streamlit before implementing it in Rust/Svelte, the key question is: **how much visual or parametric uncertainty does the idea carry?** Ideas with clear, mechanical implementations go straight to code. Ideas whose quality depends on hard-to-predict aesthetics or threshold values benefit from a fast feedback loop first.

---

## Prototype First (Python / Streamlit)

These features have non-obvious outputs that are best validated interactively before committing to a Rust implementation.

### Projection Algorithm Explorer

The `tools/projection_comparison.png` already confirms that the current UMAP layout is cramped and PCA is a strong alternative. The next question is: what are the right default parameter values for t-SNE and Diffusion Map on this specific library?

A Streamlit app with real-time sliders for `perplexity`, `n_neighbors`, `min_dist`, and `clap_blend_weight` that rerenders the scatter plot on change would answer this in an afternoon. Without it, finding good defaults means repeatedly rebuilding the Tauri app and re-running expensive passes.

**Estimated prototype time:** 1–2 hours.
**What it validates:** Default parameter values for t-SNE and Diffusion Map; which algorithm to recommend as the default.

### Outlier Satellite Region Layout

The two-pass projection concept (core tracks in `[10, 90]`, outlier sub-clusters placed in corner regions) has aesthetics that are hard to reason about on paper. The number of satellite tiles, their size, their visual separation from the main canvas, and whether the outlier sub-clusters are actually meaningful all need to be seen before designing the Rust implementation and the DB schema for `is_map_outlier`.

**Estimated prototype time:** 2–3 hours (builds on the projection explorer).
**What it validates:** Visual layout of satellite regions; whether outlier sub-clusters are coherent enough to be useful.

### Duplicate / Remix Threshold Tuning

The CLAP cosine similarity thresholds for the three detection tiers (≥ 0.97 exact, ≥ 0.90 near-duplicate, ≥ 0.75 remix) are initial estimates based on the pairwise distance distribution of this library. A Streamlit app that loads all embeddings, lets you drag the thresholds, and shows the resulting duplicate groups — with track titles and artists — would validate them against the actual library in minutes and surface false positives before any `track_relationships` schema is designed.

**Estimated prototype time:** 1–2 hours.
**What it validates:** Threshold values; false positive rate at each tier; whether the artist overlap + title keyword heuristics are necessary or sufficient on their own.

---

## Go Straight to Implementation

These are mechanical changes with well-understood outputs. Prototyping adds no value.

- **Percentile-clipped normalization** — one function change in `commands/map.rs`, proven correct by the data analysis.
- **Silent Qwen failure fix** — already implemented.
- **`description_embed` re-queuing** — already implemented.
- **English language instruction in the Qwen prompt** — one line added to the prompt string in `analysis.rs`.
- **Semantic NLP search** — the IPC plumbing is clear, the MiniLM model is already loaded, the `description_embeddings` vec0 table already exists. The implementation path is fully specified in `doc/feature-evaluations/local_nlp_semantic_search.md`.
- **Energy-based CLAP window selection** — the algorithm is unambiguous. A short Python script to validate window selection on a handful of tracks is useful; a full Streamlit UI is not.

---

## Neither — Design Discussion First

These are too large or too UI-dependent to prototype usefully in Streamlit.

- **UI redesign** (persistent player bar, filter sidebar, track detail pane) — too much surface area. Implement incrementally in Svelte once the current Qwen run finishes and the underlying data is stable. The design is already documented in `doc/ui_ideas.md`.
- **Energy-based CLAP window selection** — the algorithm is clear, but the impact (better embeddings → better map) won't be visible until the full CLAP pass re-runs for all 1,886 tracks. Not worth a Streamlit prototype; go straight to Rust once the current analysis passes settle.

---

## Recommended Build Order for Prototypes

1. **Projection Algorithm Explorer** — most leverage; unblocks all map-related decisions.
2. **Outlier Satellite Layout** — extends the explorer, can reuse most of its code.
3. **Duplicate Threshold Tuner** — independent, can be built in parallel with 1–2.
