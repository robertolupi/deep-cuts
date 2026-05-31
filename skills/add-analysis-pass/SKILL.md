---
name: add-analysis-pass
description: Checklist and guide for adding a new analysis pass to the trait-based modular pipeline
---

# Adding a New Analysis Pass

The analysis pipeline lives in `src-tauri/src/analysis/` and is driven by the type-safe `AnalysisPass` and `PassJob` traits, and orchestrated by the static `PASS_REGISTRY` in `src-tauri/src/analysis/mod.rs`.

---

## Current pass priorities

| Pass | Priority | File |
|------|----------|------|
| `audio_analysis` | 10 | `src-tauri/src/analysis/audio.rs` (DSP) |
| `bpm_correction` | 15 | `src-tauri/src/analysis/bpm_correction.rs` (Genre-based coarse BPM) |
| `clap` | 20 | `src-tauri/src/analysis/clap.rs` (ONNX Audio embeddings) |
| `qwen` | 30 | `src-tauri/src/analysis/qwen.rs` (Prose/Tags LLM completion) |
| `description_embed` | 40 | `src-tauri/src/analysis/description_embed.rs` (Prose sentence embeddings) |
| `essentia` | 50 | `src-tauri/src/analysis/essentia.rs` (ONNX EffNet classifiers) |
| `bpm_refinement` | 55 | `src-tauri/src/analysis/bpm_refinement.rs` (Genre-based fine BPM) |

Pick a priority that places your pass at the correct logical point in this pipeline sequence.

---

## Step-by-step

### 1. DB Migration

Add columns to the `tracks` table or create new secondary tables in a new migration file:
```
src-tauri/migrations/NN_your_pass.sql
```
Register it in `src-tauri/src/database.rs`:
```rust
M::up(include_str!("../migrations/NN_your_pass.sql")),
```

### 2. Create the Pass Submodule

Create a new file `src-tauri/src/analysis/your_pass.rs`. 

1. Define a struct for your pass jobs and implement the `PassJob` trait:
```rust
pub struct YourJob {
    pub pass_id: i64,
    pub track_id: i64,
    // Add other fields you need spooled upfront
}

impl super::PassJob for YourJob {
    fn pass_id(&self) -> i64 { self.pass_id }
    fn track_id(&self) -> i64 { self.track_id }
}
```

2. Define a struct for your pass and implement the `AnalysisPass` trait:
```rust
use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct YourPass;

impl super::AnalysisPass for YourPass {
    type Job = YourJob;
    type Output = YourResultType;

    fn name(&self) -> &'static str {
        "your_pass_name"
    }

    fn priority(&self) -> i32 {
        60 // Select appropriate priority
    }

    fn version(&self) -> u32 {
        pass_version::YOUR_PASS // Add constant in scanner/sidecar.rs
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["clap"] // Declare upstream dependency pass names
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &["your_column_name"] // Columns to null out automatically on reset
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &[] // Secondary table rows to delete on reset
    }

    fn custom_reset(&self, conn: &Connection) -> Result<(), String> {
        // Optional custom reset hook
        Ok(())
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        // Spool all pending jobs upfront in a single clean query!
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id FROM track_passes tp
             WHERE tp.status = ?1 AND tp.pass_name = 'your_pass_name'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(YourJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        // Perform the heavy computation. Keep this DB query-free!
        Ok(do_heavy_work(job))
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        // Write the result back to the database
        conn.execute(
            "UPDATE tracks SET your_column_name = ?1 WHERE id = ?2",
            rusqlite::params![output, job.track_id],
        ).map_err(|e| e.to_string())?;

        // Proactively save metadata sidecar to disk
        if let Err(e) = crate::scanner::sidecar::save(conn, job.track_id) {
            log::error!("[your_pass] Failed to save sidecar: {}", e);
        }
        Ok(())
    }
}
```

3. Expose the compile-time `SPEC` constant on your pass struct:
```rust
impl YourPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "your_pass_name",
        priority: 60,
        version: pass_version::YOUR_PASS,
        dependencies: &["clap"],
        owned_columns: &["your_column_name"],
        owned_tables: &[],
        custom_reset: None,
    };
}
```

### 3. Register the Submodule & SPEC

1. Declare your new submodule in `src-tauri/src/analysis/mod.rs`:
   ```rust
   pub mod your_pass;
   ```
2. Add your pass `SPEC` to the static `PASS_REGISTRY` slice:
   ```rust
   pub static PASS_REGISTRY: &[PassSpec] = &[
       audio::AudioPass::SPEC,
       // ...
       your_pass::YourPass::SPEC,
   ];
   ```

Adding the `SPEC` to `PASS_REGISTRY` automatically handles:
- **Automatic Backfilling**: Seeds missing pass rows for all existing tracks.
- **Stale Invalidation**: Auto-resets records if the algorithm version increases.
- **Dynamic sidecars**: Automatically includes new columns and tables in `.dc.json` export/import without any extra manual Serde updates!
- **Dynamic IPC Resets**: Automatically exposes the pass to the global and individual `reset_pass` endpoints.

### 4. Call the Phase Submodule in the pipeline runner

Inside `PipelineManager::run()` background loop in `src-tauri/src/analysis/mod.rs`, invoke your pass using the generic `run_pass_pipeline` runner:
```rust
// ── Phase X: Your Pass ─────────────────────────────────────────────
log::info!("[pipeline] starting your_pass phase");
if let Err(e) = analysis::run_pass_pipeline(&app, &conn_arc, your_pass::YourPass) {
    analysis::emit_pipeline_error(&app, "your_pass", e);
}
log::info!("[pipeline] your_pass phase done");
```

---

## Common mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Forgetting to register in `PASS_REGISTRY` | Pass is skipped, resets do not work, and sidecar does not back up your new fields | Append your pass `SPEC` to `PASS_REGISTRY` in `analysis/mod.rs` |
| Performing DB queries inside `execute_job` | Locking issues or thread contention on heavy work | Load all needed fields upfront inside your `load_jobs` implementation |
