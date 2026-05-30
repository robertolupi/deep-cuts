---
name: add-analysis-pass
description: Checklist and gotchas for adding a new analysis pass to the deep-cuts pipeline
---

# Adding a New Analysis Pass

The analysis pipeline lives in `src-tauri/src/analysis.rs`. Passes run sequentially
in priority order after `audio_analysis` (priority 10) and `clap` (priority 20).

---

## Current pass priorities

| Pass | Priority | File |
|------|----------|------|
| `audio_analysis` | 10 | `analysis.rs` Phase 1 |
| `clap` | 20 | `analysis.rs` Phase 2 |
| `essentia` | 50 | `analysis.rs` Phase 3 |

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

### 2. Backfill the pass row — **in `PipelineManager::run()`, alongside the other passes**

This is critical. Add the `INSERT OR IGNORE` for your new pass in the same block
as `audio_analysis` and `clap`, **before** the early-exit gate:

```rust
conn.execute(
    "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
     SELECT id, 'your_pass', <priority>, ?1 FROM tracks",
    [pass_status::PENDING],
).map_err(|e| e.to_string())?;
```

**⚠️ Do NOT backfill inside your phase function.** If you backfill there, the
early-exit check fires before your pass rows exist, so "Run Analysis" returns
immediately when all prior passes are done.

### 3. Update the early-exit gate

The gate in `PipelineManager::run()` currently reads:

```rust
let has_pending_passes = conn.query_row(
    "SELECT EXISTS(SELECT 1 FROM track_passes WHERE status = ?1)",
    [pass_status::PENDING],
    |row| row.get(0),
).unwrap_or(false);

if total == 0 && !has_pending_passes {
    return Ok(());
}
```

This checks all passes generically — no change needed as long as step 2 is done
(rows exist before the gate fires).

### 4. Add a phase function

Add a `run_your_pass_phase(app, conn_arc)` function and call it from the background
thread at the end of `PipelineManager::run()`, after the previous phase completes:

```rust
run_your_pass_phase(&app, &conn_arc);
```

The phase function pattern:

```rust
fn run_your_pass_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<SpoolJob> = {
        let conn = conn_arc.lock().unwrap();
        // query pending jobs, mark IN_PROGRESS
        ...
    };

    for job in jobs {
        let start = std::time::Instant::now();
        let result = /* do work */;
        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = conn_arc.lock().unwrap();

        match result {
            Ok(data) => {
                // UPDATE tracks SET ... WHERE id = ?
                conn.execute("UPDATE track_passes SET status = ?1, duration_ms = ?2,
                    last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                    rusqlite::params![pass_status::DONE, elapsed_ms, job.pass_id])?;
                app.emit("analysis-progress", json!({
                    "track_id": job.track_id,
                    "pass_name": "your_pass",
                    "status": pass_status::DONE,
                })).ok();
            }
            Err(e) => {
                conn.execute("UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                    last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id])?;
                app.emit("analysis-progress", json!({
                    "track_id": job.track_id,
                    "pass_name": "your_pass",
                    "status": pass_status::FAILED,
                })).ok();
            }
        }
    }

    app.emit("analysis-phase-complete", json!({ "pass": "your_pass" })).ok();
}
```

### 5. Add `reset_pass` support in `src-tauri/src/commands/analysis.rs`

```rust
if pass_name == "your_pass" {
    conn.execute("UPDATE tracks SET your_col = NULL, ...", [])
        .map_err(|e| e.to_string())?;
}
```

### 6. Build and test

```bash
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

---

## Common mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Backfill inside phase fn, not in `run()` | "Run Analysis" does nothing when prior passes are done | Move `INSERT OR IGNORE` to `PipelineManager::run()` |
| Using a pass-specific check in the early-exit gate | New pass skipped when old passes are done | Use the generic `WHERE status = ?1` check (no pass_name filter) |
| Not marking jobs `IN_PROGRESS` before processing | Jobs re-queued if app restarts mid-run | Set `status = IN_PROGRESS` when loading the job batch |
