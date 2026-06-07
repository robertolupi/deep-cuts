---
status: active
owner: Roberto
last_verified: 2026-06-07
---

# Deep Cuts — Architecture Map

A concise reference connecting all major subsystems. Use this when assessing the blast radius of a change, onboarding an agent, or finding where a feature lives.

---

## 1. Top-Level Data Flow

```
Filesystem
  └─► LibraryScanner (scanner/)
        ├─ detects new / changed / deleted files
        ├─ reads metadata with lofty (scanner/metadata.rs)
        └─ writes/updates rows in `tracks` table (scanner/db.rs)
              └─► Analysis pipeline (analysis/)
                    ├─ one `track_passes` row per track per pass
                    ├─ passes write results back to `tracks` columns
                    │   and to virtual tables (audio_embeddings, etc.)
                    └─► Tauri IPC events → Frontend stores → UI
```

On startup (`lib.rs:run`) the app: loads sqlite-vec as a global SQLite extension, initialises both databases, spawns `scan_all_libraries` in the background, and registers all IPC handlers.

---

## 2. Scanner (`src-tauri/src/scanner/`)

Entry point: `scanner::scan_all_libraries(AppHandle)` — registered as an IPC command and called automatically on startup.

| File | Responsibility |
|---|---|
| `mod.rs` | `LibraryScanner::scan` orchestrator; emits `scan:progress` events; manages watched-directory loop |
| `fs.rs` | Recursive filesystem walk; filters by audio extension |
| `metadata.rs` | Reads embedded tags (title, artist, BPM, …) via `lofty` |
| `db.rs` | Upserts tracks, marks deleted files, flags changed tracks as `is_stale = 1` |
| `sidecar.rs` | Reads/writes `.dc.json` sidecar files; `SUFFIX = ".dc.json"` |

After scanning, stale tracks gain pending `track_passes` rows that the analysis pipeline will process.

---

## 3. Database

### Main DB — `deep-cuts.db` (app data dir)

Managed by `DbManager` (`database.rs`). Migration system: `rusqlite_migration` — sequential `M` structs compiled into the binary. Fatal on startup failure.

Key tables:

| Table | Purpose |
|---|---|
| `watched_directories` | Root paths the user has added to the library |
| `tracks` | One row per audio file; contains metadata, analysis results, and ML columns |
| `track_passes` | Per-track per-pass status (`pending/in_progress/done/failed`), duration, log, pass version |
| `track_tags` | Many-to-many tags with `source` (who wrote it) and `score` |
| `tags` | Canonical tag names with `namespace:label` format |
| `audio_embeddings` | sqlite-vec virtual table — 512-dim CLAP embeddings |
| `description_embeddings` | sqlite-vec virtual table — sentence-transformer embeddings of Qwen descriptions |
| `structure_clusters` | Batch-pass output; structural cluster assignments |
| `playlists` / `playlist_tracks` | User playlists |
| `saved_searches` | Saved filter presets |
| `app_settings` | Key/value store (theme, update preferences, model path, etc.) |

Key structs: `Track`, `WatchedDirectory` (both in `database.rs`, both `Serialize/Deserialize`).

Startup recovery: any `track_passes` rows stuck at `in_progress` (crash) are reset to `null` acoustid_status on boot.

### Metrics DB — `deep-cuts-metrics.db` (app data dir)

Managed by `MetricsDbManager` (`metrics_database.rs`). Optional — app degrades gracefully if it fails to open. Stores per-job timing traces (`pipeline_metrics`) and system events (`system_events`), surfaced in the Dev Inspector.

---

## 4. Analysis Pipeline (`src-tauri/src/analysis/`)

### Traits

- **`AnalysisPass<R>`** — per-track sequential pass. Implements `load_jobs`, `execute_job`, `save_result`. Default `run_pass` loop handles pause polling, timing, metrics logging, and sidecar save.
- **`BatchAnalysisPass`** — operates on all tracks at once (global data or I/O-bound). Implements `needs_run` + `execute`.

### `PASS_REGISTRY` — canonical ordered list of all passes

```
audio_analysis    → audio::AudioPass
bpm_correction    → bpm_correction::BpmCorrectionPass
sax               → sax::SaxPass               (BatchAnalysisPass)
sax_alignment     → sax_alignment::SaxAlignmentPass
clap              → clap::ClapPass
essentia          → essentia::EssentiaPass
bpm_refinement    → bpm_refinement::BpmRefinementPass
structure_cluster → structure_cluster::StructureClusterPass (BatchAnalysisPass)
qwen              → qwen::QwenPass
description_embed → description_embed::DescriptionEmbedPass
```

`PassSpec` records each pass's `name`, `priority`, `version`, `dependencies`, `owned_columns`, `owned_tables`, and `owned_tag_sources`. Resetting a pass cascades automatically to all dependents (recursive).

### `PipelineManager::run`

Runs on a background thread (IPC returns immediately). Execution order (from `analysis/mod.rs`):

1. audio_analysis (parallel workers, count from `hardware::PipelineConfig::auto_tune`)
2. bpm_correction
3. sax (batch)
4. sax_alignment
5. clap
6. essentia
7. structure_cluster (batch)
8. bpm_refinement
9. qwen
10. description_embed

Pause/resume: `ANALYSIS_MANUALLY_PAUSED` and `ANALYSIS_AUTO_PAUSED` atomics checked per-job. A `SleepPreventer` (via `keepawake`) blocks system sleep for the pipeline duration. Progress emitted as `analysis-progress`, `analysis-phase-complete`, `analysis-complete`, `analysis-error` Tauri events.

---

## 5. IPC Command Domains (`src-tauri/src/commands/`)

| File | Domain |
|---|---|
| `config.rs` | Theme, model path, AcoustID key, sidecar toggle, update settings |
| `library.rs` | Watched dirs, track CRUD, tags, cover art, semantic search, sidecar export |
| `analysis.rs` | Run/pause pipeline, pass stats, reset passes, model existence check |
| `map.rs` | UMAP projection, audio similarity search, duplicate detection |
| `manifest.rs` | Fetch remote model manifest, update check settings |
| `download.rs` | Model download lifecycle (`DownloadState`, resume, cancel, status) |
| `chat.rs` | Qwen chat sessions and messages (stored in main DB) |
| `playlists.rs` | Playlists, saved searches, M3U export |
| `statistics.rs` | Aggregate track stats |
| `metrics.rs` | Pipeline metrics summary and run traces (reads metrics DB) |
| `structure.rs` | Structure cluster queries |
| `debug.rs` | Raw track debug dump (debug builds only) |

---

## 6. Model Manifest & Download

`models/manifest.json` is compiled into the binary as a fallback via `include_str!`. At runtime `fetch_app_manifest` fetches the live version from GitHub (`ModelManifest::MANIFEST_URL`), compares `manifest_version` against the fallback, and returns an `update_available` flag.

`ModelManifest` maps model keys (`qwen`, `clap`, `sentence`, `essentia`) to `ModelGroup { label, files: [{ filename, url, sha256, size_bytes }] }`.

Downloads are managed by `commands::download` with `DownloadState` (Tauri managed state). Models land in the user-configured path (stored in `app_settings`). SHA-256 is verified after download. Partial downloads can be resumed.

---

## 7. Frontend Stores (`src/lib/stores/`)

All stores use Svelte 5 runes (`$state`, `$derived`). Suffix `.svelte.ts` denotes a runes module.

| File | Responsibility |
|---|---|
| `library.svelte.ts` | Track list, watched directories, tags, cover art cache; primary data layer |
| `filters.svelte.ts` | Active filter state (search text, tag filters, sort order) |
| `player.svelte.ts` | Playback state — current track, play/pause, queue |
| `ui.svelte.ts` | Panel visibility, sidebar state, active view, modal state |
| `theme.svelte.ts` | Current theme (light/dark/system); persisted via IPC |
| `curation.svelte.ts` | Tag curation mode state |
| `structureClusters.svelte.ts` | Structure cluster data for the visualisation |
| `devInspector.svelte.ts` | Dev Inspector panel state (metrics, pipeline traces) |

---

## 8. IPC Boundary (`src/lib/ipc.ts`)

Single thin wrapper around `@tauri-apps/api/core`. All frontend code imports `invoke` and `listen` from `$lib/ipc` — never directly from `@tauri-apps/api`.

**Mock system**: if `?local_debug` is present in the URL, `invoke` is intercepted and served from `MOCK_RESPONSES` (static fixtures in `$lib/mock-data`). This lets the frontend run in a browser without a Tauri backend. Unknown commands log a warning and resolve to `undefined`.

`listen` in mock mode returns a no-op unlisten function — no real events fire.

---

## 9. Sidecar Export/Restore (`src-tauri/src/scanner/sidecar.rs`)

A `.dc.json` file written alongside each audio file (e.g. `track.mp3.dc.json`). Contains:

- `version` — sidecar schema version
- `pass_versions` — map of pass name → algorithm version at write time
- `pass_run_times` — map of pass name → timestamp
- `user_tags` / `suppressed_tags` — user edits that must survive a DB wipe
- `ml_metadata` — flattened map of all ML columns owned by registered passes

Sidecar writes are triggered automatically after a successful pass result (if the sidecar setting is enabled). Bulk export available via `commands::library::export_sidecars`. On rescan, the sidecar is read back to restore analysis results, avoiding redundant re-processing.

---

## 10. Other Notable Modules

| Module | Purpose |
|---|---|
| `acoustid.rs` | MusicBrainz AcoustID fingerprint lookup (debug builds; batch + single-track) |
| `llama.rs` | Manages the llama-server child process for Qwen chat; `LlamaServerState` holds PID + port |
| `hardware.rs` | `PipelineConfig::auto_tune()` — detects CPU/RAM to set decode thread count |
| `embeddings.rs` | Shared helpers for ONNX-based sentence-transformer inference |
| `spectrogram.rs` | Spectrogram generation for waveform display |
| `dsp.rs` | DSP utilities shared across passes |
| `bpm.rs` | BPM detection primitives |
| `classifier.rs` | Shared classifier inference helpers |
| `metrics_database.rs` | Metrics DB schema, `log_pipeline_metric`, `log_system_event` |
| `error.rs` | `AppError` enum — unified IPC error type |
