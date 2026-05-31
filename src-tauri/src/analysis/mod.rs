use crate::database::{pass_status, DbManager};
use rusqlite::Connection;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tauri::{AppHandle, Emitter};

pub mod audio;
pub mod bpm_correction;
pub mod clap;
pub mod qwen;
pub mod description_embed;
pub mod essentia;
pub mod bpm_refinement;

static ANALYSIS_ACTIVE: AtomicBool = AtomicBool::new(false);

// RAII guard that clears ANALYSIS_ACTIVE when the pipeline scope exits
struct ActiveGuard;
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        ANALYSIS_ACTIVE.store(false, Ordering::SeqCst);
    }
}

struct SleepPreventer {
    _handle: Option<keepawake::AwakeHandle>,
}

impl SleepPreventer {
    fn new() -> Self {
        let handle = keepawake::Builder::new()
            .display(false)
            .idle(true)
            .sleep(true)
            .reason("Deep Cuts Backend Analysis")
            .create();

        match handle {
            Ok(h) => {
                log::info!("[sleep-preventer] Sleep prevention active across all platforms!");
                Self { _handle: Some(h) }
            }
            Err(e) => {
                log::warn!("[sleep-preventer] Failed to enable sleep prevention: {}", e);
                Self { _handle: None }
            }
        }
    }
}

pub struct SpoolJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub path: String,
}

pub(crate) fn emit_pipeline_error(app: &tauri::AppHandle, phase: &str, message: impl Into<String>) {
    let message = message.into();
    log::error!("[pipeline] {} failed: {}", phase, message);
    let _ = app.emit(
        "analysis-error",
        serde_json::json!({
            "phase": phase,
            "message": message,
        }),
    );
}

pub(crate) fn lock_analysis_conn<'a>(
    conn_arc: &'a Arc<Mutex<Connection>>,
    phase: &str,
) -> Result<MutexGuard<'a, Connection>, String> {
    conn_arc
        .lock()
        .map_err(|e| format!("[{}] database lock poisoned: {}", phase, e))
}

// ── Traits Definitions ─────────────────────────────────────────────────────

pub trait PassJob {
    fn pass_id(&self) -> i64;
    fn track_id(&self) -> i64;
}

#[allow(dead_code)]
pub trait AnalysisPass<R: tauri::Runtime = tauri::Wry> {
    type Job: PassJob + Send + 'static;
    type Output: Send + 'static;

    // Static Specs
    fn name(&self) -> &'static str;
    fn priority(&self) -> i32;
    fn version(&self) -> u32;
    fn dependencies(&self) -> &'static [&'static str];
    fn owned_columns(&self) -> &'static [&'static str];
    fn owned_tables(&self) -> &'static [&'static str];
    fn custom_reset(&self, _conn: &Connection) -> Result<(), String> {
        Ok(())
    }

    // Dynamic Operations
    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String>;
    fn execute_job(&self, app: &tauri::AppHandle<R>, job: &Self::Job) -> Result<Self::Output, String>;
    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        duration_ms: i64,
    ) -> Result<(), String>;

    /// Optional structured JSON string logged to `track_passes.raw_result` on success.
    /// Called before `save_result` so the output can be borrowed without affecting the move.
    fn raw_result_json(&self, _output: &Self::Output) -> Option<String> {
        None
    }

    // Setup & Teardown optional hooks
    fn setup(&self, _app: &tauri::AppHandle<R>) -> Result<(), String> {
        Ok(())
    }
    fn teardown(&self, _app: &tauri::AppHandle<R>) -> Result<(), String> {
        Ok(())
    }

    // Default Sequential Execution Loop
    fn run_pass(
        &self,
        app: &tauri::AppHandle<R>,
        conn_arc: &Arc<Mutex<Connection>>,
    ) -> Result<(), String> {
        let jobs = {
            let conn = lock_analysis_conn(conn_arc, self.name())?;
            let spooled = self.load_jobs(&conn)?;
            for job in &spooled {
                conn.execute(
                    "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                    rusqlite::params![pass_status::IN_PROGRESS, job.pass_id()],
                ).map_err(|e| e.to_string())?;
            }
            spooled
        };

        if jobs.is_empty() {
            return Ok(());
        }

        self.setup(app)?;

        for job in jobs {
            let start = std::time::Instant::now();
            let result = self.execute_job(app, &job);
            let duration_ms = start.elapsed().as_millis() as i64;

            let conn = lock_analysis_conn(conn_arc, self.name())?;
            match result {
                Ok(output) => {
                    let raw = self.raw_result_json(&output);
                    self.save_result(&conn, &job, output, duration_ms)?;
                    conn.execute(
                        "UPDATE track_passes SET status = ?1, duration_ms = ?2, pass_version = ?3, raw_result = ?4, last_run_at = CURRENT_TIMESTAMP WHERE id = ?5",
                        rusqlite::params![pass_status::DONE, duration_ms, self.version(), raw, job.pass_id()],
                    ).map_err(|e| e.to_string())?;
                    if !self.owned_columns().is_empty() {
                        if let Err(e) = crate::scanner::sidecar::save(&conn, job.track_id()) {
                            log::error!("[{}] sidecar save failed for track {}: {}", self.name(), job.track_id(), e);
                        }
                    }
                    let _ = app.emit("analysis-progress", serde_json::json!({
                        "track_id": job.track_id(),
                        "pass_name": self.name(),
                        "status": pass_status::DONE,
                    }));
                }
                Err(e) => {
                    conn.execute(
                        "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                        rusqlite::params![pass_status::FAILED, e, duration_ms, job.pass_id()],
                    ).map_err(|e| e.to_string())?;
                    let _ = app.emit("analysis-progress", serde_json::json!({
                        "track_id": job.track_id(),
                        "pass_name": self.name(),
                        "status": pass_status::FAILED,
                    }));
                }
            }
        }

        self.teardown(app)?;
        let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": self.name() }));
        Ok(())
    }
}

// Generic Runner Execution
pub fn run_pass_pipeline<R: tauri::Runtime, P: AnalysisPass<R>>(
    app: &tauri::AppHandle<R>,
    conn_arc: &Arc<Mutex<Connection>>,
    pass: P,
) -> Result<(), String> {
    pass.run_pass(app, conn_arc)
}

// ── Pass Specification & Registry ──────────────────────────────────────────

#[allow(dead_code)]
pub struct PassSpec {
    pub name: &'static str,
    pub priority: i32,
    pub version: u32,
    pub dependencies: &'static [&'static str],
    pub owned_columns: &'static [&'static str],
    pub owned_tables: &'static [&'static str],
    pub custom_reset: Option<fn(&rusqlite::Connection) -> Result<(), String>>,
}

pub static PASS_REGISTRY: &[PassSpec] = &[
    audio::AudioPass::SPEC,
    bpm_correction::BpmCorrectionPass::SPEC,
    clap::ClapPass::SPEC,
    qwen::QwenPass::SPEC,
    description_embed::DescriptionEmbedPass::SPEC,
    essentia::EssentiaPass::SPEC,
    bpm_refinement::BpmRefinementPass::SPEC,
];

// ── Generic Lifecycle Helpers ──────────────────────────────────────────────

pub fn invalidate_stale_versions(conn: &rusqlite::Connection) -> Result<(), String> {
    for spec in PASS_REGISTRY {
        conn.execute(
            "UPDATE track_passes SET status = ?1, log = NULL
             WHERE pass_name = ?2 AND status = ?3 AND pass_version < ?4",
            rusqlite::params![
                pass_status::PENDING,
                spec.name,
                pass_status::DONE,
                spec.version
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn backfill_track_passes(conn: &rusqlite::Connection) -> Result<(), String> {
    for spec in PASS_REGISTRY {
        conn.execute(
            "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
             SELECT id, ?1, ?2, ?3 FROM tracks",
            rusqlite::params![spec.name, spec.priority, pass_status::PENDING],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn reset_pass(conn: &rusqlite::Connection, pass_name: &str) -> Result<(), String> {
    let spec = PASS_REGISTRY
        .iter()
        .find(|s| s.name == pass_name)
        .ok_or_else(|| format!("Unknown pass name: {}", pass_name))?;

    // 1. Reset row itself
    conn.execute(
        "UPDATE track_passes SET status = ?1, log = NULL, result = NULL,
         last_run_at = NULL, duration_ms = NULL WHERE pass_name = ?2",
        rusqlite::params![pass_status::PENDING, pass_name],
    )
    .map_err(|e| e.to_string())?;

    // 2. Clear owned column metrics
    if !spec.owned_columns.is_empty() {
        let mut set_clauses = Vec::new();
        for col in spec.owned_columns {
            if *col == "has_long_silence" {
                set_clauses.push(format!("{} = 0", col));
            } else {
                set_clauses.push(format!("{} = NULL", col));
            }
        }
        let query = format!("UPDATE tracks SET {}", set_clauses.join(", "));
        conn.execute(&query, []).map_err(|e| e.to_string())?;
    }

    // 3. Delete owned dependent tables (e.g. vector embedding tables)
    for table in spec.owned_tables {
        let query = format!("DELETE FROM {}", table);
        conn.execute(&query, []).map_err(|e| e.to_string())?;
    }

    // 4. Run custom pass reset logic if specified
    if let Some(custom_fn) = spec.custom_reset {
        custom_fn(conn)?;
    }

    // 5. Find and recursively reset any passes that depend on this pass
    for other_spec in PASS_REGISTRY {
        if other_spec.dependencies.contains(&pass_name) {
            log::info!(
                "[reset] Pass '{}' is a dependency of '{}'. Recursively resetting '{}'.",
                pass_name,
                other_spec.name,
                other_spec.name
            );
            reset_pass(conn, other_spec.name)?;
        }
    }

    Ok(())
}

pub fn reset_all_passes(conn: &rusqlite::Connection) -> Result<(), String> {
    for spec in PASS_REGISTRY {
        reset_pass(conn, spec.name)?;
    }
    Ok(())
}

// ── Orchestrator ───────────────────────────────────────────────────────────

pub struct PipelineManager;

impl PipelineManager {
    /// Returns true if the analysis pipeline is currently active.
    pub fn is_running() -> bool {
        ANALYSIS_ACTIVE.load(Ordering::SeqCst)
    }

    /// Runs the audio analysis and embedding pipeline concurrently.
    pub fn run(app: AppHandle, conn_mutex: &Mutex<Connection>) -> Result<(), String> {
        log::info!("[pipeline] run() called");
        if ANALYSIS_ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            log::warn!("[pipeline] already running, rejecting");
            return Err("Analysis is already running".to_string());
        }
        let _guard = ActiveGuard;
        let sleep_preventer = SleepPreventer::new();

        let pending: Vec<SpoolJob> = {
            let conn = conn_mutex.lock().map_err(|e| e.to_string())?;

            // Reset interrupted/failed rows for retry
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL, last_run_at = NULL
                 WHERE status IN (?2, ?3)",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::IN_PROGRESS,
                    pass_status::FAILED
                ],
            )
            .map_err(|e| e.to_string())?;

            // Invalidate stale versions generics
            invalidate_stale_versions(&conn)?;

            // Backfill track passes generics
            backfill_track_passes(&conn)?;

            let mut stmt = conn
                .prepare(
                    "SELECT tp.id, tp.track_id, t.path
                     FROM track_passes tp
                     JOIN tracks t ON t.id = tp.track_id
                     WHERE tp.status = ?1 AND tp.pass_name = 'audio_analysis'
                     ORDER BY tp.id ASC",
                )
                .map_err(|e| e.to_string())?;

            let rows: Vec<SpoolJob> = stmt
                .query_map([pass_status::PENDING], |row| {
                    Ok(SpoolJob {
                        pass_id: row.get(0)?,
                        track_id: row.get(1)?,
                        path: row.get(2)?,
                    })
                })
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();

            for job in &rows {
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP
                     WHERE id = ?2",
                    rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
                );
            }
            rows
        };

        let total = pending.len();
        log::info!("[pipeline] audio_analysis pending: {}", total);

        // Check if there is any pending work across all passes
        let has_pending_passes = {
            let conn = conn_mutex.lock().map_err(|e| e.to_string())?;
            let pending_counts: Vec<(String, i64)> = conn
                .prepare(
                    "SELECT pass_name, COUNT(*) FROM track_passes WHERE status = ?1 GROUP BY pass_name",
                )
                .map_err(|e| e.to_string())?
                .query_map([pass_status::PENDING], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            log::info!("[pipeline] pending counts: {:?}", pending_counts);
            !pending_counts.is_empty()
        };

        if total == 0 && !has_pending_passes {
            log::info!("[pipeline] nothing to do, returning early");
            return Ok(());
        }
        log::info!(
            "[pipeline] proceeding — has_pending_passes={}",
            has_pending_passes
        );

        let concurrency = crate::hardware::PipelineConfig::auto_tune().decode_threads;

        let conn_arc = Arc::new(Mutex::new({
            let db_manager = DbManager::new(&app);
            db_manager
                .connect_and_migrate()
                .map_err(|e| e.to_string())?
        }));

        let handles = audio::run_audio_analysis_phase(&app, &conn_arc, pending, concurrency);

        // Wait for workers on a background thread so the IPC call returns immediately.
        std::thread::spawn(move || {
            let _guard = _guard;
            let _preventer_guard = sleep_preventer;

            // ── Phase 1: audio_analysis (parallel) ────────────────────────────
            log::info!("[pipeline] waiting for audio_analysis workers");
            for h in handles {
                let _ = h.join();
            }
            log::info!("[pipeline] audio_analysis done");
            let _ = app.emit(
                "analysis-phase-complete",
                serde_json::json!({ "pass": "audio_analysis" }),
            );

            // ── Phase 1b: BPM correction ──────────────────────────────────────
            log::info!("[pipeline] starting bpm_correction phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, bpm_correction::BpmCorrectionPass) {
                emit_pipeline_error(&app, "bpm_correction", e);
            }

            // ── Phase 2: CLAP ─────────────────────────────────────────────────
            log::info!("[pipeline] starting clap phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, clap::ClapPass) {
                emit_pipeline_error(&app, "clap", e);
            }

            // ── Phase 3: Qwen listener ────────────────────────────────────────
            log::info!("[pipeline] starting qwen phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, qwen::QwenPass) {
                emit_pipeline_error(&app, "qwen", e);
            }

            // ── Phase 4: Description embedding ────────────────────────────────
            log::info!("[pipeline] starting description_embed phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, description_embed::DescriptionEmbedPass) {
                emit_pipeline_error(&app, "description_embed", e);
            }

            // ── Phase 5: Essentia classifier ──────────────────────────────────
            log::info!("[pipeline] starting essentia phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, essentia::EssentiaPass) {
                emit_pipeline_error(&app, "essentia", e);
            }

            // ── Phase 6: BPM refinement (precise Discogs-400 genre) ───────────
            log::info!("[pipeline] starting bpm_refinement phase");
            if let Err(e) = run_pass_pipeline(&app, &conn_arc, bpm_refinement::BpmRefinementPass) {
                emit_pipeline_error(&app, "bpm_refinement", e);
            }

            let _ = app.emit("analysis-complete", ());
        });

        Ok(())
    }
}

// ── Mock Unit Testing ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    struct MockJob {
        pass_id: i64,
        track_id: i64,
        should_fail: bool,
    }

    impl PassJob for MockJob {
        fn pass_id(&self) -> i64 {
            self.pass_id
        }
        fn track_id(&self) -> i64 {
            self.track_id
        }
    }

    struct MockPass;

    impl<R: tauri::Runtime> AnalysisPass<R> for MockPass {
        type Job = MockJob;
        type Output = String;

        fn name(&self) -> &'static str {
            "mock_pass"
        }

        fn priority(&self) -> i32 {
            99
        }

        fn version(&self) -> u32 {
            1
        }

        fn dependencies(&self) -> &'static [&'static str] {
            &[]
        }

        fn owned_columns(&self) -> &'static [&'static str] {
            &[]
        }

        fn owned_tables(&self) -> &'static [&'static str] {
            &[]
        }

        fn load_jobs(&self, _conn: &Connection) -> Result<Vec<Self::Job>, String> {
            Ok(vec![
                MockJob {
                    pass_id: 101,
                    track_id: 1,
                    should_fail: false,
                },
                MockJob {
                    pass_id: 102,
                    track_id: 2,
                    should_fail: true,
                },
            ])
        }

        fn execute_job(&self, _app: &tauri::AppHandle<R>, job: &Self::Job) -> Result<Self::Output, String> {
            if job.should_fail {
                Err("Injected failure".to_string())
            } else {
                Ok("Success result".to_string())
            }
        }

        fn save_result(
            &self,
            _conn: &Connection,
            _job: &Self::Job,
            _output: Self::Output,
            _duration_ms: i64,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn test_generic_runner_lifecycle() {
        let conn = setup_test_db();

        // 1. Seed mock data in watched_directories and tracks tables
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', '/tracks')",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, 1, '/tracks/1.mp3', '1.mp3', 100, 0, 100)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (2, 1, '/tracks/2.mp3', '2.mp3', 100, 0, 100)",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO track_passes (id, track_id, pass_name, priority, status)
             VALUES (101, 1, 'mock_pass', 99, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO track_passes (id, track_id, pass_name, priority, status)
             VALUES (102, 2, 'mock_pass', 99, 0)",
            [],
        ).unwrap();

        let app = tauri::test::mock_app();
        let conn_arc = Arc::new(Mutex::new(conn));

        // 2. Run generic pipeline
        let result = run_pass_pipeline(app.handle(), &conn_arc, MockPass);
        assert!(result.is_ok());

        // 3. Assert DB state updates correctly
        let conn = conn_arc.lock().unwrap();
        
        let (status_101, duration_101): (i64, i64) = conn.query_row(
            "SELECT status, duration_ms FROM track_passes WHERE id = 101",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();

        let (status_102, log_102): (i64, Option<String>) = conn.query_row(
            "SELECT status, log FROM track_passes WHERE id = 102",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();

        // Pass 101 should be marked DONE (status = 2)
        assert_eq!(status_101, 2);
        assert!(duration_101 >= 0);

        // Pass 102 should be marked FAILED (status = 3) with injected error message
        assert_eq!(status_102, 3);
        assert_eq!(log_102, Some("Injected failure".to_string()));
    }
}
