## [Antigravity, 2026-06-08T11:15:00+02:00]

# Session Synthesis: Full Repository History Review & Brainstorming Preparation

This log provides the main agent's synthesis of the full repository history review (commits 1 to 546) based on the detailed analysis from `HistoryAnalyzerEarly` and `HistoryAnalyzerRecent`. It maps the historical phases, characterizes the patterns of documentation/concept drift, and presents the initial brainstorming proposals.

---

## 1. Synthesis of Development Phases

Deep Cuts evolved from a skeleton workspace to a highly sophisticated, multi-agent CCREP-ratcheted desktop app in just **10 days (May 29, 2026 to June 8, 2026)**. We synthesize this rapid evolution into three overarching epochs:

```
+-----------------------------------------------------------------------------------+
|  EPOCH I: DSP & Embeddings (Commits 1-200)                                         |
|  - Loft parallel scanner, DB setup, WaveSurfer.js player                          |
|  - ONNX local runtime: 512-d CLAP vectors & 2D D3 Music Map                       |
|  - Essentia Discogs classifiers & joint BPM/Key Fuzzy Gaussian preprocessing       |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  EPOCH II: UI Redesign, LLM Chat, & Registry (Commits 201-450)                    |
|  - "Sonic Glitch" design system & modular class-based Svelte 5 reactive stores    |
|  - Bundled Llama-Server macOS sidecar running Qwen2-Audio & 896-d blended search  |
|  - Backend modularization of Passes (PASS_REGISTRY & PassSpec)                    |
+-----------------------------------------------------------------------------------+
                                         |
                                         v
+-----------------------------------------------------------------------------------+
|  EPOCH III: SAX, Type Safety, & Team Collaboration (Commits 451-546)               |
|  - Centralized TypeScript $lib/ipc.ts and compile-safe Tauri CommandMap           |
|  - Symbolic Aggregate Approximation (SAX) structural grouping & Levenshtein edits  |
|  - Event-sourced CCREP evidence ledger & maildir-based Collab MCP network         |
+-----------------------------------------------------------------------------------+
```

### Key Architectural Inflection Points:
1. **Registry Pattern for Passes (Phase 5 of Early History)**: Migrating the analysis pipeline from an ad-hoc loop in `analysis.rs` to a trait-based `PASS_REGISTRY` unified schema versioning, database backfills, and sidecar syncs.
2. **Type-Safe Command Boundary (Phase 3 of Recent History)**: Centralizing all Tauri command dispatching into a single, typed `CommandMap` inside `$lib/ipc.ts` checked at compile time.
3. **Maildir-Based Coordination (Phase 5 of Recent History)**: Moving coordination from sequential named-pipe blockers (FIFO baton) to an asynchronous, mailbox-based MCP server (`collab_mcp`) utilizing separate git worktrees to eliminate write races.

---

## 2. Documentation and Concept Drift Audit

By comparing implementation history with documentation, we identify the following drift patterns:

*   **Scraping vs. Querying (`metrics_monitoring.md`)**: The telemetry document outlines a Prometheus-style scraping architecture. In practice, metrics are stored in a local SQLite database (`pipeline_metrics.db`) and queried dynamically by the `DevDrawer` UI. The Prometheus model is deferred.
*   **Batch Passes in `skills/add-analysis-pass/SKILL.md`**: The pipeline was upgraded from single-track iterations to include `BatchAnalysisPass` (processing all tracks at once, e.g., for SAX structural clustering). The early skill file lacked batch-pass instructions, transaction management, and runner pause/resume hooks.
*   **Spec Obsolescence vs. Backlog (`doc/proposals/` and `doc/architecture/`)**:
    *   *Clean-up*: Fully implemented specifications (like the Model Downloader or Qwen audio describer) were deleted, leaving a clean footprint.
    *   *Partial Implementation*: Proposals like `user_edit_song.md` describe wide field-level overrides, whereas the codebase only implements tag-level overrides and suppressions.
    *   *Research Trail*: `sax_structure_learning.md` proposes training neural transformers, but the code implements Viterbi path alignment.

---

## 3. Brainstorming Proposals for System Stability

To prevent architectural drift and improve agent coordination as the repo moves at AI-accelerated speed, we propose three distinct pathways:

### Proposal A: Routine Automated Concept Drift Analysis
Instead of manual reviews, we can build a lightweight scheduler to run drift detection automatically:
1. **Mechanism**: A Python cron-script or a CCREP post-commit hook that:
   * Extracts all active symbols (Tauri IPC commands, Rust `AnalysisPass` specs, Svelte stores) and active DB schemas.
   * Compares them against documentation indices (`doc/INDEX.md`, `skills/INDEX.md`).
   * Uses a local fast model (or regex/AST parsers) to flag undocumented commands, dead links, or schema mismatches.
2. **Pros**: High accuracy, fully automated, operates as a build-level quality ratchet.
3. **Cons**: Requires keeping the parser updated; might trigger false positives on experimental branches.

### Proposal B: Lightweight Glossary/Dictionary Directory
Implement a strict, single-source-of-truth directory mapping terms and concepts:
1. **Structure**: Create a `doc/glossary/` folder containing precise Markdown definitions (e.g., `sax.md`, `clap.md`, `ccrep.md`).
2. **Rule**: Every new IPC command, DB column, or analysis pass must link to a glossary term. The `generate_skill_index.py` or a custom linter can enforce this link.
3. **Pros**: Zero runtime overhead, minimal token footprint, highly readable for both humans and agents.
4. **Cons**: Relies on agents and humans maintaining it manually, though linters can verify link presence.

### Proposal C: Local Knowledge Graph Database / Codebase Index
Equip agents with an MCP service querying a local SQLite-backed knowledge graph:
1. **Structure**: An SQLite schema representing entities (`Symbol`, `File`, `Concept`, `Doc`) and relations (`IMPLEMENTS`, `CALLS`, `DOCUMENTS`, `DEP_ON`).
2. **Access**: An MCP tool `kg/query(concept)` allows agents to quickly resolve relations (e.g., `kg/query("SAX")` returns the exact parser path, DB columns, and related skills).
3. **Pros**: Saves thousands of tokens by preventing broad search/grep queries; acts as a unified semantic map of the repo.
4. **Cons**: Setup complexity; requires an indexer (e.g. running tree-sitter over commits) to update the graph on checkout/merge.

---

**→ Handoff:**
**Task:** Review the synthesized history, documentation drift, and three stability proposals.
**Context:** Compacting the early review, recent review, and synthesis logs into the canonical `session.md`.
**Deliverable:** Compacted session file and initiation of the brainstorming session.
