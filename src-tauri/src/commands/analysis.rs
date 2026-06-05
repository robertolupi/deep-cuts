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
    pub concurrency: f64,
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
    metrics_state: tauri::State<'_, crate::metrics_database::MetricsState>,
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

    // Query historical averages from metrics database
    let historical_averages = {
        if let Ok(metrics_conn) = metrics_state.0.lock() {
            let stmt = metrics_conn
                .prepare(
                    "SELECT pass_name, AVG(duration_ms)
                     FROM pipeline_metrics
                     WHERE status = 'success'
                     GROUP BY pass_name"
                )
                .ok();
            if let Some(mut stmt) = stmt {
                stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                })
                .ok()
                .map(|rows| {
                    rows.filter_map(|r| r.ok())
                        .collect::<std::collections::HashMap<String, f64>>()
                })
                .unwrap_or_default()
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        }
    };

    let decode_threads = crate::hardware::PipelineConfig::auto_tune().decode_threads as f64;

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

                let final_avg = avg_duration_ms
                    .or_else(|| historical_averages.get(&pass_name).copied())
                    .or_else(|| {
                        // Hardcoded fallback default durations (in ms) if no database history exists
                        match pass_name.as_str() {
                            "audio_analysis" => Some(400.0),
                            "bpm_correction" => Some(5.0),
                            "clap" => Some(600.0),
                            "essentia" => Some(500.0),
                            "bpm_refinement" => Some(150.0),
                            "qwen" => Some(3000.0),
                            "description_embed" => Some(20.0),
                            _ => None,
                        }
                    });

                let concurrency = match pass_name.as_str() {
                    "audio_analysis" => decode_threads,
                    _ => 1.0,
                };

                PassStats {
                    pass_name,
                    pending,
                    in_progress,
                    done,
                    failed,
                    total,
                    avg_duration_ms: final_avg,
                    concurrency,
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
pub fn reset_pass_for_track(
    pass_name: String,
    track_id: i64,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    analysis::reset_pass_for_track(&conn, &pass_name, track_id)
}

#[tauri::command]
pub fn reset_all_passes(conn_state: tauri::State<'_, Mutex<Connection>>) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    analysis::reset_all_passes(&conn)
}

#[tauri::command]
pub fn check_models_exist(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    use tauri::Manager;

    // Load the live manifest from DB cache, falling back to the compiled-in one.
    let manifest = app
        .try_state::<std::sync::Mutex<rusqlite::Connection>>()
        .and_then(|state| {
            state.lock().ok().and_then(|conn| {
                conn.query_row(
                    "SELECT value FROM app_settings WHERE key = 'manifest_cached_json'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .ok()
            })
        })
        .and_then(|json| crate::models::ModelManifest::parse(&json).ok())
        .unwrap_or_else(crate::models::ModelManifest::fallback);

    // For each group, check every file listed in the manifest.
    let mut group_status: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    let mut missing_files: Vec<String> = Vec::new();

    for (group_key, group) in &manifest.models {
        let mut all_present = true;
        for file in &group.files {
            let path = crate::embeddings::get_model_path(&file.filename, Some(&app));
            if !path.exists() {
                missing_files.push(format!("{}/{}", group_key, file.filename));
                all_present = false;
            }
        }
        group_status.insert(group_key.clone(), all_present);
    }

    let all_exist = group_status.values().all(|&v| v);

    // Build the response, keeping backward-compatible per-group keys.
    let mut result = serde_json::json!({
        "all_exist": all_exist,
        "missing_files": missing_files,
    });

    for (key, exists) in &group_status {
        result[format!("{}_exists", key)] = serde_json::Value::Bool(*exists);
    }

    Ok(result)
}

#[tauri::command]
pub fn set_analysis_manually_paused(app: tauri::AppHandle, paused: bool) {
    analysis::ANALYSIS_MANUALLY_PAUSED.store(paused, std::sync::atomic::Ordering::SeqCst);
    analysis::emit_paused_status(&app);
}

#[tauri::command]
pub fn set_analysis_auto_paused(app: tauri::AppHandle, paused: bool) {
    analysis::ANALYSIS_AUTO_PAUSED.store(paused, std::sync::atomic::Ordering::SeqCst);
    analysis::emit_paused_status(&app);
}

#[tauri::command]
pub fn get_analysis_paused_status() -> serde_json::Value {
    serde_json::json!({
        "manually_paused": analysis::ANALYSIS_MANUALLY_PAUSED.load(std::sync::atomic::Ordering::SeqCst),
        "auto_paused": analysis::ANALYSIS_AUTO_PAUSED.load(std::sync::atomic::Ordering::SeqCst),
    })
}
