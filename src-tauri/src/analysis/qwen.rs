use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub fn run_qwen_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<super::SpoolJob> = {
        let conn = match super::lock_analysis_conn(conn_arc, "qwen") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "qwen", e);
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
                super::emit_pipeline_error(app, "qwen", format!("Failed to query pending jobs: {}", e));
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
        "[qwen] loaded {} jobs, checking/booting llama-server",
        jobs.len()
    );

    // Boot llama-server
    let guard = match crate::llama::ensure_llama_server_running(app) {
        Ok(g) => g,
        Err(err) => {
            log::error!("[qwen] Failed to boot llama-server: {}", err);
            // Mark all jobs as failed
            let conn = match super::lock_analysis_conn(conn_arc, "qwen") {
                Ok(conn) => conn,
                Err(e) => {
                    super::emit_pipeline_error(app, "qwen", e);
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
                let conn = super::lock_analysis_conn(conn_arc, "qwen")?;
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
        let conn = match super::lock_analysis_conn(conn_arc, "qwen") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "qwen", e);
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
