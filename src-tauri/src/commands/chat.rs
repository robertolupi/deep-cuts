use std::io::BufRead;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

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

    // 3. Decode and resample audio to 16 kHz
    let (audio, sample_rate) = crate::dsp::decode_audio_to_mono(&path)?;
    let audio_16k = crate::spectrogram::resample_to_16k(&audio, sample_rate)?;

    // 4. Slice to requested window, capping at 180 s when no window is specified
    const MAX_DEFAULT_SECS: f64 = 180.0;
    let audio_window: Vec<f32> = if let (Some(start), Some(dur)) = (window_start_secs, window_duration_secs) {
        let start_idx = ((start * 16000.0) as usize).min(audio_16k.len());
        let end_idx = (((start + dur) * 16000.0) as usize).min(audio_16k.len());
        audio_16k[start_idx..end_idx].to_vec()
    } else {
        let max_samples = (MAX_DEFAULT_SECS * 16000.0) as usize;
        if audio_16k.len() <= max_samples {
            audio_16k
        } else {
            // Centre the window on the midpoint of the track
            let mid = audio_16k.len() / 2;
            let half = max_samples / 2;
            let start = mid.saturating_sub(half);
            let end = (start + max_samples).min(audio_16k.len());
            audio_16k[start..end].to_vec()
        }
    };

    // 5. Sanitise samples — clamp to [-1, 1] and replace NaN/Inf produced by
    //    malformed frames (e.g. MP3 bitstream overruns) before encoding.
    //    Without this, bad samples can trigger a segfault inside llama.cpp's
    //    audio encoder even when the file plays back fine in a player.
    let audio_window: Vec<f32> = audio_window
        .into_iter()
        .map(|s| if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 })
        .collect();

    // 6. Encode to base64 WAV
    let wav_bytes = crate::dsp::encode_audio_to_wav(&audio_window, 16000);
    let base64_audio = crate::dsp::base64_encode(&wav_bytes);

    // 7. Build messages array — audio is only attached to the first user turn
    let mut msgs: Vec<serde_json::Value> = Vec::new();

    if history.is_empty() {
        msgs.push(serde_json::json!({
            "role": "user",
            "content": [
                { "type": "input_audio", "input_audio": { "data": base64_audio, "format": "wav" } },
                { "type": "text", "text": question }
            ]
        }));
    } else {
        let (first_user, first_asst) = &history[0];
        msgs.push(serde_json::json!({
            "role": "user",
            "content": [
                { "type": "input_audio", "input_audio": { "data": base64_audio, "format": "wav" } },
                { "type": "text", "text": first_user }
            ]
        }));
        msgs.push(serde_json::json!({ "role": "assistant", "content": first_asst }));

        for (user_msg, asst_msg) in &history[1..] {
            msgs.push(serde_json::json!({ "role": "user", "content": user_msg }));
            msgs.push(serde_json::json!({ "role": "assistant", "content": asst_msg }));
        }

        msgs.push(serde_json::json!({ "role": "user", "content": question }));
    }

    // 8. Stream from llama-server, emitting tokens as they arrive
    let port = crate::llama::get_llama_port(&app)
        .ok_or("llama-server port unavailable")?;
    let api_url = format!("http://127.0.0.1:{}/v1/chat/completions", port);

    let payload = serde_json::json!({ "messages": msgs, "stream": true });

    let resp = match ureq::post(&api_url)
        .timeout(std::time::Duration::from_secs(180))
        .send_json(&payload)
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
