use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use crate::dsp;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub struct AudioPass;

impl AudioPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "audio_analysis",
        priority: 10,
        version: pass_version::AUDIO_ANALYSIS,
        dependencies: &[],
        owned_columns: &[
            "waveform_data", "bpm", "bpm_raw", "key", "scale",
            "key_strength", "loudness_lufs", "loudness_range",
            "silence_regions", "has_long_silence"
        ],
        owned_tables: &[],
        owned_tag_sources: &["audio_analysis"],
        custom_reset: None,
    };
}

pub fn run_audio_analysis_phase(
    app: &tauri::AppHandle,
    conn_arc: &Arc<Mutex<Connection>>,
    pending: Vec<super::SpoolJob>,
    concurrency: usize,
    run_id: &str,
) -> Vec<std::thread::JoinHandle<()>> {
    let queue = Arc::new(Mutex::new(VecDeque::from(pending)));
    let mut handles = Vec::new();
    let run_id_clone = run_id.to_string();

    for _ in 0..concurrency {
        let queue_clone = Arc::clone(&queue);
        let conn_clone = Arc::clone(conn_arc);
        let app_clone = app.clone();
        let run_id_clone = run_id_clone.clone();

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
                let start_time_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                let result = dsp::run_audio_analysis(&job.path);
                let elapsed_ms = start.elapsed().as_millis() as i64;
                let ended_time_ms = start_time_ms + elapsed_ms;

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
                                loudness_range = ?8,
                                silence_regions = ?9,
                                has_long_silence = ?10
                             WHERE id = ?11",
                            rusqlite::params![
                                analysis.duration_seconds as i64,
                                analysis.waveform_data,
                                analysis.bpm,
                                analysis.key,
                                analysis.scale,
                                analysis.key_strength,
                                analysis.loudness_lufs,
                                analysis.loudness_range,
                                analysis.silence_regions,
                                if analysis.has_long_silence { 1 } else { 0 },
                                job.track_id,
                            ],
                        );

                        let _ = conn.execute(
                            "DELETE FROM track_tags WHERE track_id = ?1 AND source = 'audio_analysis'",
                            rusqlite::params![job.track_id],
                        );

                        // Key scale
                        match analysis.scale.as_deref() {
                            Some("minor") => { let _ = super::upsert_track_tag(&conn, job.track_id, "key", "minor", "audio_analysis"); }
                            Some("major") => { let _ = super::upsert_track_tag(&conn, job.track_id, "key", "major", "audio_analysis"); }
                            _ => {}
                        }

                        // Mastering dynamics
                        if analysis.loudness_lufs > -7.0 && analysis.loudness_range < 4.0 {
                            let _ = super::upsert_track_tag(&conn, job.track_id, "mastering", "brickwalled", "audio_analysis");
                        } else if analysis.loudness_range > 8.0 {
                            let _ = super::upsert_track_tag(&conn, job.track_id, "mastering", "dynamic", "audio_analysis");
                        }

                        // Length profile
                        let dur = analysis.duration_seconds as i64;
                        if dur < 120 {
                            let _ = super::upsert_track_tag(&conn, job.track_id, "len", "short", "audio_analysis");
                        } else if dur > 420 {
                            let _ = super::upsert_track_tag(&conn, job.track_id, "len", "extended", "audio_analysis");
                        }
                        let raw_result = serde_json::json!({
                            "bpm_raw": analysis.bpm,
                            "key": analysis.key,
                            "scale": analysis.scale,
                            "key_strength": analysis.key_strength,
                            "loudness_lufs": analysis.loudness_lufs,
                            "loudness_range": analysis.loudness_range,
                            "duration_s": analysis.duration_seconds,
                            "has_long_silence": analysis.has_long_silence,
                        }).to_string();
                        let _ = conn.execute(
                            "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                             pass_version = ?3, raw_result = ?4, last_run_at = CURRENT_TIMESTAMP WHERE id = ?5",
                            rusqlite::params![
                                pass_status::DONE,
                                elapsed_ms,
                                pass_version::AUDIO_ANALYSIS,
                                raw_result,
                                job.pass_id
                            ],
                        );
                        if crate::commands::config::is_sidecar_enabled(&conn) {
                            if let Err(e) = crate::scanner::sidecar::save(&conn, job.track_id) {
                                log::error!("[audio_analysis] sidecar save failed for track {}: {}", job.track_id, e);
                            }
                        }
                        let _ = app_clone.emit("analysis-progress", serde_json::json!({
                            "track_id": job.track_id,
                            "status": pass_status::DONE,
                        }));
                        crate::metrics_database::log_pipeline_metric(
                            &app_clone,
                            &run_id_clone,
                            job.track_id,
                            "audio_analysis",
                            "success",
                            elapsed_ms,
                            start_time_ms,
                            ended_time_ms,
                            Some(analysis.duration_seconds as f64),
                            None
                        );
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
                        crate::metrics_database::log_pipeline_metric(
                            &app_clone,
                            &run_id_clone,
                            job.track_id,
                            "audio_analysis",
                            "failed",
                            elapsed_ms,
                            start_time_ms,
                            ended_time_ms,
                            None,
                            Some(&e)
                        );
                    }
                }
            }
        }));
    }

    handles
}
