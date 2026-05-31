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
                "SELECT tp.id, tp.track_id, t.path, t.bpm, t.key, t.scale, t.genre
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
            "\nListen carefully and respond strictly in English and using ONLY the following format, one field per line, nothing else:\n\n\
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

        let port = crate::llama::get_llama_port(app)
            .ok_or_else(|| "[qwen] llama-server port not available; was ensure_llama_server_running called?".to_string())?;
        let api_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);
        log::info!(
            "[qwen] Dispatching audio to local llama-server completions endpoint for track {}...",
            job.track_id
        );

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
        if let Some(conn_mutex) = app.try_state::<std::sync::Mutex<rusqlite::Connection>>() {
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
        priority: 30,
        version: pass_version::QWEN,
        dependencies: &["audio_analysis", "bpm_correction"],
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
    let mut cut_points: Vec<usize> = vec![0];

    for kw in FIELD_KEYWORDS {
        let pat = format!("{}:", kw);
        let mut from = 0;
        while let Some(rel) = lower[from..].find(pat.as_str()) {
            let kw_start = from + rel;
            if kw_start > 0 {
                // Only cut here if the keyword is preceded by a field delimiter
                let preceding = lower[..kw_start].trim_end();
                if let Some(last) = preceding.chars().last() {
                    if last == ',' || last == '.' || last == ';' {
                        cut_points.push(kw_start);
                    }
                }
            }
            from = kw_start + 1;
        }
    }

    if cut_points.len() <= 1 {
        return vec![segment];
    }
    cut_points.sort_unstable();
    cut_points.dedup();

    let last_start = *cut_points.last().unwrap();
    let non_last: Vec<&str> = cut_points
        .windows(2)
        .map(|w| {
            segment[w[0]..w[1]]
                .trim_start_matches(|c: char| c == ',' || c == '.' || c == ' ')
                .trim()
                // Strip trailing field delimiters (comma, period, semicolon) from
                // non-final segments — these are separators, not sentence punctuation.
                .trim_end_matches(|c: char| c == ',' || c == '.' || c == ';')
                .trim()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let last = segment[last_start..]
        .trim_start_matches(|c: char| c == ',' || c == '.' || c == ' ')
        .trim();
    non_last
        .into_iter()
        .chain(std::iter::once(last).filter(|s| !s.is_empty()))
        .collect()
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
}
