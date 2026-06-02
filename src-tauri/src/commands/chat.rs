use std::io::BufRead;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

// ── Domain types ──────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatSession {
    pub id: i64,
    pub track_id: i64,
    pub title: String,
    pub window_start_secs: Option<f64>,
    pub window_duration_secs: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ChatSearchResult {
    pub session_id: i64,
    pub track_id: i64,
    pub track_title: String,
    pub session_title: String,
    pub excerpt: String,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Session commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_chat_session(
    track_id: i64,
    window_start_secs: Option<f64>,
    window_duration_secs: Option<f64>,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<ChatSession, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let now = now_ms();
    conn.execute(
        "INSERT INTO chat_sessions (track_id, title, window_start_secs, window_duration_secs, created_at, updated_at)
         VALUES (?1, 'New Chat', ?2, ?3, ?4, ?4)",
        rusqlite::params![track_id, window_start_secs, window_duration_secs, now],
    ).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(ChatSession { id, track_id, title: "New Chat".into(), window_start_secs, window_duration_secs, created_at: now, updated_at: now })
}

#[tauri::command]
pub fn list_chat_sessions(
    track_id: i64,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<Vec<ChatSession>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, track_id, title, window_start_secs, window_duration_secs, created_at, updated_at
         FROM chat_sessions WHERE track_id = ?1 ORDER BY updated_at DESC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![track_id], |row| {
        Ok(ChatSession {
            id: row.get(0)?,
            track_id: row.get(1)?,
            title: row.get(2)?,
            window_start_secs: row.get(3)?,
            window_duration_secs: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe_ok()
}

#[tauri::command]
pub fn rename_chat_session(
    session_id: i64,
    title: String,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<(), String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE chat_sessions SET title = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![title, now_ms(), session_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_chat_session(
    session_id: i64,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<(), String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM chat_sessions WHERE id = ?1", rusqlite::params![session_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Message commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_chat_messages(
    session_id: i64,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<Vec<ChatMessage>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, created_at
         FROM chat_messages WHERE session_id = ?1 ORDER BY created_at ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![session_id], |row| {
        Ok(ChatMessage {
            id: row.get(0)?,
            session_id: row.get(1)?,
            role: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe_ok()
}

#[tauri::command]
pub fn save_chat_message(
    session_id: i64,
    role: String,
    content: String,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<ChatMessage, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let now = now_ms();
    conn.execute(
        "INSERT INTO chat_messages (session_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session_id, role, content, now],
    ).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    // Auto-title session from first user message if still default
    if role == "user" {
        let short: String = content.chars().take(60).collect();
        let title = if content.len() > 60 { format!("{}…", short) } else { short };
        conn.execute(
            "UPDATE chat_sessions SET title = ?1, updated_at = ?2
             WHERE id = ?3 AND title = 'New Chat'",
            rusqlite::params![title, now, session_id],
        ).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "UPDATE chat_sessions SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, session_id],
        ).map_err(|e| e.to_string())?;
    }
    Ok(ChatMessage { id, session_id, role, content, created_at: now })
}

// ── Search ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_chats(
    query: String,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<Vec<ChatSearchResult>, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT cs.id, cs.track_id,
                COALESCE(t.title, t.filename, '') AS track_title,
                cs.title,
                snippet(chat_messages_fts, 0, '', '', '…', 20) AS excerpt
         FROM chat_messages_fts
         JOIN chat_messages cm ON cm.id = chat_messages_fts.rowid
         JOIN chat_sessions cs ON cs.id = cm.session_id
         JOIN tracks t ON t.id = cs.track_id
         WHERE chat_messages_fts MATCH ?1
         ORDER BY rank
         LIMIT 50",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![query], |row| {
        Ok(ChatSearchResult {
            session_id: row.get(0)?,
            track_id: row.get(1)?,
            track_title: row.get(2)?,
            session_title: row.get(3)?,
            excerpt: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;
    rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe_ok()
}

trait PipeOk<T> { fn pipe_ok(self) -> Result<T, String>; }
impl<T> PipeOk<T> for T { fn pipe_ok(self) -> Result<T, String> { Ok(self) } }

#[tauri::command]
pub async fn ask_qwen(
    app: tauri::AppHandle,
    track_id: i64,
    question: String,
    window_start_secs: Option<f64>,
    window_duration_secs: Option<f64>,
    history: Vec<(String, String)>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        ask_qwen_blocking(app, track_id, question, window_start_secs, window_duration_secs, history)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

fn ask_qwen_blocking(
    app: tauri::AppHandle,
    track_id: i64,
    question: String,
    window_start_secs: Option<f64>,
    window_duration_secs: Option<f64>,
    history: Vec<(String, String)>,
) -> Result<String, String> {
    // 1. Look up track path
    let path = {
        let conn_state = app.state::<Mutex<rusqlite::Connection>>();
        let conn = conn_state.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT path FROM tracks WHERE id = ?1",
            rusqlite::params![track_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|e| format!("Track not found: {}", e))?
    };

    // 2. Ensure llama-server is running (keep it alive after this command)
    let guard = crate::llama::ensure_llama_server_running(&app)?;
    std::mem::forget(guard);

    // 3. Decode and resample to 16 kHz mono WAV.
    //    The bundled llama-server only handles WAV reliably via the
    //    input_audio API — native formats like MP3 produce garbage output.
    //    NaN/Inf from malformed frames are zeroed before encoding;
    //    encode_audio_to_wav additionally clamps to [-1, 1].
    let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(&path)?;
    let audio_16k = crate::spectrogram::resample_to_16k(&audio, sample_rate)?;

    // 4-minute hard cap keeps payload within llama-server's context budget.
    // The frontend always passes an explicit window (from the WaveSurfer region
    // selector), so the None branch acts as a safety net for callers that don't.
    const MAX_SECS: f64 = 240.0;
    let window: Vec<f32> = if let (Some(start), Some(dur)) = (window_start_secs, window_duration_secs) {
        // Clamp duration to the hard cap so the user can't exceed it even if
        // they somehow select a longer region on the frontend.
        let dur = dur.min(MAX_SECS);
        let start_idx = ((start * 16000.0) as usize).min(audio_16k.len());
        let end_idx = (((start + dur) * 16000.0) as usize).min(audio_16k.len());
        audio_16k[start_idx..end_idx].to_vec()
    } else if audio_16k.len() > (MAX_SECS * 16000.0) as usize {
        // Fallback: centre a 4-minute window on the track midpoint
        let max_samples = (MAX_SECS * 16000.0) as usize;
        let mid = audio_16k.len() / 2;
        let half = max_samples / 2;
        let start = mid.saturating_sub(half);
        audio_16k[start..(start + max_samples).min(audio_16k.len())].to_vec()
    } else {
        audio_16k
    };

    let window: Vec<f32> = window
        .into_iter()
        .map(|s| if s.is_finite() { s } else { 0.0 })
        .collect();

    let wav_bytes = crate::dsp::encode_audio_to_wav(&window, 16000);
    let base64_audio = crate::dsp::base64_encode(&wav_bytes);

    // 4. Build messages — audio attached only to the first user turn
    let mut msgs: Vec<serde_json::Value> = Vec::new();
    let audio_content = serde_json::json!([
        { "type": "input_audio", "input_audio": { "data": base64_audio, "format": "wav" } },
        { "type": "text", "text": if history.is_empty() { &question } else { &history[0].0 } }
    ]);

    if history.is_empty() {
        msgs.push(serde_json::json!({ "role": "user", "content": audio_content }));
    } else {
        msgs.push(serde_json::json!({ "role": "user", "content": audio_content }));
        msgs.push(serde_json::json!({ "role": "assistant", "content": &history[0].1 }));

        for (user_msg, asst_msg) in &history[1..] {
            msgs.push(serde_json::json!({ "role": "user", "content": user_msg }));
            msgs.push(serde_json::json!({ "role": "assistant", "content": asst_msg }));
        }

        msgs.push(serde_json::json!({ "role": "user", "content": question }));
    }

    // 5. Stream from llama-server, emitting tokens as they arrive
    let port = crate::llama::get_llama_port(&app)
        .ok_or("llama-server port unavailable")?;
    let api_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let resp = match ureq::post(&api_url)
        .timeout(std::time::Duration::from_secs(180))
        .send_json(&serde_json::json!({ "messages": msgs, "stream": true }))
    {
        Ok(r) => r,
        Err(ureq::Error::Status(400, _)) => {
            return Err(
                "The audio model couldn't process this track — it may be corrupted or \
                 the selected region is too long. Try selecting a shorter section."
                    .to_string(),
            );
        }
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            return Err(format!("llama-server returned HTTP {}: {}", code, body));
        }
        Err(e) => return Err(format!("llama-server request failed: {}", e)),
    };

    let reader = std::io::BufReader::new(resp.into_reader());
    let mut full_response = String::new();

    for line in reader.lines() {
        let line = line.map_err(|e| format!("SSE read error: {}", e))?;
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(token) = json["choices"][0]["delta"]["content"].as_str() {
                    full_response.push_str(token);
                    app.emit("chat_token", token).ok();
                }
            }
        }
    }

    Ok(full_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    fn insert_track(conn: &rusqlite::Connection) -> i64 {
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/music')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO tracks (watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds)
             VALUES (1, '/music/track.mp3', 'track.mp3', 0, 0, 180)",
            [],
        ).unwrap();
        conn.last_insert_rowid()
    }

    fn insert_session(conn: &rusqlite::Connection, track_id: i64) -> i64 {
        let now = now_ms();
        conn.execute(
            "INSERT INTO chat_sessions (track_id, title, created_at, updated_at) VALUES (?1, 'New Chat', ?2, ?2)",
            rusqlite::params![track_id, now],
        ).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn test_save_message_auto_titles_session_from_first_user_message() {
        let conn = setup_test_db();
        let track_id = insert_track(&conn);
        let session_id = insert_session(&conn, track_id);

        // First user message should set the session title
        let question = "What's the key of this track?";
        conn.execute(
            "INSERT INTO chat_messages (session_id, role, content, created_at) VALUES (?1, 'user', ?2, ?3)",
            rusqlite::params![session_id, question, now_ms()],
        ).unwrap();
        let short: String = question.chars().take(60).collect();
        conn.execute(
            "UPDATE chat_sessions SET title = ?1 WHERE id = ?2 AND title = 'New Chat'",
            rusqlite::params![short, session_id],
        ).unwrap();

        let title: String = conn.query_row(
            "SELECT title FROM chat_sessions WHERE id = ?1",
            rusqlite::params![session_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, question);
    }

    #[test]
    fn test_save_message_truncates_long_title_at_60_chars() {
        let conn = setup_test_db();
        let track_id = insert_track(&conn);
        let session_id = insert_session(&conn, track_id);

        let long_question = "Can you describe the overall mood and emotional feel of the intro section in detail?";
        assert!(long_question.len() > 60);

        let short: String = long_question.chars().take(60).collect();
        let title = format!("{}…", short);
        conn.execute(
            "UPDATE chat_sessions SET title = ?1 WHERE id = ?2 AND title = 'New Chat'",
            rusqlite::params![title, session_id],
        ).unwrap();

        let saved_title: String = conn.query_row(
            "SELECT title FROM chat_sessions WHERE id = ?1",
            rusqlite::params![session_id],
            |r| r.get(0),
        ).unwrap();
        assert!(saved_title.chars().count() <= 62); // 60 chars + "…"
        assert!(saved_title.ends_with('…'));
    }

    #[test]
    fn test_search_chats_finds_message_via_fts() {
        let conn = setup_test_db();
        let track_id = insert_track(&conn);
        let session_id = insert_session(&conn, track_id);
        let now = now_ms();

        conn.execute(
            "INSERT INTO chat_messages (session_id, role, content, created_at) VALUES (?1, 'user', 'The bassline feels very muddy', ?2)",
            rusqlite::params![session_id, now],
        ).unwrap();

        let mut stmt = conn.prepare(
            "SELECT cs.id FROM chat_messages_fts
             JOIN chat_messages cm ON cm.id = chat_messages_fts.rowid
             JOIN chat_sessions cs ON cs.id = cm.session_id
             WHERE chat_messages_fts MATCH 'muddy'",
        ).unwrap();
        let results: Vec<i64> = stmt.query_map([], |r| r.get(0)).unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], session_id);
    }

    #[test]
    fn test_search_chats_does_not_find_unrelated_message() {
        let conn = setup_test_db();
        let track_id = insert_track(&conn);
        let session_id = insert_session(&conn, track_id);
        let now = now_ms();

        conn.execute(
            "INSERT INTO chat_messages (session_id, role, content, created_at) VALUES (?1, 'user', 'The bassline feels very muddy', ?2)",
            rusqlite::params![session_id, now],
        ).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chat_messages_fts WHERE chat_messages_fts MATCH 'reverb'",
            [],
            |r| r.get(0),
        ).unwrap();

        assert_eq!(count, 0);
    }
}
