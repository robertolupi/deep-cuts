use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;
use tauri::Manager;

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
    type Output = QwenOutput;

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

        let status_code = resp.status();
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

        let raw_response = format!(
            "[Status: {}] {}",
            status_code,
            serde_json::to_string(&resp_json).unwrap_or_default()
        );

        // Preemptively save the raw completions response to the database
        // so that it is persisted and inspectable even if structured parsing downstream fails!
        if let Some(conn_mutex) = _app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
            if let Ok(conn) = conn_mutex.lock() {
                let _ = conn.execute(
                    "UPDATE track_passes SET raw_result = ?1 WHERE track_id = ?2 AND pass_name = 'qwen'",
                    rusqlite::params![raw_response, job.track_id],
                );
            }
        }

        // 6. Parse Response
        let parsed = parse_qwen_response(content);

        // Handle unrecognized formats as failures
        let all_empty = parsed.is_music.is_none()
            && parsed.ai_genre.is_none()
            && parsed.ai_mood.is_none()
            && parsed.ai_instruments.is_none()
            && parsed.description.is_none();

        if all_empty {
            return Err("Qwen response contained no parseable fields".to_string());
        }

        Ok(QwenOutput {
            parsed,
            raw_response,
        })
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        let is_music_val = output.parsed.is_music;
        let ai_genre_val = output.parsed.ai_genre.as_deref();
        let ai_mood_val = output.parsed.ai_mood.as_deref();
        let ai_instruments_val = output.parsed.ai_instruments.as_deref();
        let description_val = output.parsed.description.as_deref();

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

        // Persist raw completions response in the newly migrated track_passes table column
        conn.execute(
            "UPDATE track_passes SET raw_result = ?1
             WHERE track_id = ?2 AND pass_name = 'qwen'",
            rusqlite::params![output.raw_response, job.track_id],
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

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct QwenOutput {
    pub parsed: ParsedQwenResponse,
    pub raw_response: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct ParsedQwenResponse {
    pub is_music: Option<i64>,
    pub ai_genre: Option<String>,
    pub ai_mood: Option<String>,
    pub ai_instruments: Option<String>,
    pub description: Option<String>,
}

pub fn parse_qwen_response(content: &str) -> ParsedQwenResponse {
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
            let key_upper = trimmed[..pos].to_uppercase();
            let val = trimmed[pos + 1..].trim().to_string();
            if val.is_empty() {
                continue;
            }
            if key_upper.contains("MUSIC") {
                is_music = Some(if val.to_lowercase().contains("yes") {
                    1
                } else {
                    0
                });
            } else if key_upper.contains("GENRE") {
                ai_genre = Some(val);
            } else if key_upper.contains("MOOD") {
                ai_mood = Some(val);
            } else if key_upper.contains("INSTRUMENTS") {
                ai_instruments = Some(val);
            } else if key_upper.contains("DESCRIPTION") {
                description = Some(val);
            }
        }
    }

    // Fallback: If structured description parsing failed, use the entire raw content
    if description.is_none() || description.as_ref().map_or(true, |d| d.trim().is_empty()) {
        if !content.trim().is_empty() {
            log::warn!("[qwen] Failed to parse structured description. Falling back to raw response output.");
            description = Some(content.to_string());
            if is_music.is_none() {
                is_music = Some(1);
            }
        }
    }

    ParsedQwenResponse {
        is_music,
        ai_genre,
        ai_mood,
        ai_instruments,
        description,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_response() {
        let content = "MUSIC: yes\nGENRE: Electronic, Techno\nMOOD: aggressive, driving\nINSTRUMENTS: synthesizer, drum machine\nDESCRIPTION: A heavy pounding techno track.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("Electronic, Techno"));
        assert_eq!(res.ai_mood.as_deref(), Some("aggressive, driving"));
        assert_eq!(res.ai_instruments.as_deref(), Some("synthesizer, drum machine"));
        assert_eq!(res.description.as_deref(), Some("A heavy pounding techno track."));
    }

    #[test]
    fn test_parse_bolded_response() {
        let content = "**MUSIC**: yes\n**GENRE**: House\n**MOOD**: happy\n**INSTRUMENTS**: piano\n**DESCRIPTION**: A bright house song.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("House"));
        assert_eq!(res.description.as_deref(), Some("A bright house song."));
    }

    #[test]
    fn test_parse_numbered_list_response() {
        let content = "1. MUSIC: yes\n2. GENRE: Ambient\n3. DESCRIPTION: A very quiet ambient soundscape.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("Ambient"));
        assert_eq!(res.description.as_deref(), Some("A very quiet ambient soundscape."));
    }

    #[test]
    fn test_parse_fallback_response() {
        let content = "Just a plain paragraph describing the track completely without headers or colons.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1)); // fallback defaults to 1
        assert_eq!(res.description.as_deref(), Some(content));
    }
}
