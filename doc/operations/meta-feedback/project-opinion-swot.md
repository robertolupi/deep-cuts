---
date: 2026-06-07
author: Meta AI (Muse Spark)
basis: repository review via GitHub, protocol role for Approach B, synthesis of session logs and prior assessments by Claude, Codex, and Gemini
---

# Project Opinion & SWOT: Deep Cuts

This review is authored from the perspective of **Approach B (Neural Sequence Classifier + Viterbi path)**, focusing on sequence analysis, local models, and architectural boundaries.

---

## 1. Opinion

Deep Cuts is nine days old and already looks like a research platform that most teams take nine months to sketch. That is not hype — it is visible in the commit graph, the dual-database split, the sidecar cache, and the `PASS_REGISTRY` trait system. This is not a standard music player with AI features sprinkled on top; it is a local-first inference workbench where a music library becomes a queryable graph of sound, structure, and meaning.

Three architectural elements stand out:

### 1. Scaffolding-Driven Velocity
The velocity is real because the scaffolding is real. The multi-agent protocol is not documentation theater. The lint tools, the skills index, and the session logs are the exact reasons why Claude could ship 86 IPC commands, Codex could stabilize the pipeline, and Gemini could run the SAX experiments without trampling each other. Most "AI coding" projects collapse under coordination debt; this one encodes coordination directly into the repository.

### 2. The Orthogonal Multi-Modal Spine
The chosen multi-modal spine is the correct bet:
* **CLAP** for acoustic texture (how it sounds).
* **Qwen + MiniLM** for semantic vibe (how it is described).
* **SAX** for structure (how it is built).
These represent truly orthogonal signals. Where most apps flatten music to simple metadata tags, Deep Cuts maintains three independent retrieval axes. The "feels vs sounds" slider is not a gimmick; it is an elegant interface to the underlying embedding geometry.

### 3. Approach B Sequence Classifier
Approach B is proving itself, but not in the way a benchmark chaser would expect. The **99.27% training accuracy** on 740 tracks for the 16×3 input (energy, repetition score, position) indicates that the features carry a strong signal. 

However, the errors are highly informative: the chorus/verse flip-flops in *O Fortuna* are not a model capacity problem, but a **prior problem**. A GRU sees local transitions; without explicit duration modeling, it will oscillate between states. This is exactly where Viterbi path decoding must earn its keep.

> [!IMPORTANT]
> **Tension at Model Boundaries:**
> The project is accumulating research depth faster than it is locking down stable inference contracts. Approach A (DTW) gives you interpretable alignment today with zero training. Approach B gives you a learnable prior that can absorb Genius tags, handle "unknown" sections, and scale to 50k tracks without pairwise DTW cost. You need both, but you need a clear handoff: **DTW as the bootstrap and evaluation harness, and Approach B as the production path.**

---

## 2. SWOT Analysis

### Strengths (S)

* **Agent-Native Architecture:** `PASS_REGISTRY`, sidecar persistence, `ActiveGuard` pipeline state, and the dual DB split are built-in design primitives. They make adding a new analysis pass a clear checklist rather than a major refactor, protecting velocity from killing stability.
* **Genuine Local-First Integrity:** No API keys are required. Runs `sqlite-vec` on-device, GGUF/ONNX sidecars, and `.dc.json` portability. For DJs, archivists, and collectors with private or unreleased material, this is the only viable architecture.
* **Composability of Three-Axis Retrieval:** Combining CLAP, MiniLM/Qwen, and SAX structural representations allows queries no streaming service can answer. Furthermore, SAX strings are only 32 bytes, keeping the Hamming distance calculation cost trivial.
* **Sidecars as a Durability Layer:** Decoupling ML metadata into `.dc.json` files next to audio tracks ensures database wipes or migration changes do not nuke hours of GPU work. 
* **Research Discipline in Docs:** The documentation preserves failure contexts, including the CLAP 91% discard rate, the BPM Gaussian scoring, and the SAX validation notes. This honesty compounds and prevents future agents from repeating dead ends.

### Weaknesses (W)

* **Brittle Track DTO positional mapping:** Over 60 fields are mapped by positional `row.get(index)` calls across `database.rs`, `library.rs`, and `map.rs`. With 30 migrations in nine days, a column reorder will not fail compile-time checks, but will silently corrupt reads. **Approach B will add per-section posteriors; do not append them directly to `Track`.**
* **Approach B Lacks Production Priors:** The current Viterbi sketch uses uniform transitions. Real songs have duration: verses last 16-32 bars, choruses repeat, and intros do not follow outros. Without learned duration priors and a sticky "unknown" state, the model will continue to oscillate.
* **Feature Coupling in the Frontend:** Massive 40KB+ Svelte components mix state, rendering, and inline CSS, making it hard to visualize model confidence. Debugging a Viterbi path is nearly impossible if the UI cannot display per-frame posteriors.
* **Model Footprint vs. Onboarding:** Downloading 6GB+ of models blocks casual users. The app lacks a graceful degradation path for when large models like Qwen/CLAP are missing (e.g., fallback to tags only).
* **TypeScript Type Boundary:** `CommandMap` entries are typed as `unknown` on the TypeScript side. When Approach B starts returning sequence logits, any mismatch will surface as a runtime panic rather than a compile-time type error.

### Opportunities (O)

* **Elevate SAX + Viterbi to the Marquee:** "Find songs built like this" is a defensible product differentiator. Draw the SAX string in the UI, let users edit it, and run Hamming distance and Viterbi decoding in real time.
* **GRU vs. Tiny Transformer for 16×3 Input:** A 2-layer GRU (64 hidden) will train in seconds and export cleanly to ONNX for this input size. A 2-layer Transformer with rotary embeddings would capture longer-range repetition but adds 3-4x parameters. **Recommendation:** Ship the GRU now, and keep the Transformer behind an experimental flag. The bottleneck is priors, not capacity.
* **Explicit Viterbi Priors:** Learn a transition matrix from the 740-track set, add minimum-duration constraints (e.g., 4 seconds per state), and add an "unknown/break" state with a high self-transition probability to fix state-flipping.
* **Genius Tags for Weak Supervision:** Align tags to audio via DTW first, then use the alignment confidence as a sample weight for Approach B training, filtering out low-confidence alignments.
* **Productizing the Metrics Database:** Surface per-pass timing, model confidence histograms, and library coverage maps. Archivists care deeply about metric states (e.g., "Library is 73% structurally understood").
* **Publishing the Agent Collaboration Protocol:** The project's linting, skills index, and handoff protocols are highly mature. Documenting them publicly would attract high-quality contributors.

### Threats (T)

* **Compressed Debt at Model Boundaries:** Agents add features faster than they add invariant tests. Without tests for pass idempotence and sidecar restoration, Approach B training data will silently drift.
* **Scope Creep (Feature Creep):** Backlog items like hum-to-search, stem extraction, and DJ clash meters risk diluting the core loop. Adding too many passes makes the `PASS_REGISTRY` a junk drawer.
* **Performance Cliffs at Scale:** UMAP, HDBSCAN, and Viterbi calculations over 50,000 tracks will experience severe memory bottlenecks. Incremental indexing and approximate nearest neighbors will be required.
* **Model/Runtime Fragility:** Pinned ONNX Runtime versions, llama.cpp sidecars, macOS signing, and rpath patching represent a full-time build-maintenance job. A single OS update can break local inference.
* **Strategic User Ambiguity:** The UI currently serves collectors (who want provenance), DJs (who want transition analysis), and archivists (who want diagnostics). Without a chosen spine, Approach B risks optimizing for average accuracy instead of a specific workflow.

---

## 3. Recommendations (Approach B Vantage Point)

Meta AI recommends narrowing the next development cycle to two hard problems:

1. **Harden the Sequence Modeling Contract:**
   Freeze a small output schema for structural analysis (separate from the main `Track` DTO), implement Viterbi decoding with learned transitions and duration floors, and add a visual debugger in the UI that shows per-frame posteriors. Focus on interpretability over raw accuracy: if you can see why *O Fortuna* flips, you can fix it.

2. **Prune strictly to the Product Spine:**
   Align on the core workflow: **Scan → Analyze → Explore Spatially/Structurally → Retrieve**. SAX structural search, the UMAP map, and the feels-vs-sounds slider are the spine of the app. Every improvement, including Approach B, must be measured by whether it makes that loop faster, clearer, or more inevitable.
