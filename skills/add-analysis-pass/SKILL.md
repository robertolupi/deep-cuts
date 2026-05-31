---
name: add-analysis-pass
description: Checklist and gotchas for adding a new analysis pass to the deep-cuts pipeline
---

# Adding a New Analysis Pass

The analysis pipeline lives in `src-tauri/src/analysis/` and is orchestrated by the unified `PASS_REGISTRY` spec list inside `src-tauri/src/analysis/mod.rs`.

---

## Current pass priorities

| Pass | Priority | File |
|------|----------|------|
| `audio_analysis` | 10 | `src-tauri/src/analysis/audio.rs` Phase 1 |
| `bpm_correction` | 15 | `src-tauri/src/analysis/bpm_correction.rs` Phase 1b — coarse metadata genre |
| `clap` | 20 | `src-tauri/src/analysis/clap.rs` Phase 2 |
| `qwen` | 30 | `src-tauri/src/analysis/qwen.rs` Phase 3 |
| `description_embed` | 40 | `src-tauri/src/analysis/description_embed.rs` Phase 4 |
| `essentia` | 50 | `src-tauri/src/analysis/essentia.rs` Phase 5 |
| `bpm_refinement` | 55 | `src-tauri/src/analysis/bpm_refinement.rs` Phase 6 |

Pick a priority that places your pass at the right point in that sequence.

---

## Step-by-step

### 1. DB migration

Add columns to `tracks` and/or new tables in a new migration file:
```
src-tauri/migrations/NN_your_pass.sql
```
Register it in `src-tauri/src/database.rs`:
```rust
M::up(include_str!("../migrations/NN_your_pass.sql")),
```

### 2. Register the pass spec in the global `PASS_REGISTRY`

In [analysis/mod.rs](file:///Users/rlupi/src/deep-cuts/src-tauri/src/analysis/mod.rs), add your pass to the static `PASS_REGISTRY` slice. This automatically handles:
- **Automatic Backfilling**: Adds pending pass rows for all existing tracks.
- **Stale Version Invalidation**: Detects algorithm/model updates and automatically resets completed passes if the code version increases.
- **Generic Cascaded Resets**: Handles column clearing, table deletion, and downstream dependent pass resets automatically when this or upstream passes are reset!

```rust
PassSpec {
    name: "your_pass",
    priority: 60, // set appropriate priority
    version: pass_version::YOUR_PASS, // add to scanner/sidecar pass_version
    dependencies: &["upstream_pass"], // declare dependent pass names
    owned_columns: &["your_new_column"], // columns to null on reset
    owned_tables: &[], // table rows to delete on reset
    custom_reset: None, // Option<fn(&rusqlite::Connection) -> Result<(), String>>
}
```

### 3. Create a dedicated pass submodule

Create a new file `src-tauri/src/analysis/your_pass.rs`. 
Define the phase function inside it:

```rust
use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub fn run_your_pass_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<super::SpoolJob> = {
        let conn = match super::lock_analysis_conn(conn_arc, "your_pass") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "your_pass", e);
                return;
            }
        };
        // Retrieve jobs and mark as IN_PROGRESS
        ...
    };

    for job in jobs {
        let start = std::time::Instant::now();
        let result = /* do work */;
        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match super::lock_analysis_conn(conn_arc, "your_pass") {
            Ok(conn) => conn,
            Err(_) => break,
        };

        match result {
            Ok(data) => {
                // Update tracks and mark as DONE
                conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2, pass_version = ?3
                     WHERE id = ?4",
                    rusqlite::params![pass_status::DONE, elapsed_ms, pass_version::YOUR_PASS, job.pass_id]
                );
                app.emit("analysis-progress", ...);
            }
            Err(e) => {
                // Mark as FAILED
                conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3
                     WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id]
                );
                app.emit("analysis-progress", ...);
            }
        }
    }
}
```

### 4. Register and call the phase submodule in the orchestrator

1. Declare your new submodule in `src-tauri/src/analysis/mod.rs`:
   ```rust
   pub mod your_pass;
   ```
2. Call it in sequence inside `PipelineManager::run()` background thread loop:
   ```rust
   // ── Phase 7: Your Pass ─────────────────────────────────────────────
   log::info!("[pipeline] starting your_pass phase");
   your_pass::run_your_pass_phase(&app, &conn_arc);
   log::info!("[pipeline] your_pass phase done");
   ```

### 5. Build and test

```bash
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## Common mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Forgetting to register in `PASS_REGISTRY` | Pass is skipped entirely during runs and resets | Append your pass to `PASS_REGISTRY` in `analysis/mod.rs` |
| Not marking jobs `IN_PROGRESS` before processing | Jobs re-queued if app restarts mid-run | Set `status = IN_PROGRESS` when loading the job batch |
