use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use crate::dsp;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

struct PreppedPatches {
    pass_id: i64,
    track_id: i64,
    patches: Vec<Vec<f32>>,
}

pub fn run_essentia_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let config = crate::hardware::PipelineConfig::auto_tune();

    let jobs: Vec<super::SpoolJob> = {
        let conn = match super::lock_analysis_conn(conn_arc, "essentia") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "essentia", e);
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
                super::emit_pipeline_error(
                    app,
                    "essentia",
                    format!("Failed to query pending jobs: {}", e),
                );
                return;
            }
        };
        let rows: Vec<super::SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(super::SpoolJob {
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

    let (tx, rx) = std::sync::mpsc::sync_channel::<PreppedPatches>(config.decode_threads * 2);
    let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));

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
    drop(tx);

    for prepped in rx {
        let start = std::time::Instant::now();

        let result = if prepped.patches.is_empty() {
            Err("Preprocessing failed".to_string())
        } else {
            crate::classifier::run_classifier_inference(&prepped.patches, Some(app))
        };

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match super::lock_analysis_conn(conn_arc, "essentia") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "essentia", e);
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
