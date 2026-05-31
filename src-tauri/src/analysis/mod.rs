use crate::database::{pass_status, DbManager};
use crate::scanner::sidecar::pass_version;
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

            // Reset DONE rows whose pass_version is below the current algorithm version.
            // This forces re-inference when a model or algorithm is updated.
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'audio_analysis' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::DONE,
                    pass_version::AUDIO_ANALYSIS
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'clap' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![pass_status::PENDING, pass_status::DONE, pass_version::CLAP],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'qwen' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![pass_status::PENDING, pass_status::DONE, pass_version::QWEN],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'description_embed' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::DONE,
                    pass_version::DESCRIPTION_EMBED
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'essentia' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::DONE,
                    pass_version::ESSENTIA
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'bpm_correction' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::DONE,
                    pass_version::BPM_CORRECTION
                ],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'bpm_refinement' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![
                    pass_status::PENDING,
                    pass_status::DONE,
                    pass_version::BPM_REFINEMENT
                ],
            )
            .map_err(|e| e.to_string())?;

            // Backfill: insert a row for every track that doesn't have one yet
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'audio_analysis', 10, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill bpm_correction pass (priority 15 — runs after audio_analysis)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'bpm_correction', 15, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill clap pass (priority 20 — runs after bpm_correction)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'clap', 20, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill qwen pass (priority 30 — runs after clap)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'qwen', 30, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill description_embed pass (priority 40 — runs after qwen)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'description_embed', 40, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill essentia pass (priority 50 — runs after description_embed)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'essentia', 50, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

            // Backfill bpm_refinement pass (priority 55 — runs after essentia)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'bpm_refinement', 55, ?1 FROM tracks",
                [pass_status::PENDING],
            )
            .map_err(|e| e.to_string())?;

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

            // ── Phase 1b: BPM correction (coarse metadata genre) ──────────────
            log::info!("[pipeline] starting bpm_correction phase");
            bpm_correction::run_bpm_correction_phase(&app, &conn_arc);
            log::info!("[pipeline] bpm_correction phase done");

            // ── Phase 2: CLAP ─────────────────────────────────────────────────
            log::info!("[pipeline] starting clap phase");
            clap::run_clap_phase(&app, &conn_arc);
            log::info!("[pipeline] clap phase done");

            // ── Phase 3: Qwen listener ────────────────────────────────────────
            log::info!("[pipeline] starting qwen phase");
            qwen::run_qwen_phase(&app, &conn_arc);
            log::info!("[pipeline] qwen phase done");

            // ── Phase 4: Description embedding ────────────────────────────────
            log::info!("[pipeline] starting description_embed phase");
            description_embed::run_description_embed_phase(&app, &conn_arc);
            log::info!("[pipeline] description_embed phase done");

            // ── Phase 5: Essentia classifier ──────────────────────────────────
            log::info!("[pipeline] starting essentia phase");
            essentia::run_essentia_phase(&app, &conn_arc);
            log::info!("[pipeline] essentia phase done");

            // ── Phase 6: BPM refinement (precise Discogs-400 genre) ───────────
            log::info!("[pipeline] starting bpm_refinement phase");
            bpm_refinement::run_bpm_refinement_phase(&app, &conn_arc);
            log::info!("[pipeline] bpm_refinement phase done");

            let _ = app.emit("analysis-complete", ());
        });

        Ok(())
    }
}
