use crate::database::{pass_status, DbManager};
use crate::scanner::sidecar::pass_version;
use crate::{dsp, embeddings};
use rusqlite::Connection;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use tauri::{AppHandle, Emitter};

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

fn emit_pipeline_error(app: &tauri::AppHandle, phase: &str, message: impl Into<String>) {
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

fn lock_analysis_conn<'a>(
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

        let queue = Arc::new(Mutex::new(VecDeque::from(pending)));
        let conn_arc = Arc::new(Mutex::new({
            let db_manager = DbManager::new(&app);
            db_manager
                .connect_and_migrate()
                .map_err(|e| e.to_string())?
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
                            match queue_clone.lock() {
                                Ok(mut q) => q.pop_front(),
                                Err(e) => {
                                    log::error!("[audio_analysis] queue lock poisoned: {}", e);
                                    break;
                                }
                            }
                        };
                        let job = match job {
                            Some(j) => j,
                            None => break,
                        };

                        let start = std::time::Instant::now();
                        let result = dsp::run_audio_analysis(&job.path);
                        let elapsed_ms = start.elapsed().as_millis() as i64;

                        let conn = match conn_clone.lock() {
                            Ok(conn) => conn,
                            Err(e) => {
                                log::error!("[audio_analysis] database lock poisoned: {}", e);
                                let _ = app_clone.emit("analysis-error", serde_json::json!({
                                    "phase": "audio_analysis",
                                    "message": format!("Database lock poisoned: {}", e),
                                }));
                                break;
                            }
                        };
                        match result {
                            Ok(analysis) => {
                                let _ = conn.execute(
                                    "UPDATE tracks SET
                                        duration_seconds = ?1,
                                        waveform_data = ?2,
                                        bpm = ?3,
                                        bpm_raw = ?3,
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
                                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                    rusqlite::params![pass_status::DONE, elapsed_ms,
                                        pass_version::AUDIO_ANALYSIS, job.pass_id],
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
            run_bpm_correction_phase(&app, &conn_arc);
            log::info!("[pipeline] bpm_correction phase done");

            // ── Phase 2: CLAP — producer-consumer with seek-aware parallel preprocessing ──
            let config = crate::hardware::PipelineConfig::auto_tune();

            if let Err(e) =
                embeddings::configure_session(config.use_coreml, config.intra_threads, Some(&app))
            {
                emit_pipeline_error(
                    &app,
                    "clap",
                    format!("Failed to configure ONNX session: {}", e),
                );
                let _ = app.emit("analysis-complete", ());
                return;
            }

            let clap_pending: Vec<SpoolJob> = {
                let conn = match lock_analysis_conn(&conn_arc, "clap") {
                    Ok(conn) => conn,
                    Err(e) => {
                        emit_pipeline_error(&app, "clap", e);
                        let _ = app.emit("analysis-complete", ());
                        return;
                    }
                };
                let mut stmt = match conn.prepare(
                    "SELECT tp.id, tp.track_id, t.path
                     FROM track_passes tp
                     JOIN tracks t ON t.id = tp.track_id
                     WHERE tp.status = ?1 AND tp.pass_name = 'clap'
                     ORDER BY tp.id ASC",
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        emit_pipeline_error(
                            &app,
                            "clap",
                            format!("Failed to prepare clap query: {}", e),
                        );
                        let _ = app.emit("analysis-complete", ());
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

            struct PreppedSpectrogram {
                pass_id: i64,
                track_id: i64,
                result: Result<[Vec<f32>; 3], String>,
                elapsed_ms: i64,
            }

            let (tx, rx) =
                std::sync::mpsc::sync_channel::<PreppedSpectrogram>(config.decode_threads * 2);
            let clap_jobs_queue = Arc::new(Mutex::new(VecDeque::from(clap_pending)));

            let mut prep_workers = Vec::new();
            for _ in 0..config.decode_threads {
                let queue_clone = Arc::clone(&clap_jobs_queue);
                let tx_clone = tx.clone();
                let app_clone = app.clone();

                prep_workers.push(std::thread::spawn(move || loop {
                    let job = {
                        match queue_clone.lock() {
                            Ok(mut q) => q.pop_front(),
                            Err(e) => {
                                log::error!("[clap] queue lock poisoned: {}", e);
                                break;
                            }
                        }
                    };
                    let job = match job {
                        Some(j) => j,
                        None => break,
                    };

                    let start = std::time::Instant::now();
                    let result = (|| -> Result<[Vec<f32>; 3], String> {
                        Ok([
                            embeddings::preprocess_window_at_pct(
                                &job.path,
                                0.25,
                                Some(&app_clone),
                            )?,
                            embeddings::preprocess_window_at_pct(
                                &job.path,
                                0.50,
                                Some(&app_clone),
                            )?,
                            embeddings::preprocess_window_at_pct(
                                &job.path,
                                0.75,
                                Some(&app_clone),
                            )?,
                        ])
                    })();
                    let elapsed_ms = start.elapsed().as_millis() as i64;

                    match result {
                        Ok(mel_windows) => {
                            let _ = tx_clone.send(PreppedSpectrogram {
                                pass_id: job.pass_id,
                                track_id: job.track_id,
                                result: Ok(mel_windows),
                                elapsed_ms,
                            });
                        }
                        Err(e) => {
                            log::error!(
                                "[clap] Preprocessing failed for track {}: {}",
                                job.track_id,
                                e
                            );
                            let _ = tx_clone.send(PreppedSpectrogram {
                                pass_id: job.pass_id,
                                track_id: job.track_id,
                                result: Err(e),
                                elapsed_ms,
                            });
                        }
                    }
                }));
            }
            drop(tx);

            for prepped in rx {
                let (result, elapsed_ms) = match prepped.result {
                    Ok(mel_windows) => {
                        let start = std::time::Instant::now();
                        let result = embeddings::run_clap_inference_pooled(mel_windows);
                        (result, start.elapsed().as_millis() as i64)
                    }
                    Err(e) => (
                        Err(format!("Preprocessing failed: {}", e)),
                        prepped.elapsed_ms,
                    ),
                };

                let conn = match lock_analysis_conn(&conn_arc, "clap") {
                    Ok(conn) => conn,
                    Err(e) => {
                        emit_pipeline_error(&app, "clap", e);
                        let _ = app.emit("analysis-complete", ());
                        return;
                    }
                };
                match result {
                    Ok(embedding) => {
                        let blob: Vec<u8> =
                            embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                            rusqlite::params![prepped.track_id, blob],
                        );
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                             pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                            rusqlite::params![
                                pass_status::DONE,
                                elapsed_ms,
                                pass_version::CLAP,
                                prepped.pass_id
                            ],
                        );
                        let _ = app.emit(
                            "analysis-progress",
                            serde_json::json!({
                                "track_id": prepped.track_id,
                                "pass_name": "clap",
                                "status": pass_status::DONE,
                            }),
                        );
                    }
                    Err(e) => {
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                             last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                            rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                        );
                        let _ = app.emit(
                            "analysis-progress",
                            serde_json::json!({
                                "track_id": prepped.track_id,
                                "pass_name": "clap",
                                "status": pass_status::FAILED,
                            }),
                        );
                    }
                }
            }

            for h in prep_workers {
                let _ = h.join();
            }

            let _ = app.emit(
                "analysis-phase-complete",
                serde_json::json!({ "pass": "clap" }),
            );

            // ── Phase 3: Qwen listener (sequential, single-threaded) ──────────────
            run_qwen_phase(&app, &conn_arc);

            // ── Phase 4: Description embedding (sequential, single-threaded) ──────
            run_description_embed_phase(&app, &conn_arc);

            // ── Phase 5: Essentia classifier (sequential, single-threaded) ────────
            run_essentia_phase(&app, &conn_arc);

            // ── Phase 6: BPM refinement (precise Discogs-400 genre) ─────────
            run_bpm_refinement_phase(&app, &conn_arc);

            let _ = app.emit("analysis-complete", ());
        });

        Ok(())
    }
}

/// Preprocessed spectrogram patches ready for ONNX inference.
struct PreppedPatches {
    pass_id: i64,
    track_id: i64,
    patches: Vec<Vec<f32>>,
}

/// Runs pending `essentia` jobs using a producer-consumer pipeline:
///   - `decode_threads` workers each decode → resample → spectrogram → patches
///   - 1 inference consumer runs all ONNX sessions and writes results to DB
fn run_essentia_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let config = crate::hardware::PipelineConfig::auto_tune();

    let jobs: Vec<SpoolJob> = {
        let conn = match lock_analysis_conn(conn_arc, "essentia") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "essentia", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'essentia'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                emit_pipeline_error(
                    app,
                    "essentia",
                    format!("Failed to query pending jobs: {}", e),
                );
                return;
            }
        };
        let rows: Vec<SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    if jobs.is_empty() {
        return;
    }

    log::info!(
        "[essentia] {} jobs, {} decode workers",
        jobs.len(),
        config.decode_threads
    );

    // Channel: preprocessing workers → inference consumer.
    // Bounded to 2× workers so fast decoders don't get too far ahead of inference.
    let (tx, rx) = std::sync::mpsc::sync_channel::<PreppedPatches>(config.decode_threads * 2);
    let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));

    // ── Preprocessing workers (decode + resample + spectrogram) ──────────────
    let mut prep_handles = Vec::new();
    for _ in 0..config.decode_threads {
        let queue_clone = Arc::clone(&queue);
        let tx_clone = tx.clone();

        prep_handles.push(std::thread::spawn(move || {
            loop {
                let job = {
                    match queue_clone.lock() {
                        Ok(mut q) => q.pop_front(),
                        Err(e) => {
                            log::error!("[essentia] queue lock poisoned: {}", e);
                            break;
                        }
                    }
                };
                let job = match job {
                    Some(j) => j,
                    None => break,
                };

                let result = (|| -> Result<Vec<Vec<f32>>, String> {
                    let (audio, sr) = dsp::decode_audio_to_mono(&job.path)?;
                    let audio_16k = crate::spectrogram::resample_to_16k(&audio, sr)?;
                    let mid = audio_16k.len() / 2;
                    let half = 16_000 * 30;
                    let start = mid.saturating_sub(half);
                    let end = (mid + half).min(audio_16k.len());
                    let spec =
                        crate::spectrogram::compute_log_mel_spectrogram(&audio_16k[start..end])?;
                    crate::spectrogram::extract_patches(&spec)
                })();

                match result {
                    Ok(patches) => {
                        let _ = tx_clone.send(PreppedPatches {
                            pass_id: job.pass_id,
                            track_id: job.track_id,
                            patches,
                        });
                    }
                    Err(e) => {
                        log::error!(
                            "[essentia] Preprocessing failed for track {}: {}",
                            job.track_id,
                            e
                        );
                        // Send an empty-patches sentinel so the consumer can record the failure
                        let _ = tx_clone.send(PreppedPatches {
                            pass_id: job.pass_id,
                            track_id: job.track_id,
                            patches: vec![],
                        });
                    }
                }
            }
        }));
    }
    drop(tx); // Close sender so the consumer loop terminates when all workers finish

    // ── Inference consumer (single thread — ONNX sessions serialised) ────────
    for prepped in rx {
        let start = std::time::Instant::now();

        let result = if prepped.patches.is_empty() {
            Err("Preprocessing failed".to_string())
        } else {
            crate::classifier::run_classifier_inference(&prepped.patches, Some(app))
        };

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match lock_analysis_conn(conn_arc, "essentia") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "essentia", e);
                return;
            }
        };

        match result {
            Ok(r) => {
                let _ = conn.execute(
                    "UPDATE tracks SET
                        detected_genre             = ?1,
                        detected_vocal             = ?2,
                        detected_vocal_confidence  = ?3,
                        mood_happy                 = ?4,
                        mood_sad                   = ?5,
                        mood_aggressive            = ?6,
                        mood_relaxed               = ?7,
                        mood_party                 = ?8,
                        mood_acoustic              = ?9,
                        mood_electronic            = ?10
                     WHERE id = ?11",
                    rusqlite::params![
                        r.genre,
                        r.vocal,
                        r.vocal_confidence,
                        r.mood_happy,
                        r.mood_sad,
                        r.mood_aggressive,
                        r.mood_relaxed,
                        r.mood_party,
                        r.mood_acoustic,
                        r.mood_electronic,
                        prepped.track_id,
                    ],
                );
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![
                        pass_status::DONE,
                        elapsed_ms,
                        pass_version::ESSENTIA,
                        prepped.pass_id
                    ],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": prepped.track_id,
                        "pass_name": "essentia",
                        "status": pass_status::DONE,
                    }),
                );
            }
            Err(e) => {
                log::error!("[essentia] Track {} failed: {}", prepped.track_id, e);
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": prepped.track_id,
                        "pass_name": "essentia",
                        "status": pass_status::FAILED,
                    }),
                );
            }
        }
    }

    for h in prep_handles {
        let _ = h.join();
    }
    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({ "pass": "essentia" }),
    );
}

/// Runs pending `bpm_correction` jobs using the coarse metadata `genre` field.
fn run_bpm_correction_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    // Each job needs the track's metadata genre alongside the standard SpoolJob fields.
    struct BpmJob {
        pass_id: i64,
        track_id: i64,
        bpm_raw: Option<f64>,
        genre: Option<String>,
    }

    let jobs: Vec<BpmJob> = {
        let conn = match lock_analysis_conn(conn_arc, "bpm_correction") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "bpm_correction", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.bpm_raw, t.genre
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'bpm_correction'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                emit_pipeline_error(
                    app,
                    "bpm_correction",
                    format!("Failed to prepare pending jobs query: {}", e),
                );
                return;
            }
        };
        let rows: Vec<BpmJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(BpmJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    bpm_raw: row.get(2)?,
                    genre: row.get(3)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    let start_phase = std::time::Instant::now();
    let mut corrected = 0usize;
    let mut nulled = 0usize;

    log::info!(
        "[bpm_correction] loaded {} jobs, computing corrections",
        jobs.len()
    );

    // Compute all corrections first (pure CPU, no lock needed)
    let corrections: Vec<crate::bpm::CorrectResult> = jobs
        .iter()
        .map(|job| crate::bpm::correct_bpm(job.bpm_raw, job.genre.as_deref()))
        .collect();

    log::info!("[bpm_correction] corrections computed, acquiring DB lock for transaction");
    // Write everything in a single transaction — avoids 1886 individual fsyncs
    {
        let conn = match lock_analysis_conn(conn_arc, "bpm_correction") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "bpm_correction", e);
                return;
            }
        };
        log::debug!("[bpm_correction] lock acquired, beginning transaction");
        let begin_result = conn.execute("BEGIN", []);
        log::debug!("[bpm_correction] BEGIN result: {:?}", begin_result);
        for (job, result) in jobs.iter().zip(corrections.iter()) {
            match result {
                crate::bpm::CorrectResult::Corrected(new_bpm) => {
                    corrected += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = ?1 WHERE id = ?2",
                        rusqlite::params![new_bpm, job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Null => {
                    nulled += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = NULL WHERE id = ?1",
                        rusqlite::params![job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Unchanged => {}
            }
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, duration_ms = 0,
                 pass_version = ?2, last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                rusqlite::params![pass_status::DONE, pass_version::BPM_CORRECTION, job.pass_id],
            );
        }
        let commit_result = conn.execute("COMMIT", []);
        log::info!("[bpm_correction] COMMIT result: {:?}", commit_result);
    } // lock released before any emit

    log::info!(
        "[bpm_correction] {} tracks: {} corrected, {} nulled in {:.1}s",
        jobs.len(),
        corrected,
        nulled,
        start_phase.elapsed().as_secs_f32()
    );
    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({
            "pass": "bpm_correction", "corrected": corrected, "nulled": nulled,
        }),
    );
}

/// Runs pending `bpm_refinement` jobs using the precise Discogs-400 `detected_genre` field.
fn run_bpm_refinement_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    struct BpmJob {
        pass_id: i64,
        track_id: i64,
        bpm_raw: Option<f64>,
        detected_genre: Option<String>,
    }

    let jobs: Vec<BpmJob> = {
        let conn = match lock_analysis_conn(conn_arc, "bpm_refinement") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "bpm_refinement", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.bpm_raw, t.detected_genre
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'bpm_refinement'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                emit_pipeline_error(
                    app,
                    "bpm_refinement",
                    format!("Failed to prepare pending jobs query: {}", e),
                );
                return;
            }
        };
        let rows: Vec<BpmJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(BpmJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    bpm_raw: row.get(2)?,
                    detected_genre: row.get(3)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    let start_phase = std::time::Instant::now();
    let mut corrected = 0usize;
    let mut nulled = 0usize;

    log::info!(
        "[bpm_refinement] loaded {} jobs, computing corrections",
        jobs.len()
    );

    // Compute all corrections first (pure CPU, no lock needed)
    // bpm_refinement always re-corrects from bpm_raw so the two passes are independent
    let corrections: Vec<crate::bpm::CorrectResult> = jobs
        .iter()
        .map(|job| crate::bpm::correct_bpm(job.bpm_raw, job.detected_genre.as_deref()))
        .collect();

    log::info!("[bpm_refinement] corrections computed, acquiring DB lock for transaction");
    {
        let conn = match lock_analysis_conn(conn_arc, "bpm_refinement") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "bpm_refinement", e);
                return;
            }
        };
        log::debug!("[bpm_refinement] lock acquired, beginning transaction");
        let begin_result = conn.execute("BEGIN", []);
        log::debug!("[bpm_refinement] BEGIN result: {:?}", begin_result);
        for (job, result) in jobs.iter().zip(corrections.iter()) {
            match result {
                crate::bpm::CorrectResult::Corrected(new_bpm) => {
                    corrected += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = ?1 WHERE id = ?2",
                        rusqlite::params![new_bpm, job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Null => {
                    nulled += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = NULL WHERE id = ?1",
                        rusqlite::params![job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Unchanged => {}
            }
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, duration_ms = 0,
                 pass_version = ?2, last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                rusqlite::params![pass_status::DONE, pass_version::BPM_REFINEMENT, job.pass_id],
            );
        }
        let commit_result = conn.execute("COMMIT", []);
        log::info!("[bpm_refinement] COMMIT result: {:?}", commit_result);
    } // lock released before any emit

    log::info!(
        "[bpm_refinement] {} tracks: {} corrected, {} nulled in {:.1}s",
        jobs.len(),
        corrected,
        nulled,
        start_phase.elapsed().as_secs_f32()
    );
    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({
            "pass": "bpm_refinement", "corrected": corrected, "nulled": nulled,
        }),
    );
}

/// Sequential, thread-safe listener pass running the Qwen2-Audio model.
fn run_qwen_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<SpoolJob> = {
        let conn = match lock_analysis_conn(conn_arc, "qwen") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "qwen", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'qwen'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                emit_pipeline_error(app, "qwen", format!("Failed to query pending jobs: {}", e));
                return;
            }
        };
        let rows: Vec<SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    if jobs.is_empty() {
        return;
    }

    log::info!(
        "[qwen] loaded {} jobs, checking/booting llama-server",
        jobs.len()
    );

    // Boot llama-server
    let guard = match crate::llama::ensure_llama_server_running(app) {
        Ok(g) => g,
        Err(err) => {
            log::error!("[qwen] Failed to boot llama-server: {}", err);
            // Mark all jobs as failed
            let conn = match lock_analysis_conn(conn_arc, "qwen") {
                Ok(conn) => conn,
                Err(e) => {
                    emit_pipeline_error(app, "qwen", e);
                    return;
                }
            };
            for job in jobs {
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = 0, last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                    rusqlite::params![pass_status::FAILED, err, job.pass_id],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "qwen",
                        "status": pass_status::FAILED,
                    }),
                );
            }
            return;
        }
    };

    // Single-threaded sequential processing (Qwen inference is very heavy)
    for job in jobs {
        let start = std::time::Instant::now();

        let result = (|| -> Result<serde_json::Value, String> {
            // 1. Retrieve BPM/Key/Scale/Genre from DB to build prompt
            let track_data: (Option<f64>, Option<String>, Option<String>, Option<String>) = {
                let conn = lock_analysis_conn(conn_arc, "qwen")?;
                conn.query_row(
                    "SELECT bpm, key, scale, genre FROM tracks WHERE id = ?1",
                    [job.track_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .map_err(|e| e.to_string())?
            };

            let bpm = track_data.0.unwrap_or(120.0);
            let key = track_data.1.unwrap_or_else(|| "C".to_string());
            let scale = track_data.2.unwrap_or_else(|| "major".to_string());
            let genre = track_data.3;

            // 2. Decode audio & resample to 16 kHz
            let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(&job.path)?;
            let audio_16k_full = crate::spectrogram::resample_to_16k(&audio, sample_rate)?;

            // 3. Take 30 seconds centered midpoint window (15s on each side)
            let mid_16k = audio_16k_full.len() / 2;
            let half_16k = 16000 * 15;
            let start_idx = mid_16k.saturating_sub(half_16k);
            let end_idx = (mid_16k + half_16k).min(audio_16k_full.len());
            let audio_window = &audio_16k_full[start_idx..end_idx];

            // 4. Encode audio to WAV & Base64
            let wav_bytes = crate::dsp::encode_audio_to_wav(audio_window, 16000);
            let base64_audio = crate::dsp::base64_encode(&wav_bytes);

            // 5. Formulate prompt
            let mut prompt = format!(
                "The measured tempo of this track is approximately {:.0} BPM and the detected key is {} {}.",
                bpm, key, scale
            );
            if let Some(g) = genre {
                if !g.trim().is_empty() {
                    prompt.push_str(&format!(
                        " The file metadata tags this track as \"{}\", though that label may be broad or imprecise.",
                        g
                    ));
                }
            }
            prompt.push_str(
                "\nListen carefully and respond using ONLY the following format, one field per line, nothing else:\n\n\
                MUSIC: yes or no (is this music, as opposed to speech, podcast, sound effects, or silence?)\n\
                GENRE: genre and subgenre in a few words\n\
                MOOD: mood and emotional feel in a few words\n\
                INSTRUMENTS: main instruments, comma-separated\n\
                DESCRIPTION: two to three sentences of plain prose describing the track"
            );

            // 6. HTTP API Call
            let payload = serde_json::json!({
                "messages": [
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "input_audio",
                                "input_audio": {
                                    "data": base64_audio,
                                    "format": "wav"
                                }
                            },
                            {
                                "type": "text",
                                "text": prompt
                            }
                        ]
                    }
                ]
            });

            let api_url = format!(
                "http://127.0.0.1:{}/v1/chat/completions",
                crate::llama::LLAMA_PORT
            );
            log::info!("[qwen] Dispatching audio to local llama-server completions endpoint for track {}...", job.track_id);

            let resp = ureq::post(&api_url)
                .timeout(std::time::Duration::from_secs(120))
                .send_json(&payload)
                .map_err(|e| format!("Completions request to llama-server failed: {}", e))?;

            let resp_json = resp
                .into_json::<serde_json::Value>()
                .map_err(|e| format!("Failed to parse completions response JSON: {}", e))?;

            let content = resp_json["choices"][0]["message"]["content"]
                .as_str()
                .ok_or_else(|| {
                    format!(
                        "Unexpected JSON response structure from llama-server: {:?}",
                        resp_json
                    )
                })?;

            // 7. Parse Response
            let mut is_music = None;
            let mut ai_genre = None;
            let mut ai_mood = None;
            let mut ai_instruments = None;
            let mut description = None;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(pos) = trimmed.find(':') {
                    let key = trimmed[..pos].trim().to_uppercase();
                    let val = trimmed[pos + 1..].trim().to_string();
                    if val.is_empty() {
                        continue;
                    }
                    match key.as_str() {
                        "MUSIC" => {
                            is_music = Some(if val.to_lowercase().contains("yes") {
                                1
                            } else {
                                0
                            });
                        }
                        "GENRE" => ai_genre = Some(val),
                        "MOOD" => ai_mood = Some(val),
                        "INSTRUMENTS" => ai_instruments = Some(val),
                        "DESCRIPTION" => description = Some(val),
                        _ => {}
                    }
                }
            }

            Ok(serde_json::json!({
                "is_music": is_music,
                "ai_genre": ai_genre,
                "ai_mood": ai_mood,
                "ai_instruments": ai_instruments,
                "description": description
            }))
        })();

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match lock_analysis_conn(conn_arc, "qwen") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "qwen", e);
                return;
            }
        };

        match result {
            Ok(data) => {
                let is_music_val = data["is_music"].as_i64();
                let ai_genre_val = data["ai_genre"].as_str();
                let ai_mood_val = data["ai_mood"].as_str();
                let ai_instruments_val = data["ai_instruments"].as_str();
                let description_val = data["description"].as_str();

                // If Qwen returned a valid response but all fields parsed as None, the model
                // produced output in an unrecognised format (free prose, Chinese, refusal, etc.).
                // Treat this as a failure so the pass is retried rather than silently skipped.
                let all_empty = is_music_val.is_none()
                    && ai_genre_val.is_none()
                    && ai_mood_val.is_none()
                    && ai_instruments_val.is_none()
                    && description_val.is_none();

                if all_empty {
                    log::warn!(
                        "[qwen] Track {} produced no parseable fields — marking FAILED for retry",
                        job.track_id
                    );
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                         last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                        rusqlite::params![
                            pass_status::FAILED,
                            "Qwen response contained no parseable GENRE/MOOD/INSTRUMENTS/DESCRIPTION fields",
                            elapsed_ms,
                            job.pass_id
                        ],
                    );
                    let _ = app.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "track_id": job.track_id,
                            "pass_name": "qwen",
                            "status": pass_status::FAILED,
                        }),
                    );
                    continue;
                }

                let _ = conn.execute(
                    "UPDATE tracks SET
                        is_music = ?1,
                        ai_genre = ?2,
                        ai_mood = ?3,
                        ai_instruments = ?4,
                        description = ?5
                     WHERE id = ?6",
                    rusqlite::params![
                        is_music_val,
                        ai_genre_val,
                        ai_mood_val,
                        ai_instruments_val,
                        description_val,
                        job.track_id
                    ],
                );

                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![
                        pass_status::DONE,
                        elapsed_ms,
                        pass_version::QWEN,
                        job.pass_id
                    ],
                );

                // Re-queue the description_embed pass for this track so it runs after the
                // description is now available. Without this, description_embed marks itself
                // DONE on first run (when description was null) and never re-processes.
                if description_val.is_some() {
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP
                         WHERE track_id = ?2 AND pass_name = 'description_embed'",
                        rusqlite::params![pass_status::PENDING, job.track_id],
                    );
                }

                // Save to sidecar
                if let Err(e) = crate::scanner::sidecar::save(&conn, job.track_id) {
                    log::error!(
                        "[qwen] Failed to save sidecar metadata for track {}: {}",
                        job.track_id,
                        e
                    );
                }

                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "qwen",
                        "status": pass_status::DONE,
                    }),
                );
            }
            Err(e) => {
                log::error!("[qwen] Track {} failed: {}", job.track_id, e);
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "qwen",
                        "status": pass_status::FAILED,
                    }),
                );
            }
        }
    }

    drop(guard); // Automatic sequential termination of llama-server
    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({ "pass": "qwen" }),
    );
}

/// Description embedding pass utilizing all-MiniLM-L6-v2.
fn run_description_embed_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<SpoolJob> = {
        let conn = match lock_analysis_conn(conn_arc, "description_embed") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "description_embed", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'description_embed'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                emit_pipeline_error(
                    app,
                    "description_embed",
                    format!("Failed to query pending jobs: {}", e),
                );
                return;
            }
        };
        let rows: Vec<SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    if jobs.is_empty() {
        return;
    }

    log::info!(
        "[description_embed] loaded {} jobs, starting sentence embeddings",
        jobs.len()
    );

    for job in jobs {
        let start = std::time::Instant::now();

        let result = (|| -> Result<Option<Vec<f32>>, String> {
            // Retrieve description and other Qwen columns
            let track_data: (
                Option<i64>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ) = {
                let conn = lock_analysis_conn(conn_arc, "description_embed")?;
                conn.query_row(
                    "SELECT is_music, description, ai_genre, ai_mood, ai_instruments FROM tracks WHERE id = ?1",
                    [job.track_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
                ).map_err(|e| e.to_string())?
            };

            let is_music = track_data.0;
            let description = track_data.1;
            let ai_genre = track_data.2;
            let ai_mood = track_data.3;
            let ai_instruments = track_data.4;

            // If not music, skip entirely
            if let Some(0) = is_music {
                log::info!(
                    "[description_embed] Track {} marked as non-music. Skipping embedding.",
                    job.track_id
                );
                return Ok(None);
            }

            let desc = match description {
                Some(d) if !d.trim().is_empty() => d,
                _ => return Ok(None), // no description, mark done with no embedding
            };

            // Build concatenated text for richer semantic signal
            let mut embed_text = String::new();
            if let Some(g) = ai_genre {
                if !g.trim().is_empty() {
                    embed_text.push_str(&format!("Genre: {}. ", g));
                }
            }
            if let Some(m) = ai_mood {
                if !m.trim().is_empty() {
                    embed_text.push_str(&format!("Mood: {}. ", m));
                }
            }
            if let Some(i) = ai_instruments {
                if !i.trim().is_empty() {
                    embed_text.push_str(&format!("Instruments: {}. ", i));
                }
            }
            embed_text.push_str(&desc);

            let embedding = crate::embeddings::run_sentence_embed(&embed_text, Some(app))?;
            Ok(Some(embedding))
        })();

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match lock_analysis_conn(conn_arc, "description_embed") {
            Ok(conn) => conn,
            Err(e) => {
                emit_pipeline_error(app, "description_embed", e);
                return;
            }
        };

        match result {
            Ok(emb_opt) => {
                if let Some(embedding) = emb_opt {
                    let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO description_embeddings (track_id, embedding) VALUES (?1, ?2)",
                        rusqlite::params![job.track_id, blob],
                    );
                }

                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![
                        pass_status::DONE,
                        elapsed_ms,
                        pass_version::DESCRIPTION_EMBED,
                        job.pass_id
                    ],
                );

                // Save sidecar
                if let Err(e) = crate::scanner::sidecar::save(&conn, job.track_id) {
                    log::error!(
                        "[description_embed] Failed to save sidecar metadata for track {}: {}",
                        job.track_id,
                        e
                    );
                }

                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "description_embed",
                        "status": pass_status::DONE,
                    }),
                );
            }
            Err(e) => {
                log::error!("[description_embed] Track {} failed: {}", job.track_id, e);
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.pass_id],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "description_embed",
                        "status": pass_status::FAILED,
                    }),
                );
            }
        }
    }

    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({ "pass": "description_embed" }),
    );
}
