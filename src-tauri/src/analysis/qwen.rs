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
    pub waveform_data: Option<String>,
    #[allow(dead_code)]
    pub duration_seconds: i64,
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
        &["audio_analysis", "bpm_correction", "clap"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &[
            "is_music",
            "ai_genre",
            "ai_mood",
            "ai_instruments",
            "description",
        ]
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
        let mut stmt = conn
            .prepare(
                "SELECT tp.id, tp.track_id, t.path, t.bpm, t.key, t.scale, t.genre,
                        t.waveform_data, t.duration_seconds
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'qwen'
             ORDER BY tp.id ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(QwenJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
                    bpm: row.get(3)?,
                    key: row.get(4)?,
                    scale: row.get(5)?,
                    genre: row.get(6)?,
                    waveform_data: row.get(7)?,
                    duration_seconds: row.get(8)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(rows)
    }

    fn execute_job(
        &self,
        app: &tauri::AppHandle,
        job: &Self::Job,
    ) -> Result<Self::Output, String> {
        let bpm = job.bpm.unwrap_or(120.0);
        let key = job.key.as_deref().unwrap_or("C");
        let scale = job.scale.as_deref().unwrap_or("major");

        // 1. Decode audio & resample to 16 kHz
        let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(&job.path)?;
        let audio_16k_full = crate::spectrogram::resample_to_16k(&audio, sample_rate)?;

        // 2. Take 30 seconds centered on the highest-energy bin from the waveform profile.
        let window_pct = crate::embeddings::select_best_energy_window_pct(
            job.waveform_data.as_deref(),
        );
        let center_16k = (audio_16k_full.len() as f64 * window_pct) as usize;
        let half_16k = 16000 * 15;
        let start_idx = center_16k.saturating_sub(half_16k);
        let end_idx = (center_16k + half_16k).min(audio_16k_full.len());
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
            "\nListen carefully and respond strictly in English and using ONLY the following format, one field per line, nothing else:\n\n\
            MUSIC: yes or no (is this music, as opposed to speech, podcast, sound effects, or silence?)\n\
            GENRE: genre and subgenre in a few words\n\
            MOOD: mood and emotional feel in a few words\n\
            INSTRUMENTS: main instruments, comma-separated\n\
            DESCRIPTION: two to three sentences of plain prose describing the track"
        );

        // Retrieve CLAP embedding for this track from the database if available
        let audio_embedding: Option<Vec<f32>> = {
            if let Some(conn_mutex) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
                if let Ok(conn) = conn_mutex.lock() {
                    let blob: Option<Vec<u8>> = conn.query_row(
                        "SELECT embedding FROM audio_embeddings WHERE track_id = ?1",
                        rusqlite::params![job.track_id],
                        |row| row.get(0)
                    ).ok();

                    blob.and_then(|b| {
                        if b.len() == 512 * 4 {
                            let floats: Vec<f32> = b
                                .chunks_exact(4)
                                .map(|ch| f32::from_le_bytes(ch.try_into().unwrap()))
                                .collect();
                            Some(floats)
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            } else {
                None
            }
        };

        let port = crate::llama::get_llama_port(app)
            .ok_or_else(|| "[qwen] llama-server port not available; was ensure_llama_server_running called?".to_string())?;
        let api_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

        let mut outer_attempts = 0;
        let max_outer_attempts = 2;
        let mut best_output: Option<QwenOutput> = None;
        let mut best_similarity = -1.0f32;

        let query_completions = |messages_payload: &serde_json::Value| -> Result<String, String> {
            let payload = serde_json::json!({
                "messages": messages_payload,
                "temperature": 0.2
            });
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

            Ok(content.to_string())
        };

        while outer_attempts < max_outer_attempts {
            outer_attempts += 1;

            let mut messages = vec![
                serde_json::json!({
                    "role": "user",
                    "content": [
                        {
                            "type": "input_audio",
                            "input_audio": {
                                "data": base64_audio.clone(),
                                "format": "wav"
                            }
                        },
                        {
                            "type": "text",
                            "text": format!(
                                "Listen carefully to this audio. The measured tempo is approximately {:.0} BPM and the detected key is {} {}. {}\n\
                                First, is this track music (as opposed to speech, podcast, sound effects, or silence)? Respond strictly in English in this format:\n\
                                MUSIC: yes or no",
                                bpm, key, scale,
                                job.genre.as_ref().map_or("".to_string(), |g| format!("The file metadata tags it as \"{}\".", g))
                            )
                        }
                    ]
                })
            ];

            log::info!(
                "[qwen] Dispatching audio to local llama-server completions endpoint for track {} (outer attempt {}/{})...",
                job.track_id, outer_attempts, max_outer_attempts
            );

            // Step 1: MUSIC
            let mut is_music = None;
            let mut music_content = String::new();
            let mut step_success = false;

            for attempt in 1..=3 {
                match query_completions(&serde_json::json!(messages)) {
                    Ok(content) => {
                        let has_chinese = content.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
                        let parsed = parse_qwen_response(&content);
                        if parsed.is_music.is_some() && !has_chinese {
                            is_music = parsed.is_music;
                            music_content = content;
                            step_success = true;
                            break;
                        }

                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": content
                        }));
                        messages.push(serde_json::json!({
                            "role": "user",
                            "content": "CRITICAL: Please respond strictly in English and use the format: MUSIC: yes or no"
                        }));
                    }
                    Err(e) => {
                        log::warn!("[qwen] Step 1 (music) attempt {} failed: {}", attempt, e);
                    }
                }
            }

            if !step_success || is_music.is_none() {
                log::warn!("[qwen] Failed to resolve MUSIC status for track {}", job.track_id);
                continue;
            }

            // Early exit if it is not music
            if is_music == Some(0) {
                let parsed = ParsedQwenResponse {
                    is_music: Some(0),
                    ai_genre: None,
                    ai_mood: None,
                    ai_instruments: None,
                    description: None,
                };
                let raw_response = serde_json::to_string(&messages).unwrap_or_default();
                
                // Preemptively save raw response
                if let Some(conn_mutex) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
                    if let Ok(conn) = conn_mutex.lock() {
                        let _ = conn.execute(
                            "UPDATE track_passes SET raw_result = ?1 WHERE track_id = ?2 AND pass_name = 'qwen'",
                            rusqlite::params![raw_response, job.track_id],
                        );
                    }
                }
                return Ok(QwenOutput { parsed, raw_response });
            }

            messages.push(serde_json::json!({
                "role": "assistant",
                "content": music_content
            }));

            // Steps 2 to 5: GENRE, MOOD, INSTRUMENTS, DESCRIPTION
            let mut ai_genre = None;
            let mut ai_mood = None;
            let mut ai_instruments = None;
            let mut description = None;

            let steps = vec![
                ("genre", "What is the genre and subgenre of this track in a few words? Respond strictly in English in this format:\nGENRE: genre and subgenre"),
                ("mood", "What is the mood and emotional feel of this track in a few words? Respond strictly in English in this format:\nMOOD: mood and emotional feel"),
                ("instruments", "What are the main instruments playing in this track, comma-separated? Respond strictly in English in this format:\nINSTRUMENTS: main instruments"),
                ("description", "Provide two to three sentences of plain prose describing the track. Respond strictly in English in this format:\nDESCRIPTION: description"),
            ];

            let mut all_steps_ok = true;
            for (step_name, step_prompt) in steps {
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": step_prompt
                }));

                let mut current_step_success = false;
                for attempt in 1..=3 {
                    match query_completions(&serde_json::json!(messages)) {
                        Ok(content) => {
                            let has_chinese = content.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
                            let parsed = parse_qwen_response(&content);

                            let valid = match step_name {
                                "genre" => parsed.ai_genre.is_some() && !has_chinese,
                                "mood" => parsed.ai_mood.is_some() && !has_chinese,
                                "instruments" => parsed.ai_instruments.is_some() && !has_chinese,
                                "description" => parsed.description.is_some() && !has_chinese,
                                _ => false,
                            };

                            if valid {
                                match step_name {
                                    "genre" => ai_genre = parsed.ai_genre,
                                    "mood" => ai_mood = parsed.ai_mood,
                                    "instruments" => ai_instruments = parsed.ai_instruments,
                                    "description" => description = parsed.description,
                                    _ => {}
                                }
                                messages.push(serde_json::json!({
                                    "role": "assistant",
                                    "content": content
                                }));
                                current_step_success = true;
                                break;
                            }

                            messages.push(serde_json::json!({
                                "role": "assistant",
                                "content": content
                            }));
                            let correction = format!(
                                "CRITICAL: Please respond strictly in English and use the format: {}: ...",
                                step_name.to_uppercase()
                            );
                            messages.push(serde_json::json!({
                                "role": "user",
                                "content": correction
                            }));
                        }
                        Err(e) => {
                            log::warn!("[qwen] Step {} attempt {} failed: {}", step_name, attempt, e);
                        }
                    }
                }

                if !current_step_success {
                    all_steps_ok = false;
                    break;
                }
            }

            if !all_steps_ok {
                continue;
            }

            let parsed = ParsedQwenResponse {
                is_music,
                ai_genre,
                ai_mood,
                ai_instruments,
                description: description.clone(),
            };
            let raw_response = serde_json::to_string(&messages).unwrap_or_default();

            // Save raw response to database
            if let Some(conn_mutex) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
                if let Ok(conn) = conn_mutex.lock() {
                    let _ = conn.execute(
                        "UPDATE track_passes SET raw_result = ?1 WHERE track_id = ?2 AND pass_name = 'qwen'",
                        rusqlite::params![raw_response, job.track_id],
                    );
                }
            }

            let output_candidate = QwenOutput {
                parsed: parsed.clone(),
                raw_response: raw_response.clone(),
            };

            // If we don't have a CLAP embedding in the database, bypass verification
            let Some(audio_emb) = &audio_embedding else {
                log::info!(
                    "[qwen] No CLAP embedding found in database for track {}. Bypassing verification.",
                    job.track_id
                );
                return Ok(output_candidate);
            };

            let mut similarity = 0.0f32;
            let mut got_similarity = false;

            if let Some(desc) = &parsed.description {
                match crate::embeddings::run_clap_text_embed(desc, Some(app)) {
                    Ok(text_emb) => {
                        similarity = audio_emb.iter().zip(text_emb.iter()).map(|(a, t)| a * t).sum();
                        got_similarity = true;
                    }
                    Err(e) => {
                        log::warn!(
                            "[qwen] Failed to generate CLAP text embedding for track {}: {}",
                            job.track_id, e
                        );
                    }
                }
            }

            if got_similarity {
                log::info!(
                    "[qwen] Track {} CLAP similarity = {:.4}",
                    job.track_id, similarity
                );

                if similarity >= 0.28 {
                    log::info!(
                        "[qwen] Track {} passed CLAP verification with similarity {:.4} >= 0.28",
                        job.track_id, similarity
                    );
                    return Ok(output_candidate);
                }

                if similarity > best_similarity {
                    best_similarity = similarity;
                    best_output = Some(output_candidate);
                }

                log::warn!(
                    "[qwen] Track {} failed CLAP verification (similarity {:.4} < 0.28) on outer attempt {}/{}",
                    job.track_id, similarity, outer_attempts, max_outer_attempts
                );
            } else {
                if best_output.is_none() {
                    best_output = Some(output_candidate);
                }
            }
        }

        if let Some(best) = best_output {
            log::warn!(
                "[qwen] Track {} failed verification across all attempts. Saving best candidate with similarity {:.4}",
                job.track_id, best_similarity
            );
            Ok(best)
        } else {
            Err("All Qwen verification attempts failed parsing".to_string())
        }
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
        )
        .map_err(|e| e.to_string())?;

        // Re-queue the description_embed pass for this track so it runs after description is available
        if description_val.is_some() {
            conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP
                 WHERE track_id = ?2 AND pass_name = 'description_embed'",
                rusqlite::params![pass_status::PENDING, job.track_id],
            )
            .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn raw_result_json(&self, output: &Self::Output) -> Option<String> {
        let fields_found: Vec<&str> = [
            output.parsed.is_music.map(|_| "is_music"),
            output.parsed.ai_genre.as_ref().map(|_| "genre"),
            output.parsed.ai_mood.as_ref().map(|_| "mood"),
            output.parsed.ai_instruments.as_ref().map(|_| "instruments"),
            output.parsed.description.as_ref().map(|_| "description"),
        ]
        .iter()
        .filter_map(|x| *x)
        .collect();
        let all_fields = ["is_music", "genre", "mood", "instruments", "description"];
        let missing: Vec<&str> = all_fields
            .iter()
            .filter(|f| !fields_found.contains(*f))
            .copied()
            .collect();
        Some(serde_json::json!({
            "parse": { "fields_found": fields_found, "missing": missing },
            "http": output.raw_response,
        }).to_string())
    }
}

impl QwenPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "qwen",
        priority: 50,
        version: pass_version::QWEN,
        dependencies: &["audio_analysis", "bpm_correction", "clap"],
        owned_columns: &[
            "is_music",
            "ai_genre",
            "ai_mood",
            "ai_instruments",
            "description",
        ],
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

/// Keywords that introduce a new field. Longer variants must come before shorter ones
/// so that e.g. "in mood" is matched before "mood".
const FIELD_KEYWORDS: &[&str] = &[
    "instruments",
    "instrument",
    "description",
    "is music",
    "in mood",
    "genres",
    "genre",
    "music",
    "moods",
    "mood",
    "desc",
];

/// Within a single segment that may contain multiple comma/period-separated fields
/// (e.g. "MUSIC: yes, genres: pop, in mood: happy. Description: …"), split it into
/// one sub-segment per field boundary. Returns the original segment unchanged when
/// no additional boundaries are found.
fn split_segment_on_keywords(segment: &str) -> Vec<&str> {
    let lower = segment.to_lowercase();
    let mut cut_points = Vec::new();

    for &kw in FIELD_KEYWORDS {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(kw) {
            let kw_start = from + rel;
            // Validate prefix (word boundary: start of string, or preceded by whitespace/punctuation/delimiters)
            let prefix_ok = kw_start == 0 || {
                let prev_char = segment[..kw_start].chars().next_back().unwrap();
                prev_char.is_whitespace()
                    || prev_char == ','
                    || prev_char == '.'
                    || prev_char == ';'
                    || prev_char == '*'
                    || prev_char == '_'
                    || prev_char == '('
                    || prev_char == '['
                    || prev_char == '-'
            };

            if prefix_ok {
                // Validate suffix: check if after kw there's only optional spaces/markdown and then a colon
                let rest = &lower[kw_start + kw.len()..];
                let mut valid_suffix = false;
                for c in rest.chars() {
                    if c.is_whitespace() || c == '*' || c == '_' {
                        continue;
                    }
                    if c == ':' {
                        valid_suffix = true;
                    }
                    break;
                }
                if valid_suffix {
                    cut_points.push(kw_start);
                }
            }
            from = kw_start + 1;
        }
    }

    if cut_points.is_empty() {
        return vec![segment];
    }
    cut_points.push(0); // Ensure 0 is included
    cut_points.sort_unstable();
    cut_points.dedup();

    // Map byte offsets to slices of the original segment
    let mut slices = Vec::new();
    for w in cut_points.windows(2) {
        let s = segment[w[0]..w[1]]
            .trim_start_matches(|c: char| c == ',' || c == '.' || c == ' ')
            .trim()
            .trim_end_matches(|c: char| c == ',' || c == '.' || c == ';')
            .trim();
        if !s.is_empty() {
            slices.push(s);
        }
    }
    if let Some(&last_start) = cut_points.last() {
        let s = segment[last_start..]
            .trim_start_matches(|c: char| c == ',' || c == '.' || c == ' ')
            .trim();
        if !s.is_empty() {
            slices.push(s);
        }
    }

    slices
}

pub fn parse_qwen_response(content: &str) -> ParsedQwenResponse {
    let mut is_music = None;
    let mut ai_genre = None;
    let mut ai_mood = None;
    let mut ai_instruments = None;
    let mut description = None;

    // Normalize literal \n escape sequences the model sometimes emits instead of real newlines
    let normalized = content.replace("\\n", "\n");

    // Primary split: newlines then semicolons.
    // Secondary split: within each segment, split further at ", keyword:" or ". keyword:"
    // boundaries so that single-line comma-separated responses are also handled.
    let segments: Vec<&str> = normalized
        .lines()
        .flat_map(|l| l.split(';'))
        .flat_map(|seg| split_segment_on_keywords(seg))
        .collect();

    for segment in segments {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(pos) = trimmed.find(':') {
            let key_upper = trimmed[..pos].to_uppercase();
            let val = trimmed[pos + 1..]
                .trim()
                .trim_end_matches(|c: char| c == ',' || c == ';')
                .trim()
                .to_string();
            if val.is_empty() {
                continue;
            }
            if key_upper.contains("MUSIC") {
                is_music = is_music.or(Some(if val.to_lowercase().contains("yes") { 1 } else { 0 }));
            } else if key_upper.contains("GENRE") {
                ai_genre = ai_genre.or(Some(val));
            } else if key_upper.contains("MOOD") {
                ai_mood = ai_mood.or(Some(val));
            } else if key_upper.contains("INSTRUMENT") {
                ai_instruments = ai_instruments.or(Some(val));
            } else if key_upper.contains("DESCRIPTION") || key_upper.trim() == "DESC" {
                description = description.or(Some(val));
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
    use crate::analysis::AnalysisPass;

    #[test]
    fn test_parse_standard_response() {
        let content = "MUSIC: yes\nGENRE: Electronic, Techno\nMOOD: aggressive, driving\nINSTRUMENTS: synthesizer, drum machine\nDESCRIPTION: A heavy pounding techno track.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("Electronic, Techno"));
        assert_eq!(res.ai_mood.as_deref(), Some("aggressive, driving"));
        assert_eq!(
            res.ai_instruments.as_deref(),
            Some("synthesizer, drum machine")
        );
        assert_eq!(
            res.description.as_deref(),
            Some("A heavy pounding techno track.")
        );
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
        let content =
            "1. MUSIC: yes\n2. GENRE: Ambient\n3. DESCRIPTION: A very quiet ambient soundscape.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("Ambient"));
        assert_eq!(
            res.description.as_deref(),
            Some("A very quiet ambient soundscape.")
        );
    }

    #[test]
    fn test_parse_fallback_response() {
        let content =
            "Just a plain paragraph describing the track completely without headers or colons.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1)); // fallback defaults to 1
        assert_eq!(res.description.as_deref(), Some(content));
    }

    #[test]
    fn test_parse_literal_escape_newlines() {
        // Model emits \\n as text instead of real newlines
        let content = "MUSIC: yes\\nGENRE: dance, electronic\\nMOOD: happy, summer\\nINSTRUMENTS: bass, drum\\nDESCRIPTION: A groovy summer track.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("dance, electronic"));
        assert_eq!(res.ai_mood.as_deref(), Some("happy, summer"));
        assert_eq!(res.ai_instruments.as_deref(), Some("bass, drum"));
        assert_eq!(res.description.as_deref(), Some("A groovy summer track."));
    }

    #[test]
    fn test_parse_semicolon_delimited() {
        // Model uses semicolons as field separators instead of newlines
        let content = "MUSIC: yes; GENRE: electronic, house, techno; MOOD: energetic, groovy; INSTRUMENTS: synthesizer, bass, drums; DESCRIPTION: A lively dance track.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("electronic, house, techno"));
        assert_eq!(res.ai_mood.as_deref(), Some("energetic, groovy"));
        assert_eq!(
            res.ai_instruments.as_deref(),
            Some("synthesizer, bass, drums")
        );
        assert_eq!(res.description.as_deref(), Some("A lively dance track."));
    }

    /// Runs parse_qwen_response against every real Qwen response captured from
    /// the production database. The fixture file is generated by querying
    /// track_passes.raw_result and is committed alongside the test.
    ///
    /// Assertions per case:
    ///   - is_music is Some (always parseable; the fallback guarantees description)
    ///   - description is Some and non-empty
    ///   - ai_genre / ai_mood / ai_instruments are Some when the raw response
    ///     contains the expected English header (flagged as expect_* in the fixture)
    #[test]
    fn test_parse_all_real_qwen_responses() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            filename: String,
            content: String,
            expect_genre: bool,
            expect_mood: bool,
            expect_instruments: bool,
        }

        let fixtures: Vec<Fixture> = serde_json::from_str(
            include_str!("qwen_test_fixtures.json")
        ).expect("failed to parse qwen_test_fixtures.json");

        let mut failures = Vec::new();

        for f in &fixtures {
            let res = parse_qwen_response(&f.content);

            if res.is_music.is_none() {
                failures.push(format!("{}: is_music is None", f.filename));
            }
            match &res.description {
                None => failures.push(format!("{}: description is None", f.filename)),
                Some(d) if d.trim().is_empty() => {
                    failures.push(format!("{}: description is empty", f.filename));
                }
                _ => {}
            }
            if f.expect_genre && res.ai_genre.is_none() {
                failures.push(format!("{}: ai_genre expected but not parsed", f.filename));
            }
            if f.expect_mood && res.ai_mood.is_none() {
                failures.push(format!("{}: ai_mood expected but not parsed", f.filename));
            }
            if f.expect_instruments && res.ai_instruments.is_none() {
                failures.push(format!("{}: ai_instruments expected but not parsed", f.filename));
            }
        }

        if !failures.is_empty() {
            panic!(
                "{}/{} cases failed:\n{}",
                failures.len(),
                fixtures.len(),
                failures.join("\n")
            );
        }
    }

    #[test]
    fn test_parse_comma_separated_lowercase_headers() {
        // Single-line response with comma/period delimiters and mixed-case headers
        let content = "MUSIC: yes, genres: dance, deephouse, electronic, in mood: happy, summer. Instruments: synth, synth_lead. Description: A dance track perfect for a beach party, with a deep, summer vibe, featuring synthetic instruments and a lead synth that creates a happy mood.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("dance, deephouse, electronic"));
        assert_eq!(res.ai_mood.as_deref(), Some("happy, summer"));
        assert_eq!(res.ai_instruments.as_deref(), Some("synth, synth_lead"));
        assert!(res.description.as_deref().unwrap().starts_with("A dance track perfect"));
    }

    #[test]
    fn test_parse_no_delimiter_headers() {
        let content = "MUSIC: yes GENRE: country, outlaw country Mood: drinking, partying Instruments: acoustic guitar, fiddle, pedal steel, harmonica, drums, banjo DESCRIPTION: This track is a lively example of outlaw country music, with a festive mood that fits well into a setting of drinking and partying. The acoustic guitar and fiddle lead the melody, backed up by the pedal steel, harmonica, drums, and banjo.";
        let res = parse_qwen_response(content);
        assert_eq!(res.is_music, Some(1));
        assert_eq!(res.ai_genre.as_deref(), Some("country, outlaw country"));
        assert_eq!(res.ai_mood.as_deref(), Some("drinking, partying"));
        assert_eq!(
            res.ai_instruments.as_deref(),
            Some("acoustic guitar, fiddle, pedal steel, harmonica, drums, banjo")
        );
        assert!(res.description.as_deref().unwrap().starts_with("This track is a lively"));
    }

    #[test]
    fn test_qwen_pass_dependencies_include_clap() {
        let pass = QwenPass;
        let deps = pass.dependencies();
        assert!(deps.contains(&"clap"));
        assert!(deps.contains(&"audio_analysis"));
        assert!(deps.contains(&"bpm_correction"));
    }
}
