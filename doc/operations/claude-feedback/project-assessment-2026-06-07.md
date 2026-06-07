---
date: 2026-06-07
author: Claude (Sonnet 4.6, via Claude Code)
basis: direct codebase work — ~30 commits reviewed, dozens of source files read, bugs fixed, tests written
---

# Project Assessment: Deep Cuts

This assessment is based on my own direct observation while working through this codebase. I have not read the Codex SWOT before writing this.

---

## Opinion

Deep Cuts is 9 days old. That context needs to be stated before anything else.

In those 9 days: a full Rust/Tauri backend was built from scratch with SQLite migrations, a recursive scanner with sidecar persistence, a trait-based multi-pass analysis pipeline, a dual-DB setup separating operational data from metrics, 86 IPC commands, and sidecars for Essentia, CLAP, Qwen, MiniLM, and HDBSCAN. On the frontend: a Svelte 5 app with 6 major UI surfaces, a UMAP music map with D3.js density contours, real-time analysis progress tracking, WaveSurfer waveforms and spectrograms, a rich filter system with 15 orthogonal dimensions, and a semantic chat interface. Plus a full documentation and skills infrastructure for multi-agent collaboration.

This rate of progress is not typical. It is the result of running multiple AI coding agents in parallel (Claude, Codex, Gemini) under a shared protocol, with a human owner making architectural decisions and routing work. The skills system, session logs, and lint tooling are operational artifacts of that method — not aspirational docs. They already work.

Given all of that: the codebase is in exactly the state you would expect and probably better. There is real architecture here. The analysis pipeline has the right abstractions. The research decisions leave evidence of thinking — a CLAP concept-tagging approach tried and abandoned after a 91% discard rate; BPM correction with genre-aware Gaussian scoring; SAX alignment validated against structural ground truth. These are not accidents.

The honest evaluation is not "what is wrong." Nothing material is wrong for day 9. The evaluation is: given that this project is moving at AI-assisted speed, the technical debt that would accumulate over months in a normal project is accumulating in days. The patterns that need to be locked down before the next phase of growth — DTO stability, test coverage, component decomposition — are already visible, and they will be harder to fix at day 90 than at day 9.

---

## SWOT

### Strengths

**The velocity is genuinely exceptional and the infrastructure supports it.** A full music intelligence platform in 9 days is possible only because of the agent collaboration infrastructure — the skills, the protocol, the lint tooling, the session logs. This is not just fast; it is fast with a system. That system is itself a differentiator.

**The analysis pipeline has the right architecture for its ambition.** `PASS_REGISTRY`, `AnalysisPass<R>` trait, sidecar persistence, RAII `ActiveGuard` for pipeline state, sleep prevention during analysis, a separate metrics DB. These are decisions that will pay forward. Adding a new analysis pass is well-defined work with a checklist that actually works.

**The research quality is high.** `sax_structure.md`, `clap_window_selection.md`, `waveform_envelope_analysis.md` document decisions honestly — including failures. The distinction between "tried, learned something, moved on" and "implemented, current state is X" is being actively maintained. That discipline compounds.

**Local-first is a real differentiator, not a positioning choice.** Local models, local DB, local inference, local metrics. The privacy story is coherent. In a world where every audio tool is moving to cloud-subscription, running full AI analysis on a private collection without data leaving the machine is a genuine wedge — especially for collectors, archivists, DJs with unreleased tracks.

**SAX structural search is a legitimately novel idea at near-zero cost.** "Find songs *built like this*" — same energy envelope architecture, same verse-chorus shape, same drop pattern — does not exist in any mainstream tool. It uses 32-char strings, Hamming distance, and data already in the DB. The cost-to-value ratio is extraordinary. This should be the flagship feature, not a roadmap item.

---

### Weaknesses

**The `Track` struct is the primary structural risk.** 60+ fields, growing with every analysis pass, mapped with positional-style SELECT queries. This is survivable at day 9 because the codebase is small enough to audit manually. At day 60 with another 10 passes and 3 more migration files, a position shift will corrupt reads silently with no compile error and no test catching it. The clock is already ticking on this one.

**The frontend is almost entirely untested.** Two test files exist. Six Svelte components over 1000 lines each, a 484-line filter store with 15 filter dimensions, a 435-line player store — all manually tested only. At the rate this frontend is growing, a refactor accident will happen and there will be nothing to catch it.

**The TypeScript type boundary is nominal.** `CommandMap` has 86 entries after this session's work, but most are still typed as `unknown`. The type safety is structural but not yet semantic — the compiler cannot catch a mismatch between what Rust returns and what TypeScript expects.

**The multi-agent authorship pattern accumulates incoherence.** Four AI agents under a shared protocol works. But each agent brings different defaults for error handling, naming, what counts as "done." I found evidence of this in merge conflicts, duplicate patterns, and `filter_map(ok)` silently swallowing errors that had been there since day 1. The lint tooling and skills system are the right mitigations — they need to keep pace with the growth.

**Sidecar and model fragility is compounding.** Each new ONNX model, each new sidecar, each new platform target adds another thing that can be the wrong path, wrong rpath, or wrong version. At 9 days this is manageable. At 90 days with 10 sidecars and macOS/Windows targets, it will require its own dedicated infrastructure layer.

---

### Opportunities

**The collaborative infrastructure is itself worth documenting publicly.** The multi-agent session logs, skills system, and operating discipline documented here are ahead of most published work on human-AI pair programming. The blog drafts in `doc/private/` suggest this is already on the radar. It should happen — this method is producing demonstrably real software at an unusual rate.

**The "feels vs sounds" slider is a signature UI idea.** Blending CLAP acoustic embeddings with MiniLM description embeddings along a continuous axis is clever, explainable, and tangible. If it is made more discoverable and the results are visualized on the UMAP map in real time, it could become the feature that makes the app memorable.

**The metrics DB separation is an underused asset.** Most apps never track their own analysis quality. Deep Cuts has a dedicated metrics database with pass timing, pipeline health, and run history. Building this into a "how well do I understand my library?" view would be distinctive and hard to copy.

**The combination of structural + semantic + acoustic search in one interface has no equivalent.** SAX structural distance (how is it built?), CLAP acoustic similarity (how does it sound?), and MiniLM semantic similarity (how would you describe it?) — three independent similarity axes in one query surface. That combination does not exist. It is not a feature list. It is a genuinely new way to browse a music collection.

---

### Threats

**AI-assisted velocity makes technical debt accumulate on a compressed timeline.** What takes months to go wrong in a traditionally-paced project can go wrong in weeks here. The `Track` DTO fragility, the lack of frontend tests, the component size — these would be 6-month problems in a normal project. Here they could be 6-week problems. The upside of the AI collaboration method (speed) has a corresponding downside (debt accrual rate). This is the defining risk of the development method itself.

**Scope expansion is structurally incentivized by the collaboration pattern.** When adding a new AI-assisted feature is fast and cheap, every interesting idea is tempting. The roadmap already has hum-to-search, A* pathfinding playlists, DJ energy levels, double-drop clash meters, acoustic EQ prefiltering, lazy stem extraction, and more. Each of these is individually reasonable. Together they represent a product with no center of gravity. The pace of ideation is likely to exceed the pace of consolidation unless there is active prioritization discipline.

**The primary user is not yet defined.** The app simultaneously targets collectors, archivists, DJs, producers, and "obsessive listeners." These users have different primary jobs, different tolerance for complexity, and different feature priorities. Without a clearer primary user, the feature surface risks optimizing for no one in particular.

**Model and runtime maintenance is a hidden cost center.** ONNX, GGUF, sidecar rpath patching, platform-specific signing, model manifest versioning — at day 9 this is manageable. At the point where there are 10 models, 3 platforms, and quarterly model updates, it will require either a robust internal tooling layer or a significant ongoing maintenance commitment.

---

## Recommendation

The project is at an unusual inflection point: it has more real architecture than most projects 10x its age, and it is building on it faster than most projects 10x its size. The risk is not capability. It is that the velocity creates the illusion of soundness before the foundations are actually settled.

Two things before the next phase of feature growth:

**1. Harden the data layer.** Fix the `Track` DTO fragility (query-specific structs, migration invariant tests). Until this is done, every new analysis pass is adding weight to a structure that can fail silently. This is the one item where getting it wrong gets harder to fix over time, not easier.

**2. Define the primary user journey explicitly and visibly.** My nomination — structural and semantic library browse: scan, understand spatially and structurally, find tracks by how they're built and how they sound, surface surprising connections. SAX structural search, the UMAP map, and the feels-vs-sounds slider are the spine of that experience. Everything else should be evaluated against whether it strengthens or dilutes it.

The project has real substance and a real method. The next quality jump is not more features — it is making the existing ones excellent and the foundation stable enough to build on at this pace for another year.
