use std::sync::Mutex;
use rusqlite::Connection;
use crate::database::pass_status;
use crate::analysis;

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

    if !analysis::PipelineManager::is_running() {
        let _ = conn.execute(
            "UPDATE track_passes SET status = ?1, log = NULL, last_run_at = NULL
             WHERE status = ?2",
            rusqlite::params![pass_status::PENDING, pass_status::IN_PROGRESS],
        );
    }

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
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)))
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

    let error_rows: Vec<(String, String, Option<String>, Option<String>, Option<i64>)> = errors_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let stats = count_rows
        .into_iter()
        .map(|(pass_name, pending, in_progress, done, failed, total, avg_duration_ms)| {
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
            PassStats { pass_name, pending, in_progress, done, failed, total, avg_duration_ms, errors }
        })
        .collect();

    Ok(stats)
}

#[tauri::command]
pub fn reset_pass(
    pass_name: String,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE track_passes SET status = ?1, log = NULL, result = NULL,
         last_run_at = NULL, duration_ms = NULL WHERE pass_name = ?2",
        rusqlite::params![pass_status::PENDING, &pass_name],
    ).map_err(|e| e.to_string())?;
    if pass_name == "clap" {
        conn.execute("DELETE FROM audio_embeddings", [])
            .map_err(|e| e.to_string())?;
    }
    if pass_name == "essentia" {
        conn.execute(
            "UPDATE tracks SET
                detected_genre = NULL, detected_vocal = NULL, detected_vocal_confidence = NULL,
                mood_happy = NULL, mood_sad = NULL, mood_aggressive = NULL,
                mood_relaxed = NULL, mood_party = NULL, mood_acoustic = NULL,
                mood_electronic = NULL",
            [],
        ).map_err(|e| e.to_string())?;
    }
    // bpm_correction and bpm_refinement: restore bpm from bpm_raw so re-running is idempotent
    if pass_name == "bpm_correction" || pass_name == "bpm_refinement" {
        conn.execute(
            "UPDATE tracks SET bpm = bpm_raw WHERE bpm_raw IS NOT NULL",
            [],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_all_passes(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE track_passes SET status = ?1, log = NULL, result = NULL,
         last_run_at = NULL, duration_ms = NULL",
        [pass_status::PENDING],
    ).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM audio_embeddings", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
