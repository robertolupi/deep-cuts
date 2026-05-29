use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use rusqlite::Connection;
use crate::database::{pass_status, DbManager};
use crate::{dsp, embeddings};

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

struct SpoolJob {
    pass_id: i64,
    track_id: i64,
    path: String,
}

pub struct PipelineManager;

impl PipelineManager {
    /// Returns true if the analysis pipeline is currently active.
    pub fn is_running() -> bool {
        ANALYSIS_ACTIVE.load(Ordering::SeqCst)
    }

    /// Runs the audio analysis and embedding pipeline concurrently.
    pub fn run(app: AppHandle, conn_mutex: &Mutex<Connection>) -> Result<(), String> {
        if ANALYSIS_ACTIVE.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
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
                rusqlite::params![pass_status::PENDING, pass_status::IN_PROGRESS, pass_status::FAILED],
            ).map_err(|e| e.to_string())?;

            // Backfill: insert a row for every track that doesn't have one yet
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'audio_analysis', 10, ?1 FROM tracks",
                [pass_status::PENDING],
            ).map_err(|e| e.to_string())?;

            // Backfill clap pass (priority 20 — runs after audio_analysis)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'clap', 20, ?1 FROM tracks",
                [pass_status::PENDING],
            ).map_err(|e| e.to_string())?;

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

        // Check if there are any pending CLAP passes
        let has_clap = {
            let conn = conn_mutex.lock().map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM track_passes WHERE status = ?1 AND pass_name = 'clap')",
                [pass_status::PENDING],
                |row| row.get(0),
            ).unwrap_or(false)
        };

        if total == 0 && !has_clap {
            return Ok(());
        }

        let concurrency = (std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            / 2)
            .max(1);

        let queue = Arc::new(Mutex::new(VecDeque::from(pending)));
        let conn_arc = Arc::new(Mutex::new({
            let db_manager = DbManager::new(&app);
            db_manager.connect_and_migrate().map_err(|e| e.to_string())?
        }));

        let mut handles = Vec::new();
        if total > 0 {
            for _ in 0..concurrency {
                let queue_clone = Arc::clone(&queue);
                let conn_clone = Arc::clone(&conn_arc);
                let app_clone = app.clone();

                handles.push(std::thread::spawn(move || {
                    loop {
                        let job = {
                            let mut q = queue_clone.lock().unwrap();
                            q.pop_front()
                        };
                        let job = match job {
                            Some(j) => j,
                            None => break,
                        };

                        let start = std::time::Instant::now();
                        let result = dsp::run_audio_analysis(&job.path);
                        let elapsed_ms = start.elapsed().as_millis() as i64;

                        let conn = conn_clone.lock().unwrap();
                        match result {
                            Ok(analysis) => {
                                let _ = conn.execute(
                                    "UPDATE tracks SET
                                        duration_seconds = ?1,
                                        waveform_data = ?2,
                                        bpm = ?3,
                                        key = ?4,
                                        scale = ?5,
                                        key_strength = ?6,
                                        loudness_lufs = ?7,
                                        loudness_range = ?8
                                     WHERE id = ?9",
                                    rusqlite::params![
                                        analysis.duration_seconds as i64,
                                        analysis.waveform_data,
                                        analysis.bpm,
                                        analysis.key,
                                        analysis.scale,
                                        analysis.key_strength,
                                        analysis.loudness_lufs,
                                        analysis.loudness_range,
                                        job.track_id,
                                    ],
                                );
                                let _ = conn.execute(
                                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                                    rusqlite::params![pass_status::DONE, elapsed_ms, job.pass_id],
                                );
                                let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                    "track_id": job.track_id,
                                    "status": pass_status::DONE,
                                }));
                            }
                            Err(e) => {
                                let _ = conn.execute(
                                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                                );
                                let _ = app_clone.emit("analysis-progress", serde_json::json!({
                                    "track_id": job.track_id,
                                    "status": pass_status::FAILED,
                                }));
                            }
                        }
                    }
                }));
            }
        }

        // Wait for audio_analysis workers on a background thread so the IPC call returns immediately.
        // Then immediately run the clap pass sequentially (1 thread — model is memory-heavy).
        // Moving _guard into the thread keeps ANALYSIS_ACTIVE=true until all passes finish.
        std::thread::spawn(move || {
            let _guard = _guard;
            let _preventer_guard = sleep_preventer;

            // ── Phase 1: audio_analysis (parallel) ────────────────────────────
            for h in handles {
                let _ = h.join();
            }
            let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": "audio_analysis" }));

            // ── Phase 2: clap (single thread, model loaded once via OnceLock) ─
            let clap_pending: Vec<SpoolJob> = {
                let conn = conn_arc.lock().unwrap();
                let mut stmt = match conn.prepare(
                    "SELECT tp.id, tp.track_id, t.path
                     FROM track_passes tp
                     JOIN tracks t ON t.id = tp.track_id
                     WHERE tp.status = ?1 AND tp.pass_name = 'clap'
                     ORDER BY tp.id ASC",
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[clap] Failed to prepare clap query: {}", e);
                        return;
                    }
                };
                let rows: Vec<SpoolJob> = match stmt.query_map([pass_status::PENDING], |row| {
                    Ok(SpoolJob {
                        pass_id: row.get(0)?,
                        track_id: row.get(1)?,
                        path: row.get(2)?,
                    })
                }) {
                    Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
                    Err(_) => Vec::new(),
                };
                for job in &rows {
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                        rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
                    );
                }
                rows
            };

            for job in clap_pending {
                let start = std::time::Instant::now();
                let result = embeddings::run_clap_audio_embed(&job.path, Some(&app));
                let elapsed_ms = start.elapsed().as_millis() as i64;

                let conn = conn_arc.lock().unwrap();
                match result {
                    Ok(embedding) => {
                        // Serialise 512 floats as little-endian bytes for sqlite-vec
                        let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                            rusqlite::params![job.track_id, blob],
                        );
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                             last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                            rusqlite::params![pass_status::DONE, elapsed_ms, job.pass_id],
                        );
                        let _ = app.emit("analysis-progress", serde_json::json!({
                            "track_id": job.track_id,
                            "pass_name": "clap",
                            "status": pass_status::DONE,
                        }));
                    }
                    Err(e) => {
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                             last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                            rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                        );
                        let _ = app.emit("analysis-progress", serde_json::json!({
                            "track_id": job.track_id,
                            "pass_name": "clap",
                            "status": pass_status::FAILED,
                        }));
                    }
                }
            }

            let _ = app.emit("analysis-complete", ());
        });

        Ok(())
    }
}
