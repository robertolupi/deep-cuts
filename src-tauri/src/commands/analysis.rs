use crate::analysis;
use crate::database::pass_status;
use rusqlite::Connection;
use std::sync::Mutex;

#[derive(serde::Serialize)]
pub struct PassError {
    pub path: String,
    pub log: Option<String>,
    pub duration_ms: Option<i64>,
    pub last_run_at: Option<String>,
}

#[derive(serde::Serialize)]
pub struct PassStats {
    pub pass_name: String,
    pub pending: i64,
    pub in_progress: i64,
    pub done: i64,
    pub failed: i64,
    pub total: i64,
    pub avg_duration_ms: Option<f64>,
    pub errors: Vec<PassError>,
}

#[tauri::command]
pub fn run_analysis_pipeline(
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    analysis::PipelineManager::run(app, &conn_state)
}

#[tauri::command]
pub fn is_analysis_running() -> bool {
    analysis::PipelineManager::is_running()
}

#[tauri::command]
pub fn get_pass_stats(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<PassStats>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;

    let mut counts_stmt = conn
        .prepare(
            "SELECT pass_name,
                    SUM(CASE WHEN status = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 1 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 2 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status = 3 THEN 1 ELSE 0 END),
                    COUNT(*),
                    AVG(CASE WHEN status = 2 THEN duration_ms ELSE NULL END)
             FROM track_passes
             GROUP BY pass_name
             ORDER BY pass_name",
        )
        .map_err(|e| e.to_string())?;

    let count_rows: Vec<(String, i64, i64, i64, i64, i64, Option<f64>)> = counts_stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut errors_stmt = conn
        .prepare(
            "SELECT tp.pass_name, t.path, tp.log, tp.last_run_at, tp.duration_ms
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = 3
             ORDER BY tp.pass_name, tp.last_run_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let error_rows: Vec<(String, String, Option<String>, Option<String>, Option<i64>)> =
        errors_stmt
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

    let stats = count_rows
        .into_iter()
        .map(
            |(pass_name, pending, in_progress, done, failed, total, avg_duration_ms)| {
                let errors = error_rows
                    .iter()
                    .filter(|(p, _, _, _, _)| p == &pass_name)
                    .map(|(_, path, log, last_run_at, duration_ms)| PassError {
                        path: path.clone(),
                        log: log.clone(),
                        duration_ms: *duration_ms,
                        last_run_at: last_run_at.clone(),
                    })
                    .collect();
                PassStats {
                    pass_name,
                    pending,
                    in_progress,
                    done,
                    failed,
                    total,
                    avg_duration_ms,
                    errors,
                }
            },
        )
        .collect();

    Ok(stats)
}

#[tauri::command]
pub fn recover_stuck_passes(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<usize, String> {
    if analysis::PipelineManager::is_running() {
        return Err("Cannot recover stuck passes while analysis is running.".to_string());
    }

    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            "UPDATE track_passes SET status = ?1,
             log = 'Recovered after interrupted analysis run',
             last_run_at = NULL
         WHERE status = ?2",
            rusqlite::params![pass_status::PENDING, pass_status::IN_PROGRESS],
        )
        .map_err(|e| e.to_string())?;
    Ok(changed)
}

#[tauri::command]
pub fn reset_pass(
    pass_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    analysis::reset_pass(&conn, &pass_name)
}

#[tauri::command]
pub fn reset_all_passes(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    analysis::reset_all_passes(&conn)
}

#[tauri::command]
pub fn check_models_exist(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let qwen_model =
        crate::embeddings::get_model_path("Qwen2-Audio-7B-Instruct.Q4_K_M.gguf", Some(&app));
    let qwen_mmproj =
        crate::embeddings::get_model_path("Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf", Some(&app));
    let sentence_model = crate::embeddings::get_model_path("all-minilm-l6-v2.onnx", Some(&app));
    let sentence_tok =
        crate::embeddings::get_model_path("all-minilm-l6-v2-tokenizer.json", Some(&app));
    let clap_model = crate::embeddings::get_model_path("clap_audio_encoder.onnx", Some(&app));
    let clap_mel = crate::embeddings::get_model_path("clap_mel_weights.bin", Some(&app));

    // Essentia models
    let essentia_base =
        crate::embeddings::get_model_path("discogs-effnet-bsdynamic-1.onnx", Some(&app));
    let essentia_base_json =
        crate::embeddings::get_model_path("discogs-effnet-bsdynamic-1.json", Some(&app));

    // Check all head files
    let heads = [
        "genre_discogs400-discogs-effnet-1",
        "mood_happy-discogs-effnet-1",
        "mood_sad-discogs-effnet-1",
        "mood_aggressive-discogs-effnet-1",
        "mood_relaxed-discogs-effnet-1",
        "mood_party-discogs-effnet-1",
        "mood_acoustic-discogs-effnet-1",
        "mood_electronic-discogs-effnet-1",
        "voice_instrumental-discogs-effnet-1",
    ];

    let mut essentia_heads_exist = true;
    for head in &heads {
        let onnx_path = crate::embeddings::get_model_path(&format!("{}.onnx", head), Some(&app));
        let json_path = crate::embeddings::get_model_path(&format!("{}.json", head), Some(&app));
        if !onnx_path.exists() || !json_path.exists() {
            essentia_heads_exist = false;
        }
    }

    let qwen_exists = qwen_model.exists() && qwen_mmproj.exists();
    let sentence_exists = sentence_model.exists() && sentence_tok.exists();
    let clap_exists = clap_model.exists() && clap_mel.exists();
    let essentia_exists =
        essentia_base.exists() && essentia_base_json.exists() && essentia_heads_exist;

    let all_exist = qwen_exists && sentence_exists && clap_exists && essentia_exists;

    Ok(serde_json::json!({
        "qwen_model": qwen_model.exists(),
        "qwen_mmproj": qwen_mmproj.exists(),
        "sentence_model": sentence_model.exists(),
        "sentence_tok": sentence_tok.exists(),
        "clap_model": clap_model.exists(),
        "clap_mel": clap_mel.exists(),
        "essentia_base": essentia_base.exists(),
        "essentia_base_json": essentia_base_json.exists(),
        "essentia_heads": essentia_heads_exist,
        "qwen_exists": qwen_exists,
        "sentence_exists": sentence_exists,
        "clap_exists": clap_exists,
        "essentia_exists": essentia_exists,
        "all_exist": all_exist,
    }))
}
