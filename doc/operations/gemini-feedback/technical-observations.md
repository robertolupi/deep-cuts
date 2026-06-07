# Technical Observations & Recommendations

Date: 2026-06-07
Author: Gemini 3.5 Flash (Medium)

This document contains a detailed technical review of the Deep Cuts codebase, focusing on database serialization, frontend architecture, model orchestration, and testing.

---

## 1. Database & DTO Coupling (Critical Risk)

### The Issue: Positional Mapping (`row.get(index)`)
Throughout the Tauri backend (e.g., [database.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/database.rs#L125-L203) and [library.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/commands/library.rs#L163-L167)), tracks are fetched via raw SQL and mapped to the `Track` struct using hardcoded column indices:

```rust
pub fn find_all(conn: &Connection) -> Result<Vec<Self>, rusqlite::Error> {
    let mut stmt = conn.prepare("SELECT id, watched_directory_id, path, filename, ... FROM tracks ...")?;
    let rows = stmt.query_map([], |row| {
        Ok(Self {
            id: row.get(0)?,
            watched_directory_id: row.get(1)?,
            path: row.get(2)?,
            // ... up to index 52
        })
    })?;
    // ...
}
```

This pattern creates a severe maintenance bottleneck:
1. **Schema Drift:** If a developer adds a column in a migration (30 migrations have already run in 9 days) and inserts it in the middle of a `SELECT` query, all subsequent indices shift.
2. **Silent Corruption:** If two columns share the same database type (e.g., two `Option<String>` fields or two `Option<f64>` fields like `mood_happy` and `mood_sad`) and their select order is swapped, Rust compiles successfully, but the fields map to the wrong variables.
3. **Code Duplication:** This mapping block is duplicated across multiple queries in `database.rs`, `library.rs`, and `map.rs`.

### Recommendations:
* **Adopt Named Column Mapping:** Refactor query mapping to use column names rather than indices (e.g., `row.get::<_, Option<f64>>("bpm")?`). While slightly slower than positional indexing, it is compile-safe and resilient to query reordering.
* **Integrate a Mapper Crate:** Consider integrating `serde_rusqlite` or writing a light macro that maps database rows directly into the `Track` DTO.
* **Decouple the `Track` Struct:** Split the monolithic `Track` struct into sub-DTOs. A list query should only fetch identity and standard tag metadata (e.g., `TrackHeader`). Acoustic metrics (CLAP), LLM descriptions (Qwen), and structural alignments (SAX) should be fetched on-demand when inspecting a specific track.

---

## 2. Frontend Component Bloat (Maintenance Risk)

### The Issue: Large Component Files
The frontend utilizes Svelte 5 runes (`$state`, `$derived`, `$effect`) effectively. However, several critical components are massive:
* `FilterSidebar.svelte` (~1,260 lines, 43.4KB)
* `TrackDetailPane.svelte` (~1,300 lines, 46KB)
* `StatisticsPanel.svelte` (~1,400 lines, 46.7KB)
* `MusicMap.svelte` (~1,250 lines, 42.1KB)

These components are difficult to audit because they mix:
1. Svelte component markup.
2. Complex UI interaction state (e.g., autocomplete, modals, playlist creation).
3. Data processing (e.g., histogram math, coordinate scaling).
4. Long CSS blocks (representing up to 50% of the file size).

### Recommendations:
* **Decompose Component Views:** Break massive components into isolated, single-responsibility sub-components.
  * `FilterSidebar.svelte` -> `SearchHeader.svelte`, `KeySelector.svelte`, `BpmRangeSlider.svelte`, `MoodSliders.svelte`.
  * `TrackDetailPane.svelte` -> `SpectrogramCard.svelte`, `TagEditor.svelte`, `SimilarTracksList.svelte`.
* **Separate CSS Styles:** Move long styling blocks into global CSS variables or scoped CSS files if they exceed 100 lines. Keep component files focused on logic and structure.
* **Extract Utility Functions:** Mathematical calculations (such as `makeHistogram` in `FilterSidebar.svelte`) should be extracted into separate utility modules under `src/lib/utils` where they can be unit-tested in isolation.

---

## 3. Local Model Downloader & Progressive Degradation

### The Issue: 6GB Setup Barrier
The application relies on several local AI models. If a user starts the app, they are faced with a massive model download. If they skip the download or it fails:
1. The analysis pipeline fails or stalls.
2. The user experience degrades entirely.

### Recommendations:
* **Progressive Pipeline Degradation:** Enable the pipeline to run even if only subset models are present.
  * If only the core DSP code works (no models): support file scanning, BPM/Key detection, and basic filtering.
  * If CLAP and Essentia are downloaded: enable acoustic search, mood radar, and UMAP maps.
  * If Qwen2-Audio (5GB) is downloaded: unlock the semantic chat and descriptions.
* **Download Resilience:** Wrap download operations in resilient chunked downloads with checksum verification, ensuring interrupted downloads can resume gracefully without corrupting the GGUF/ONNX files.

---

## 4. Test Coverage Gaps

### The Issue: Frontend vs. Backend Disparity
The Rust backend has excellent unit tests covering calculations, migrations, and analysis invariants. However, the Svelte 5 frontend has very few tests.
1. Major components (`MusicMap.svelte`, `ChatPanel.svelte`, `TrackDetailPane.svelte`) have zero test coverage.
2. Complex store operations (`curation.svelte.ts`, `filters.svelte.ts`) are primarily tested manually.

### Recommendations:
* **Write Component Unit Tests:** Use `@testing-library/svelte` and Vitest to write tests for autocomplete fields, range sliders, and sidebar tabs.
* **Mock Tauri IPC Commands:** Leverage `@tauri-apps/api/mocks` to simulate Tauri commands during Vitest execution, ensuring frontend components can be validated headlessly without compiling the Rust library.
* **Add Pipeline Invariant Tests:** Expand backend testing to verify that if a pass fails, the pipeline correctly recovers or pauses, rather than stalling the spool thread.

---

## 5. Prototyping Branch Hygiene

### The Issue: Git Churn
The git history shows that experimental concepts are being committed directly to the `main` branch. For example, migration `25_waveform_fingerprint.sql` was added, and then immediately dropped in migration `29_drop_waveform_fingerprint.sql`. 
While this is fine for an early 9-day project, it pollutes the production migration history and makes database schema recovery complex.

### Recommendations:
* **Isolate Experimental Code:** Run experiments in the `tools/` directory (using the Python scripts) or on temporary feature branches.
* **Consolidate Migrations:** Before making a release, squash intermediate schema modifications into single migrations to keep the database migration history clean and performant.

---

## 6. Proposed Linting Infrastructure (Planned Action Items)

To establish robust quality guardrails and enforce project conventions automatically, the following linting tools are proposed for implementation in future sessions:

### A. Backend & Database Linters
* **Clippy Enforcement (`npm run clippy`):** Address the current 58 compiler warnings in the Rust codebase and configure `cargo clippy -- -D warnings` as a pre-commit or dev startup block.
* **Migration Registration Linter (`tools/lint_migrations.py`):** Automatically scan the `src-tauri/migrations/` folder and verify that every `.sql` file is correctly imported and ordered inside `get_migrations()` in `database.rs`. This prevents runtime startup crashes from unregistered database changes.
* **Brittle Query Checker:** A Python-based linter that parses SQL queries in the Rust codebase and flags occurrences of `row.get(index)` for queries retrieving more than 5 fields, suggesting named column mapping or struct serializers.

### B. Frontend Svelte Linters
* **Component Size & Style Bloat Linter (`tools/lint_svelte_bloat.py`):** Scan `src/lib/components/` and raise warnings or fail checks when:
  1. A Svelte component exceeds 600 lines total.
  2. A `<style>` block exceeds 150 lines.
  This prevents the continuation of massive single-file components and encourages style extraction and UI decomposition.
* **CSS Color Token Checker (`tools/lint_css_colors.py`):** Scan Svelte components (`.svelte` files) and stylesheets (`.css` files) for hardcoded hex colors (e.g., `#fff`, `#1e1f25`), `rgb()`, `rgba()`, or named colors (like `red`, `blue`). It should assert that all visual styles rely on `--sg-*` theme variables or JS-based `getCssToken()` lookups, preventing theme bypasses that break dark/light mode compatibility.
* **ESLint for Svelte 5 Runes:** Configure `eslint-plugin-svelte` with Svelte 5 configurations to statically analyze reactive structures and catch rune misuse (e.g., recursive `$effect` mutations) before execution.

### C. Repo Hygiene Linters
* **Link Checker Automation (`tools/lint_links.py`):** Convert the manual markdown link checker script into a formal project check.
* **Skill Index Synchronizer:** Ensure that any additions or changes to skills under `skills/` trigger an error if the developer forgot to run the `generate_skill_index.py` script.

