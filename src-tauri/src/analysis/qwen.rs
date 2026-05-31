use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct QwenJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub path: String,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub genre: Option<String>,
}

impl super::PassJob for QwenJob {
    fn pass_id(&self) -> i64 {
        self.pass_id
    }
    fn track_id(&self) -> i64 {
        self.track_id
    }
}

pub struct QwenPass;

impl super::AnalysisPass for QwenPass {
    type Job = QwenJob;
    type Output = serde_json::Value;

    fn name(&self) -> &'static str {
        "qwen"
    }

    fn priority(&self) -> i32 {
        30
    }

    fn version(&self) -> u32 {
        pass_version::QWEN
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["audio_analysis", "bpm_correction"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &["is_music", "ai_genre", "ai_mood", "ai_instruments", "description"]
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &["description_embeddings"]
    }

    fn setup(&self, app: &tauri::AppHandle) -> Result<(), String> {
        log::info!("[qwen] Booting llama-server for pipeline run...");
        let guard = crate::llama::ensure_llama_server_running(app)?;
        // Bypass guard drop to keep server active across all sequential jobs
        std::mem::forget(guard);
        Ok(())
    }

    fn teardown(&self, app: &tauri::AppHandle) -> Result<(), String> {
        log::info!("[qwen] Terminating llama-server post-run...");
        crate::llama::terminate_llama_server(app);
        Ok(())
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.path, t.bpm, t.key, t.scale, t.genre
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'qwen'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(QwenJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                path: row.get(2)?,
                bpm: row.get(3)?,
                key: row.get(4)?,
                scale: row.get(5)?,
                genre: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        let bpm = job.bpm.unwrap_or(120.0);
        let key = job.key.as_deref().unwrap_or("C");
        let scale = job.scale.as_deref().unwrap_or("major");

        // 1. Decode audio & resample to 16 kHz
        let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(&job.path)?;
        let audio_16k_full = crate::spectrogram::resample_to_16k(&audio, sample_rate)?;

        // 2. Take 30 seconds centered midpoint window (15s on each side)
        let mid_16k = audio_16k_full.len() / 2;
        let half_16k = 16000 * 15;
        let start_idx = mid_16k.saturating_sub(half_16k);
        let end_idx = (mid_16k + half_16k).min(audio_16k_full.len());
        let audio_window = &audio_16k_full[start_idx..end_idx];

        // 3. Encode audio to WAV & Base64
        let wav_bytes = crate::dsp::encode_audio_to_wav(audio_window, 16000);
        let base64_audio = crate::dsp::base64_encode(&wav_bytes);

        // 4. Formulate prompt
        let mut prompt = format!(
            "The measured tempo of this track is approximately {:.0} BPM and the detected key is {} {}.",
            bpm, key, scale
        );
        if let Some(g) = &job.genre {
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

        // 5. HTTP API Call
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

        // 6. Parse Response
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
                let key = trimmed[..pos].trim().replace('*', "").to_uppercase();
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

        // Handle unrecognized formats as failures
        let all_empty = is_music.is_none()
            && ai_genre.is_none()
            && ai_mood.is_none()
            && ai_instruments.is_none()
            && description.is_none();

        if all_empty {
            return Err("Qwen response contained no parseable fields".to_string());
        }

        Ok(serde_json::json!({
            "is_music": is_music,
            "ai_genre": ai_genre,
            "ai_mood": ai_mood,
            "ai_instruments": ai_instruments,
            "description": description
        }))
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        let is_music_val = output["is_music"].as_i64();
        let ai_genre_val = output["ai_genre"].as_str();
        let ai_mood_val = output["ai_mood"].as_str();
        let ai_instruments_val = output["ai_instruments"].as_str();
        let description_val = output["description"].as_str();

        conn.execute(
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
        ).map_err(|e| e.to_string())?;

        // Re-queue the description_embed pass for this track so it runs after description is available
        if description_val.is_some() {
            conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP
                 WHERE track_id = ?2 AND pass_name = 'description_embed'",
                rusqlite::params![pass_status::PENDING, job.track_id],
            ).map_err(|e| e.to_string())?;
        }

        // Save to sidecar
        if let Err(e) = crate::scanner::sidecar::save(conn, job.track_id) {
            log::error!(
                "[qwen] Failed to save sidecar metadata for track {}: {}",
                job.track_id,
                e
            );
        }

        Ok(())
    }
}

impl QwenPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "qwen",
        priority: 30,
        version: pass_version::QWEN,
        dependencies: &["audio_analysis", "bpm_correction"],
        owned_columns: &["is_music", "ai_genre", "ai_mood", "ai_instruments", "description"],
        owned_tables: &["description_embeddings"],
        custom_reset: None,
    };
}
