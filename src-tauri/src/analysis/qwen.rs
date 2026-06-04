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
    pub is_music: Option<i64>,
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
        &["audio_analysis", "bpm_correction", "clap", "essentia"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &[
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
                        t.waveform_data, t.is_music, t.duration_seconds
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
                    is_music: row.get(8)?,
                    duration_seconds: row.get(9)?,
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
        if job.is_music == Some(0) {
            log::info!("[qwen] Track {} is non-music. Skipping.", job.track_id);
            return Ok(QwenOutput {
                parsed: ParsedQwenResponse { ai_genre: None, ai_mood: None, ai_instruments: None, description: None },
                tags: Vec::new(),
                raw_response: String::new(),
            });
        }

        // Ensure the llama-server is alive before processing. If it crashed (e.g. due to
        // system sleep mid-run) this will respawn it and wait for healthy before continuing.
        {
            let guard = crate::llama::ensure_llama_server_running(app)
                .map_err(|e| format!("[qwen] Failed to (re)start llama-server: {}", e))?;
            std::mem::forget(guard);
        }

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
        let window_size = 16000 * 30; // 30s
        let (start_idx, end_idx) = if audio_16k_full.len() <= window_size {
            (0, audio_16k_full.len())
        } else {
            let half = window_size / 2;
            let mut start = center_16k.saturating_sub(half);
            let mut end = start + window_size;
            if end > audio_16k_full.len() {
                end = audio_16k_full.len();
                start = end - window_size;
            }
            (start, end)
        };
        let audio_window = &audio_16k_full[start_idx..end_idx];

        // 3. Encode audio to WAV & Base64
        let wav_bytes = crate::dsp::encode_audio_to_wav(audio_window, 16000);
        let base64_audio = crate::dsp::base64_encode(&wav_bytes);

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
        let mut api_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

        let mut best_output: Option<QwenOutput> = None;
        let mut best_similarity = -1.0f32;

        // Send one chat completion request and return the assistant's text content.
        // Returns Err with a tag prefix: "SERVER:" for connection/5xx errors that warrant
        // a server restart, or "PARSE:" for well-formed responses that failed validation.
        let query_completions = |url: &str, messages_payload: &serde_json::Value| -> Result<String, String> {
            let payload = serde_json::json!({
                "messages": messages_payload,
                "temperature": 0.2
            });
            let resp = ureq::post(url)
                .timeout(std::time::Duration::from_secs(120))
                .send_json(&payload)
                .map_err(|e| format!("SERVER:Completions request to llama-server failed: {}", e));

            let resp = match resp {
                Ok(r) => r,
                Err(e) => return Err(e),
            };

            if resp.status() >= 500 {
                return Err(format!("SERVER:llama-server returned HTTP {}", resp.status()));
            }

            let resp_json = resp
                .into_json::<serde_json::Value>()
                .map_err(|e| format!("PARSE:Failed to parse completions response JSON: {}", e))?;

            let content = resp_json["choices"][0]["message"]["content"]
                .as_str()
                .ok_or_else(|| format!("PARSE:Unexpected JSON structure: {:?}", resp_json))?;

            Ok(content.to_string())
        };

        for attempt in 1..=3usize {
            log::info!(
                "[qwen] Track {} — attempt {}/3",
                job.track_id, attempt
            );

            // Build fresh conversation for each attempt
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
                                What is the genre and subgenre of this track in a few words? Respond strictly in English in this format:\n\
                                GENRE: genre and subgenre",
                                bpm, key, scale,
                                job.genre.as_ref().map_or("".to_string(), |g| format!("The file metadata tags it as \"{}\".", g))
                            )
                        }
                    ]
                })
            ];

            let mut ai_genre = None;
            let mut ai_mood = None;
            let mut ai_instruments = None;
            let mut description = None;

            // (step_name, follow-up prompt — None reuses the initial message, tag_namespace — Some emits tags)
            let steps: Vec<(&str, Option<&str>, Option<&str>)> = vec![
                ("genre",        None,        Some("genre")),
                ("mood",         Some("What is the mood and emotional feel of this track in a few words? Respond strictly in English in this format:\nMOOD: mood and emotional feel"),                                                                                                                Some("mood")),
                ("instruments",  Some("What are the main instruments playing in this track, comma-separated? Respond strictly in English in this format:\nINSTRUMENTS: main instruments"),                                                                                                            Some("inst")),
                ("description",  Some("Provide two to three sentences of plain prose describing the track. Respond strictly in English in this format:\nDESCRIPTION: description"),                                                                                                                  None),
                ("tags_vibe",    Some("Suggest 3 creative tags capturing the atmosphere, vibe, or style of this song, without repeating any genres, moods, instruments, or descriptions already discussed. Respond strictly in English in this format:\nVIBE_TAGS: ethereal, hypnotic, raw"),              Some("vibe")),
                ("tags_vocals",  Some("Identify the singer voice type (e.g. male, female, instrumental, ensemble, choir) and the lyrics language (e.g. english, spanish, instrumental), without repeating categories already discussed. Respond strictly in this format:\nVOCAL_TAGS: male, english"), Some("vocal")),
                ("tags_context", Some("Suggest 2 tags for suitable listening contexts (e.g. study, club, sleep, workout) and 1 tag for the estimated release decade (e.g. 1980s, 2000s), without repeating categories already discussed. Respond strictly in this format:\nCONTEXT_TAGS: study, workout, 1990s"), Some("context")),
            ];

            let mut all_steps_ok = true;
            let mut need_server_restart = false;
            let mut pending_tags: Vec<(String, String)> = Vec::new();

            for (step_name, step_prompt, tag_namespace) in steps {
                if let Some(prompt_text) = step_prompt {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": prompt_text
                    }));
                }

                match query_completions(&api_url, &serde_json::json!(messages)) {
                    Ok(content) => {
                        let has_chinese = content.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
                        if has_chinese {
                            log::warn!("[qwen] Track {} step '{}' returned Chinese — retrying song from scratch", job.track_id, step_name);
                            all_steps_ok = false;
                            break;
                        }

                        // Strip any "LABEL: " prefix the model echoed back; use the whole
                        // content as fallback so verbose responses are stored rather than lost.
                        let value = clean_qwen_tags(&content, step_name);

                        if step_name == "description" {
                            if is_invalid_description(&value) {
                                log::warn!(
                                    "[qwen] Track {} step 'description' returned invalid/generic text: {:?} — retrying attempt",
                                    job.track_id, value
                                );
                                all_steps_ok = false;
                                break;
                            }
                        }

                        if let Some(ns) = tag_namespace {
                            for token in value.split(',') {
                                let label = token.trim().to_string();
                                if !label.is_empty() {
                                    pending_tags.push((ns.to_string(), label));
                                }
                            }
                        }

                        match step_name {
                            "genre" => ai_genre = Some(value),
                            "mood" => ai_mood = Some(value),
                            "instruments" => ai_instruments = Some(value),
                            "description" => description = Some(value),
                            _ => {}
                        }
                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": content
                        }));
                    }
                    Err(e) => {
                        log::warn!("[qwen] Track {} step '{}' failed: {}", job.track_id, step_name, e);
                        if e.starts_with("SERVER:") {
                            need_server_restart = true;
                        }
                        all_steps_ok = false;
                        break;
                    }
                }
            }

            if need_server_restart {
                log::warn!("[qwen] Server error on track {} — restarting llama-server before next attempt", job.track_id);
                match crate::llama::ensure_llama_server_running(app) {
                    Ok(guard) => {
                        std::mem::forget(guard);
                        if let Some(new_port) = crate::llama::get_llama_port(app) {
                            api_url = format!("http://127.0.0.1:{}/v1/chat/completions", new_port);
                            log::info!("[qwen] llama-server restarted on port {}", new_port);
                        }
                    }
                    Err(restart_err) => {
                        log::error!("[qwen] Failed to restart llama-server: {}", restart_err);
                    }
                }
            }

            if !all_steps_ok {
                continue;
            }

            let parsed = ParsedQwenResponse {
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
                tags: pending_tags.clone(),
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
                    "[qwen] Track {} failed CLAP verification (similarity {:.4} < 0.28) on attempt {}/3",
                    job.track_id, similarity, attempt
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
        let ai_genre_val = output.parsed.ai_genre.as_deref();
        let ai_mood_val = output.parsed.ai_mood.as_deref();
        let ai_instruments_val = output.parsed.ai_instruments.as_deref();
        let description_val = output.parsed.description.as_deref();

        conn.execute(
            "UPDATE tracks SET
                ai_genre = ?1,
                ai_mood = ?2,
                ai_instruments = ?3,
                description = ?4
             WHERE id = ?5",
            rusqlite::params![
                ai_genre_val,
                ai_mood_val,
                ai_instruments_val,
                description_val,
                job.track_id
            ],
        )
        .map_err(|e| e.to_string())?;

        // Wipe previous qwen tags then insert the ones accumulated by this run's steps
        conn.execute(
            "DELETE FROM track_tags WHERE track_id = ?1 AND source = 'qwen'",
            rusqlite::params![job.track_id],
        ).map_err(|e| e.to_string())?;

        for (namespace, label) in &output.tags {
            super::upsert_track_tag(conn, job.track_id, namespace, label, "qwen")?;
        }

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
            output.parsed.ai_genre.as_ref().map(|_| "genre"),
            output.parsed.ai_mood.as_ref().map(|_| "mood"),
            output.parsed.ai_instruments.as_ref().map(|_| "instruments"),
            output.parsed.description.as_ref().map(|_| "description"),
        ]
        .iter()
        .filter_map(|x| *x)
        .collect();
        let all_fields = ["genre", "mood", "instruments", "description"];
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
        dependencies: &["audio_analysis", "bpm_correction", "clap", "essentia"],
        owned_columns: &[
            "ai_genre",
            "ai_mood",
            "ai_instruments",
            "description",
        ],
        owned_tables: &["description_embeddings"],
        owned_tag_sources: &["qwen"],
        custom_reset: None,
    };
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct QwenOutput {
    pub parsed: ParsedQwenResponse,
    /// Tags accumulated from steps that declare a namespace: (namespace, label).
    pub tags: Vec<(String, String)>,
    pub raw_response: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Clone)]
pub struct ParsedQwenResponse {
    pub ai_genre: Option<String>,
    pub ai_mood: Option<String>,
    pub ai_instruments: Option<String>,
    pub description: Option<String>,
}


fn is_invalid_description(desc: &str) -> bool {
    let val_lower = desc.trim().to_lowercase();
    val_lower == "not provided"
        || val_lower == "description"
        || val_lower == "description."
        || val_lower.contains("not provided in the provided information")
}

/// Strip an echoed label prefix from a model response, tolerating variations in case,
/// markdown decoration, plural, and separator style.
///
/// Examples that are all stripped to "Electronic, Pop":
///   "GENRE: Electronic, Pop"  |  "genre. Electronic, Pop"
///   "Genre Electronic, Pop"   |  "**Genres**: Electronic, Pop"
fn strip_label_prefix(content: &str, step_name: &str) -> String {
    let label = match step_name {
        "genre"        => "genre",
        "mood"         => "mood",
        "instruments"  => "instrument", // matches both "instrument" and "instruments"
        "description"  => "description",
        "tags_vibe"    => "vibe_tag",   // matches "vibe_tags", "vibe_tag"
        "tags_vocals"  => "vocal_tag",  // matches "vocal_tags", "vocal_tag"
        "tags_context" => "context_tag", // matches "context_tags", "context_tag"
        _ => return content.trim().to_string(),
    };

    let trimmed = content.trim();
    let lower   = trimmed.to_lowercase();
    let mut pos = 0usize;

    // Skip any leading markdown decoration (* _ # space)
    let decoration: &[char] = &['*', '_', '#', ' '];
    pos += skip_chars(&lower, pos, decoration);

    // Require the label to appear next (ASCII only, so byte == char offset)
    if !lower[pos..].starts_with(label) {
        return trimmed.to_string();
    }
    pos += label.len();

    // Optional plural 's'
    if lower[pos..].starts_with('s') {
        pos += 1;
    }

    // Skip trailing markdown decoration after the label word
    pos += skip_chars(&lower, pos, decoration);

    // Skip separator(s): colon, period, dash, space — any combination
    let separators: &[char] = &[':', '.', '-', ' ', '\t'];
    pos += skip_chars(&lower, pos, separators);

    let rest = trimmed[pos..].trim();
    if rest.is_empty() { trimmed.to_string() } else { rest.to_string() }
}

fn skip_chars(s: &str, from: usize, chars: &[char]) -> usize {
    s[from..].len() - s[from..].trim_start_matches(chars).len()
}

/// Clean, normalize, and strip narrative framing and fillers from Qwen responses
/// based on the target field name (genre, mood, instruments, description).
fn clean_qwen_tags(content: &str, step_name: &str) -> String {
    // 1. Initial label strip
    let stripped = strip_label_prefix(content, step_name);
    if step_name == "description" {
        return stripped;
    }
    // Tags steps use the same cleaning pipeline as genre/mood/inst
    // (no special-casing needed beyond what follows)

    let cleaned = stripped.to_lowercase();

    // Early return if the response indicates missing or unknown information
    if cleaned.contains("not explicitly")
        || cleaned.contains("not specified")
        || cleaned.contains("cannot be determined")
        || cleaned.contains("cannot be identified")
        || cleaned.contains("unknown since")
        || cleaned.contains("no specific")
        || cleaned.contains("not provided")
        || cleaned.contains("does not provide")
        || cleaned.contains("none specified")
        || cleaned.contains("no instruments")
        || cleaned.contains("no mood")
        || cleaned.contains("n/a")
        || cleaned == "none"
        || cleaned == "unknown"
        || cleaned == "specified"
    {
        return "".to_string();
    }

    // Replace period, slashes, and quotes with comma/space before tokenizing
    let mut normalized = cleaned
        .replace('.', " ")
        .replace('/', ", ")
        .replace('\'', "")
        .replace('\"', "");

    // 2. Multi-word Whitelist Protection
    // Protect these terms from being split into separate words by underscoring them
    const MULTI_WORD_WHITELIST: &[&str] = &[
        "acoustic guitar", "electric guitar", "electric bass", "acoustic bass",
        "pedal steel guitar", "pedal steel", "steel guitar", "drum machine",
        "synthesizer pad", "synthesizer pads", "synth pads", "synth pad",
        "string section", "brass instruments", "brass section", "wind instruments",
        "woodwind instruments", "backing vocals", "lead vocals", "lead guitar",
        "alternative rock", "classic rock", "hard rock", "industrial rock",
        "noise rock", "psychedelic rock", "country rock", "roots reggae",
        "hip hop", "acid jazz", "free jazz", "ambient techno", "deephouse",
        "deep house", "minimal techno", "liquid funk", "lo fi", "avant garde",
        "new age", "easy listening", "folk rock", "folk pop", "synth pop",
        "indie rock", "indie pop", "slow tempo", "fast paced", "heart broken",
        "laid back", "dark ambient", "spacey synth", "sound collage", "chicago blues",
        "harmonica blues", "sertanejo universitário", "singer songwriter", "progressive rock",
        "symphonic rock", "classic soul", "electroacoustic", "ambient pop", "art rock",
        "heavy metal", "dream pop", "post punk", "big band", "free improvisation",
        "chamber music", "chamber pop", "baroque pop", "indie folk"
    ];

    for term in MULTI_WORD_WHITELIST {
        normalized = normalized.replace(term, &term.replace(' ', "_"));
    }

    // Normalize separators like " and " or " or " to commas
    normalized = normalized.replace(" and ", ", ");
    normalized = normalized.replace(" or ", ", ");

    // 3. Word-Level Grammatical Structure Downvoting
    // Split into words to find grammatical narrative regions (e.g. "the ... are")
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let mut penalties = vec![0.0f32; words.len()];

    let start_markers = &["the", "this"];
    let end_markers = &["is", "are", "belongs", "belong", "consists", "consist", "falls", "has"];

    let mut in_narrative = false;
    for i in 0..words.len() {
        let w = words[i];
        if start_markers.contains(&w) {
            in_narrative = true;
        } else if end_markers.contains(&w) {
            in_narrative = false;
        } else if in_narrative {
            penalties[i] = -1.0;
        }
    }

    // 4. Scoring & Filtering
    const COMBINED_WHITELIST: &[&str] = &[
        "guitar", "guitarist", "bass", "bassist", "piano", "pianist", "violin",
        "violinist", "synth", "synthesizer", "synths", "synthesizers", "drums",
        "drum", "drummer", "drumming", "vocals", "vocal", "vocalist", "percussion",
        "saxophone", "sax", "trumpet", "cello", "flute", "accordion", "sitar",
        "oud", "banjo", "harmonica", "cavaquinho", "fiddle", "claps", "clap",
        "techno", "house", "ambient", "trance", "blues", "rock", "pop", "jazz",
        "classical", "folk", "country", "americana", "samba", "mpb", "sertanejo",
        "gospel", "soul", "funk", "soundtrack", "opera", "rap", "reggae",
        "disco", "ska", "dub", "glitch", "contemplative", "relaxing", "calm",
        "melodic", "sentimental", "introspective", "reflective", "laidback",
        "energetic", "joyful", "happy", "sad", "dark", "hypnotic", "uplifting",
        "atmospheric", "dreamy", "inspiring", "heavy", "epic", "intense",
        "nostalgic", "romantic", "upbeat", "chill", "cold", "chaotic", "dissonant",
        "minimal", "soundtrack", "world", "classic", "hopeful", "peaceful",
        "melancholic", "lively", "positive", "festive", "heartfelt", "vivacious"
    ];

    const STOPWORDS: &[&str] = &[
        // Format placeholder echoes from prompt templates
        "tag1", "tag2", "tag3", "context1", "context2", "decade",
        "voice_type", "language", "era_decade",
        "the", "a", "an", "and", "or", "in", "of", "on", "at", "by", "for", "with",
        "about", "to", "this", "that", "it", "is", "are", "was", "were", "be",
        "been", "being", "belongs", "belong", "consists", "consist", "features",
        "feature", "featured", "featuring", "include", "includes", "including",
        "main", "major", "minor", "likely", "possibly", "possible", "some",
        "subtle", "prominent", "various", "dominant", "traditional", "genre",
        "genres", "subgenre", "subgenres", "mood", "moods", "feel", "feeling",
        "feelings", "vibe", "vibes", "track", "tracks", "song", "songs", "music",
        "piece", "pieces", "instrument", "instruments", "instrumental",
        "instrumentation", "sound", "sounds", "soundscape", "elements",
        "element", "influences", "influence", "highly", "extremely", "very",
        "described", "describe", "description", "accompanied", "accompaniment",
        "specifically", "style", "styles", "falls", "under", "has", "its", "more",
        "often", "referred", "as", "classification", "categorized", "classified",
        "type", "category", "label", "labeled", "tagged", "feels", "character",
        "characteristics", "aspects", "aspect", "typical", "typically", "associated",
        "aims", "evoke", "evokes", "evocative", "evoking", "conveys", "convey",
        "conveying", "impression", "reminiscent", "delivers", "delivering",
        "presents", "presenting", "sense", "inner"
    ];

    let mut result_tokens = Vec::new();

    // Rebuild the string with words joined by spaces
    let rebuilt_normalized = words.join(" ");

    for token in rebuilt_normalized.split(',') {
        let clean_token = token.trim();
        if clean_token.is_empty() {
            continue;
        }

        // We split the token into individual words to calculate scores
        let token_words: Vec<&str> = clean_token.split_whitespace().collect();
        let mut clean_token_words = Vec::new();

        for tw in token_words {
            let w_clean = tw.trim_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string();
            if w_clean.is_empty() {
                continue;
            }

            // Exclude noise
            if w_clean == "n/a" || w_clean == "none" || w_clean == "not specified" || w_clean == "unknown" || w_clean == "specified" || w_clean == "n" {
                continue;
            }

            // Find index in original words list to get penalty
            let mut penalty = 0.0f32;
            if let Some(pos) = words.iter().position(|&orig| {
                orig.trim_matches(|c: char| !c.is_alphanumeric() && c != '_') == w_clean
            }) {
                penalty = penalties[pos];
            }

            let mut score = 0.0f32 + penalty;

            if w_clean.contains('_') || COMBINED_WHITELIST.contains(&w_clean.as_str()) {
                score += 1.5;
            }

            if STOPWORDS.contains(&w_clean.as_str()) {
                score -= 1.5;
            }

            if score >= 0.0 {
                clean_token_words.push(w_clean.replace('_', " "));
            }
        }

        if !clean_token_words.is_empty() {
            let assembled_token = clean_token_words.join(" ");
            if !result_tokens.contains(&assembled_token) {
                result_tokens.push(assembled_token);
            }
        }
    }

    result_tokens.join(", ")
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::AnalysisPass;

    #[test]
    fn test_strip_label_prefix() {
        let cases = [
            ("genre",       "GENRE: Electronic, Pop",      "Electronic, Pop"),
            ("genre",       "genre. Electronic, Pop",      "Electronic, Pop"),
            ("genre",       "Genre Electronic, Pop",       "Electronic, Pop"),
            ("genre",       "**Genres**: Electronic, Pop", "Electronic, Pop"),
            ("genre",       "GENRE- Electronic, Pop",      "Electronic, Pop"),
            ("mood",        "MOOD: dark, hypnotic",        "dark, hypnotic"),
            ("mood",        "mood. dark, hypnotic",        "dark, hypnotic"),
            ("mood",        "Mood dark, hypnotic",         "dark, hypnotic"),
            ("instruments", "INSTRUMENTS: guitar, bass",   "guitar, bass"),
            ("instruments", "instrument. guitar, bass",    "guitar, bass"),
            ("description", "DESCRIPTION: A mellow track.", "A mellow track."),
            // No matching prefix — return as-is
            ("genre",       "Just plain text",             "Just plain text"),
        ];
        for (step, input, expected) in cases {
            assert_eq!(
                super::strip_label_prefix(input, step),
                expected,
                "step={step:?} input={input:?}"
            );
        }
    }

    #[test]
    fn test_clean_qwen_tags() {
        let cases = [
            // Instruments
            ("instruments", "INSTRUMENTS: acoustic guitar, drums", "acoustic guitar, drums"),
            ("instruments", "The main instruments in this track are piano and violin.", "piano, violin"),
            ("instruments", "subtle synthesizer pads, prominent drums", "synthesizer pads, drums"),
            ("instruments", "traditional acoustic guitar instruments, various percussion instruments", "acoustic guitar, percussion"),
            ("instruments", "not specified", ""),
            ("instruments", "the instruments in the track are not specified.", ""),
            ("instruments", "None specified", ""),
            ("instruments", "n/a", ""),
            ("instruments", "specified", ""),
            
            // Genre
            ("genre", "GENRE: traditional country style", "country"),
            ("genre", "This piece belongs to electronic / techno and house subgenres.", "electronic, techno, house"),
            ("genre", "The genre of the track is techno.", "techno"),
            ("genre", "the genre of the track is 'ambient, soundtrack' and the subgenre is 'newage'.", "ambient, soundtrack, newage"),
            ("genre", "rock, falls under classic rock", "rock, classic rock"),
            ("genre", "pop, its experimental pop", "pop, experimental pop"),
            ("genre", "pop, international pop, often referred as world pop", "pop, international pop, world pop"),
            ("genre", "sertanejo, sertanejo universitário", "sertanejo, sertanejo universitário"),
            ("genre", "the genre is rock and the subgenre is industrial.", "rock, industrial"),
            
            // Mood
            ("mood", "MOOD: highly energetic vibe", "energetic"),
            ("mood", "extremely calm feel, introspective vibes", "calm, introspective"),
            ("mood", "The mood of the track is introspective and reflective.", "introspective, reflective"),
            ("mood", "the mood and emotional feel of this track cannot be determined from the provided information.", ""),
            ("mood", "emotional, aims evoke sense inner peace", "emotional, peace"),
            ("mood", "None specified", ""),
            ("mood", "n/a", ""),
            ("mood", "n", ""),
        ];
        for (step, input, expected) in cases {
            assert_eq!(
                super::clean_qwen_tags(input, step),
                expected,
                "step={step:?} input={input:?}"
            );
        }
    }

    #[test]
    fn test_is_invalid_description() {
        assert!(super::is_invalid_description("Not provided"));
        assert!(super::is_invalid_description("description"));
        assert!(super::is_invalid_description("description."));
        assert!(super::is_invalid_description("The description of the track is not provided in the provided information."));
        
        // Valid descriptions
        assert!(!super::is_invalid_description("A beautiful and ambient classical piece with soft pianos."));
    }

    #[test]
    fn test_qwen_pass_dependencies_include_clap() {
        let pass = QwenPass;
        let deps = pass.dependencies();
        assert!(deps.contains(&"clap"));
        assert!(deps.contains(&"audio_analysis"));
        assert!(deps.contains(&"bpm_correction"));
        assert!(deps.contains(&"essentia"));
    }
}
