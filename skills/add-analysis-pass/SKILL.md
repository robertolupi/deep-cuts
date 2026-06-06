---
name: add-analysis-pass
description: Checklist and guide for adding a new analysis pass to the trait-based modular pipeline
---

# Adding a New Analysis Pass

The analysis pipeline lives in `src-tauri/src/analysis/` and offers **two pass shapes**. Pick the right one before writing any code:

| Trait | When to use |
|-------|-------------|
| **`AnalysisPass`** | Per-track compute dominates (inference, DSP, API calls). The orchestrator drives the job loop, progress events, pause/resume, and metrics automatically. |
| **`BatchAnalysisPass`** | Either (a) the algorithm requires all data at once (clustering, nearest-neighbour indexing), or (b) the pass is I/O-bound and per-track SQLite round-trips are the bottleneck. You own the read-compute-write loop; the orchestrator calls `execute()` once. |

---

## Current pass priorities

> **Important:** Priorities do **not** determine the pipeline execution order. The execution order is the explicit call sequence in `PipelineManager::run()` in `analysis/mod.rs`. Priorities only control backfill ordering and the `reset_pass` tooling.

| Pass | Priority | Trait | File |
|------|----------|-------|------|
| `audio_analysis` | 10 | `AnalysisPass` | `analysis/audio.rs` |
| `sax` | 12 | `BatchAnalysisPass` | `analysis/sax.rs` |
| `sax_alignment` | 13 | `AnalysisPass` | `analysis/sax_alignment.rs` |
| `bpm_correction` | 15 | `AnalysisPass` | `analysis/bpm_correction.rs` |
| `clap` | 20 | `AnalysisPass` | `analysis/clap.rs` |
| `essentia` | 30 | `AnalysisPass` | `analysis/essentia.rs` |
| `qwen` | 50 | `AnalysisPass` | `analysis/qwen.rs` |
| `bpm_refinement` | 55 | `AnalysisPass` | `analysis/bpm_refinement.rs` |
| `structure_cluster` | 55 | `BatchAnalysisPass` | `analysis/structure_cluster.rs` |
| `description_embed` | 60 | `AnalysisPass` | `analysis/description_embed.rs` |

Pick a priority that places your pass at the correct logical point relative to its dependencies. After choosing a priority, **also** add your pass call at the right place in `PipelineManager::run()` — that is what actually sets execution order.

---

## Adding a per-track pass (`AnalysisPass`)

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
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

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
        owned_tag_sources: &[],
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

### 4. Call the Phase in the pipeline runner

Inside `PipelineManager::run()` in `src-tauri/src/analysis/mod.rs`, insert your pass at the correct position in the explicit call sequence:
```rust
// ── Phase X: Your Pass ─────────────────────────────────────────────
log::info!("[pipeline] starting your_pass phase");
if let Err(e) = run_pass_pipeline(&app, &conn_arc, your_pass::YourPass, &run_id_spawn) {
    emit_pipeline_error(&app, "your_pass", e);
}
```

The `run_id_spawn` variable is already declared near the top of the background thread closure — do not create a new one.

### 5. Update AnalysisPanel.svelte

Add your pass to three places in `src/lib/components/AnalysisPanel.svelte`:
- `PASS_ORDER` — insert at the correct position matching `PipelineManager::run()`
- `PASS_ROLE` — assign a color family (`'audio'`, `'neural_pink'`, `'amber'`, `'green'`)
- `PASS_META` — add `{ label, description }` for the tooltip

---

## Adding a batch pass (`BatchAnalysisPass`)

Use this when your algorithm needs global data (clustering, indexing) or when individual SQLite round-trips are the bottleneck.

### 1. DB Migration

Same as per-track passes — add a migration file and register it in `database.rs`.

If your batch pass writes to a **secondary table that has no `track_id` column** (e.g. a cluster metadata table), do not list it in `owned_tables` — that generic reset path deletes by `track_id`. Use `custom_reset` instead:
```rust
custom_reset: Some(|conn| {
    conn.execute("DELETE FROM your_table", [])
        .map(|_| ())
        .map_err(|e| e.to_string())
}),
```

### 2. Create the Pass Submodule

Create `src-tauri/src/analysis/your_batch_pass.rs` and implement `BatchAnalysisPass`:

```rust
use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rayon::prelude::*;  // if compute is parallelisable
use rusqlite::Connection;

pub struct YourBatchPass;

impl super::BatchAnalysisPass for YourBatchPass {
    fn name(&self) -> &'static str { "your_batch_pass" }
    fn priority(&self) -> i32 { 60 }
    fn version(&self) -> u32 { pass_version::YOUR_BATCH_PASS }
    fn dependencies(&self) -> &'static [&'static str] { &["audio_analysis"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn needs_run(&self, conn: &Connection) -> Result<bool, String> {
        // Return true if there is work to do.
        // For a clustering pass: check if the output table is empty or
        // if new unprocessed tracks exist.
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM track_passes WHERE pass_name = 'your_batch_pass' AND status = ?1",
                       [pass_status::PENDING], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    fn execute(&self, _app: &tauri::AppHandle, conn: &Connection) -> Result<String, String> {
        // ── 1. Read all needed data in ONE query ──────────────────────────────
        let mut stmt = conn.prepare(
            "SELECT id, some_column FROM tracks WHERE some_column IS NOT NULL",
        ).map_err(|e| e.to_string())?;
        let rows: Vec<(i64, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(stmt);

        // ── 2. Compute entirely in memory (parallelise with rayon if safe) ────
        let results: Vec<(i64, MyResult)> = rows.par_iter()
            .map(|(id, col)| (*id, compute(col)))
            .collect();

        // ── 3. Write all results in ONE transaction ───────────────────────────
        conn.execute("BEGIN", []).map_err(|e| e.to_string())?;
        for (id, result) in &results {
            conn.execute(
                "UPDATE tracks SET your_column = ?1 WHERE id = ?2",
                rusqlite::params![result, id],
            ).map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
        }
        // Mark applicable track_passes rows done so AnalysisPanel shows correct progress.
        // If some tracks are not applicable, mark them done with a log explaining why;
        // do not leave them pending forever.
        conn.execute(
            "UPDATE track_passes SET status = ?1, pass_version = ?2, last_run_at = CURRENT_TIMESTAMP
             WHERE pass_name = 'your_batch_pass' AND track_id IN (SELECT id FROM tracks WHERE some_column IS NOT NULL)",
            rusqlite::params![pass_status::DONE, pass_version::YOUR_BATCH_PASS],
        ).map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

        Ok(format!("processed {} tracks", results.len()))
    }
}

impl YourBatchPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "your_batch_pass",
        priority: 60,
        version: pass_version::YOUR_BATCH_PASS,
        dependencies: &["audio_analysis"],
        owned_columns: &["your_column"],
        owned_tables: &[],          // use custom_reset if your table has no track_id
        owned_tag_sources: &[],
        custom_reset: None,
    };
}
```

### 3. Register the Submodule & SPEC

Same as per-track: declare `pub mod your_batch_pass;` in `mod.rs`, add `YourBatchPass::SPEC` to `PASS_REGISTRY`.

### 4. Call the Phase in the pipeline runner

Use `run_batch_pass` instead of `run_pass_pipeline`:
```rust
// ── Phase X: Your Batch Pass ───────────────────────────────────────
log::info!("[pipeline] starting your_batch_pass phase");
if let Err(e) = run_batch_pass(&app, &conn_arc, your_batch_pass::YourBatchPass, &run_id_spawn) {
    emit_pipeline_error(&app, "your_batch_pass", e);
}
```

### 5. Update AnalysisPanel.svelte

Same as per-track — add to `PASS_ORDER`, `PASS_ROLE`, and `PASS_META`.

---

## Pause / Resume support

The pipeline supports both manual and automatic pause, controlled by two global atomics in `analysis/mod.rs`:

```rust
pub static ANALYSIS_MANUALLY_PAUSED: AtomicBool;
pub static ANALYSIS_AUTO_PAUSED:     AtomicBool;
```

**Passes using `run_pass_pipeline` get pause/resume for free.** The default `run_pass` implementation polls both flags between jobs:

```rust
while ANALYSIS_MANUALLY_PAUSED.load(Ordering::SeqCst)
    || ANALYSIS_AUTO_PAUSED.load(Ordering::SeqCst)
{
    std::thread::sleep(std::time::Duration::from_millis(200));
}
```

**Custom passes that override `run_pass`** (e.g. `ClapPass`, `EssentiaPass`, `AudioPass`) must add the same poll loop themselves at each job-dispatch site. See `src-tauri/src/analysis/audio.rs` for the pattern.

**`BatchAnalysisPass` passes do NOT get automatic pause/resume** — `execute()` is a single call and the orchestrator cannot interrupt it mid-flight. For long-running batch passes (e.g. building a large distance matrix), check the pause atomics at natural breakpoints:
```rust
if crate::analysis::ANALYSIS_MANUALLY_PAUSED.load(std::sync::atomic::Ordering::SeqCst) {
    return Err("paused".to_string());
}
```

---

## Metrics instrumentation

Every pass automatically gets per-track metrics logged to the metrics database via `crate::metrics_database::log_pipeline_metric(...)`. This is called inside the default `run_pass` implementation in `analysis/mod.rs` for passes that use `run_pass_pipeline`.

**Custom passes** (like `ClapPass` and `EssentiaPass`) that override `run_pass` must call `log_pipeline_metric` themselves at each success/failure site. See `src-tauri/src/analysis/clap.rs` for the pattern.

**Batch passes** emit a single summary event (`analysis-phase-complete`) via `run_batch_pass` rather than per-track metrics. The infrastructure is the same — `run_id` is forwarded automatically.

The `run_id` parameter threads through the entire pipeline so that all spans from a single invocation share the same `run_id` in `pipeline_metrics`. Always forward `run_id` — never generate a new one inside a pass.

To inspect the metrics after a run, see the `query-metrics-db` skill or use the in-app Metrics Inspector (Library Settings → Inspect Pipeline Metrics).

---

## Common mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Forgetting to register in `PASS_REGISTRY` | Pass is skipped, resets do not work, and sidecar does not back up your new fields | Append your pass `SPEC` to `PASS_REGISTRY` in `analysis/mod.rs` |
| Performing DB queries inside `execute_job` | Locking issues or thread contention on heavy work | Load all needed fields upfront inside your `load_jobs` implementation |
| Creating a new `run_id` inside a pass | Metrics for that pass appear as a separate run in the Metrics Inspector | Forward the `run_id` passed into `run_pass` / `run_pass_pipeline` |
| Overriding `run_pass` without calling `log_pipeline_metric` | Pass has no latency data in the Metrics Inspector | Call `log_pipeline_metric` at every success/failure branch, like `ClapPass` does |
| Assuming `priority` controls execution order | Pass runs in the wrong phase; dependencies not satisfied | Priority only controls backfill ordering. Set execution order by inserting your `run_pass_pipeline` / `run_batch_pass` call at the right place in `PipelineManager::run()` |
| Using `AnalysisPass` for a clustering or embedding-index pass | Must process all tracks but trait forces one-at-a-time | Switch to `BatchAnalysisPass` |
| Using `AnalysisPass` for a fast per-track pass that reads large blobs | App freezes during analysis; thousands of SQLite round-trips | Convert to `BatchAnalysisPass`: one SELECT + one transaction replaces N×3 round-trips |
| Doing per-track DB writes inside `BatchAnalysisPass::execute()` | Defeats the purpose; still causes lock contention | Collect all results in memory, write in a single `BEGIN`/`COMMIT` block |
| Forgetting to `UPDATE track_passes` to DONE in a batch pass | AnalysisPanel shows all tracks permanently pending for this pass | Add a bulk `UPDATE track_passes SET status = DONE WHERE pass_name = '...'` inside the write transaction |
| Returning early from a batch pass when there is not enough data | Same batch pass reruns forever | Mark pending rows done/skipped with a clear `log` explaining why no output was produced |
| Using `filter_map(|r| r.ok())` on DB rows | Corrupt rows or schema drift disappear silently | Use `collect::<Result<Vec<_>, _>>()` and return/log the mapping error |
| Listing a no-`track_id` table in `owned_tables` | `reset_pass` fails with "no such column: track_id" | Use `owned_tables: &[]` + `custom_reset: Some(...)` to delete the table manually |
| Emitting per-track progress events from a batch pass | UI receives thousands of events at once | Emit one event before and one after via `run_batch_pass`; add coarse checkpoints only for very large compute phases |
