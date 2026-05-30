use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use rusqlite::Connection;
use crate::database::{pass_status, DbManager};
use crate::scanner::sidecar::pass_version;
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

            // Reset DONE rows whose pass_version is below the current algorithm version.
            // This forces re-inference when a model or algorithm is updated.
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'audio_analysis' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![pass_status::PENDING, pass_status::DONE, pass_version::AUDIO_ANALYSIS],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'clap' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![pass_status::PENDING, pass_status::DONE, pass_version::CLAP],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE track_passes SET status = ?1, log = NULL
                 WHERE pass_name = 'essentia' AND status = ?2 AND pass_version < ?3",
                rusqlite::params![pass_status::PENDING, pass_status::DONE, pass_version::ESSENTIA],
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

            // Backfill essentia pass (priority 50 — runs after clap)
            conn.execute(
                "INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
                 SELECT id, 'essentia', 50, ?1 FROM tracks",
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

        // Check if there is any pending work across all passes
        let has_pending_passes = {
            let conn = conn_mutex.lock().map_err(|e| e.to_string())?;
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM track_passes WHERE status = ?1)",
                [pass_status::PENDING],
                |row| row.get(0),
            ).unwrap_or(false)
        };

        if total == 0 && !has_pending_passes {
            return Ok(());
        }

        let concurrency = crate::hardware::PipelineConfig::auto_tune().decode_threads;

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
            for h in handles {
                let _ = h.join();
            }
            let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": "audio_analysis" }));

            // ── Phase 2: CLAP — producer-consumer with seek-aware parallel preprocessing ──
            let config = crate::hardware::PipelineConfig::auto_tune();

            if let Err(e) = embeddings::configure_session(config.use_coreml, config.intra_threads, Some(&app)) {
                eprintln!("[clap] Failed to configure ONNX session: {}", e);
                let _ = app.emit("analysis-complete", ());
                return;
            }

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
                mel_windows: [Vec<f32>; 3],
            }

            let (tx, rx) = std::sync::mpsc::sync_channel::<PreppedSpectrogram>(config.decode_threads * 2);
            let clap_jobs_queue = Arc::new(Mutex::new(VecDeque::from(clap_pending)));

            let mut prep_workers = Vec::new();
            for _ in 0..config.decode_threads {
                let queue_clone = Arc::clone(&clap_jobs_queue);
                let tx_clone = tx.clone();
                let app_clone = app.clone();

                prep_workers.push(std::thread::spawn(move || {
                    loop {
                        let job = {
                            let mut q = queue_clone.lock().unwrap();
                            q.pop_front()
                        };
                        let job = match job {
                            Some(j) => j,
                            None => break,
                        };

                        let result = (|| -> Result<[Vec<f32>; 3], String> {
                            Ok([
                                embeddings::preprocess_window_at_pct(&job.path, 0.25, Some(&app_clone))?,
                                embeddings::preprocess_window_at_pct(&job.path, 0.50, Some(&app_clone))?,
                                embeddings::preprocess_window_at_pct(&job.path, 0.75, Some(&app_clone))?,
                            ])
                        })();

                        match result {
                            Ok(mel_windows) => {
                                let _ = tx_clone.send(PreppedSpectrogram {
                                    pass_id: job.pass_id,
                                    track_id: job.track_id,
                                    mel_windows,
                                });
                            }
                            Err(e) => {
                                log::error!("[clap] Preprocessing failed for track {}: {}", job.track_id, e);
                            }
                        }
                    }
                }));
            }
            drop(tx);

            for prepped in rx {
                let start = std::time::Instant::now();
                let result = embeddings::run_clap_inference_pooled(prepped.mel_windows);
                let elapsed_ms = start.elapsed().as_millis() as i64;

                let conn = conn_arc.lock().unwrap();
                match result {
                    Ok(embedding) => {
                        let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
                            rusqlite::params![prepped.track_id, blob],
                        );
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                             pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                            rusqlite::params![pass_status::DONE, elapsed_ms,
                                pass_version::CLAP, prepped.pass_id],
                        );
                        let _ = app.emit("analysis-progress", serde_json::json!({
                            "track_id": prepped.track_id,
                            "pass_name": "clap",
                            "status": pass_status::DONE,
                        }));
                    }
                    Err(e) => {
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                             last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                            rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                        );
                        let _ = app.emit("analysis-progress", serde_json::json!({
                            "track_id": prepped.track_id,
                            "pass_name": "clap",
                            "status": pass_status::FAILED,
                        }));
                    }
                }
            }

            for h in prep_workers {
                let _ = h.join();
            }

            let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": "clap" }));

            // ── Phase 3: Essentia classifier (sequential, single-threaded) ────────
            run_essentia_phase(&app, &conn_arc);

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
        let conn = conn_arc.lock().unwrap();
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'essentia'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[essentia] Failed to query pending jobs: {}", e);
                return;
            }
        };
        let rows: Vec<SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(SpoolJob { pass_id: row.get(0)?, track_id: row.get(1)?, path: row.get(2)? })
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
                let job = { queue_clone.lock().unwrap().pop_front() };
                let job = match job { Some(j) => j, None => break };

                let result = (|| -> Result<Vec<Vec<f32>>, String> {
                    let (audio, sr) = dsp::decode_audio_to_mono(&job.path)?;
                    let audio_16k = crate::spectrogram::resample_to_16k(&audio, sr)?;
                    let mid = audio_16k.len() / 2;
                    let half = 16_000 * 30;
                    let start = mid.saturating_sub(half);
                    let end = (mid + half).min(audio_16k.len());
                    let spec = crate::spectrogram::compute_log_mel_spectrogram(&audio_16k[start..end])?;
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
                        log::error!("[essentia] Preprocessing failed for track {}: {}", job.track_id, e);
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
        let conn = conn_arc.lock().unwrap();

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
                        r.genre, r.vocal, r.vocal_confidence,
                        r.mood_happy, r.mood_sad, r.mood_aggressive,
                        r.mood_relaxed, r.mood_party, r.mood_acoustic,
                        r.mood_electronic, prepped.track_id,
                    ],
                );
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::DONE, elapsed_ms,
                        pass_version::ESSENTIA, prepped.pass_id],
                );
                let _ = app.emit("analysis-progress", serde_json::json!({
                    "track_id": prepped.track_id,
                    "pass_name": "essentia",
                    "status": pass_status::DONE,
                }));
            }
            Err(e) => {
                log::error!("[essentia] Track {} failed: {}", prepped.track_id, e);
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                );
                let _ = app.emit("analysis-progress", serde_json::json!({
                    "track_id": prepped.track_id,
                    "pass_name": "essentia",
                    "status": pass_status::FAILED,
                }));
            }
        }
    }

    for h in prep_handles { let _ = h.join(); }
    let _ = app.emit("analysis-phase-complete", serde_json::json!({ "pass": "essentia" }));
}
