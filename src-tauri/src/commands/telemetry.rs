use crate::metrics_database::MetricsState;
use crate::error::AppError;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct LatencyStat {
    pub pass_name: String,
    pub avg_duration_ms: f64,
    pub min_duration_ms: i64,
    pub max_duration_ms: i64,
    pub count: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PipelineMetricRow {
    pub id: i64,
    pub run_id: String,
    pub track_id: i64,
    pub pass_name: String,
    pub status: String,
    pub duration_ms: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub audio_duration_sec: Option<f64>,
    pub error_message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SystemEventRow {
    pub id: i64,
    pub event_type: String,
    pub details: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TelemetrySummary {
    pub latencies: Vec<LatencyStat>,
    pub recent_failures: Vec<PipelineMetricRow>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RawTelemetryPayload {
    pub pipeline_metrics: Vec<PipelineMetricRow>,
    pub system_events: Vec<SystemEventRow>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AggregatedPassSpan {
    pub run_id: String,
    pub pass_name: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub total: i64,
    pub succeeded: i64,
    pub failed: i64,
}

#[tauri::command]
pub fn get_telemetry_summary(
    state: tauri::State<'_, MetricsState>,
) -> Result<TelemetrySummary, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Generic("Metrics lock poisoned".to_string()))?;

    // 1. Get average latencies
    let mut stmt = conn.prepare(
        "SELECT pass_name, AVG(duration_ms) as avg_dur, MIN(duration_ms) as min_dur, MAX(duration_ms) as max_dur, COUNT(*) as cnt
         FROM pipeline_metrics
         GROUP BY pass_name
         ORDER BY avg_dur DESC"
    ).map_err(AppError::Database)?;

    let latencies: Vec<LatencyStat> = stmt.query_map([], |row| {
        Ok(LatencyStat {
            pass_name: row.get(0)?,
            avg_duration_ms: row.get(1)?,
            min_duration_ms: row.get(2)?,
            max_duration_ms: row.get(3)?,
            count: row.get(4)?,
        })
    }).map_err(AppError::Database)?
    .filter_map(|r| r.ok())
    .collect();

    // 2. Get recent failures
    let mut stmt = conn.prepare(
        "SELECT id, run_id, track_id, pass_name, status, duration_ms, started_at, ended_at, audio_duration_sec, error_message
         FROM pipeline_metrics
         WHERE status = 'failed'
         ORDER BY id DESC
         LIMIT 50"
    ).map_err(AppError::Database)?;

    let recent_failures: Vec<PipelineMetricRow> = stmt.query_map([], |row| {
        Ok(PipelineMetricRow {
            id: row.get(0)?,
            run_id: row.get(1)?,
            track_id: row.get(2)?,
            pass_name: row.get(3)?,
            status: row.get(4)?,
            duration_ms: row.get(5)?,
            started_at: row.get(6)?,
            ended_at: row.get(7)?,
            audio_duration_sec: row.get(8)?,
            error_message: row.get(9)?,
        })
    }).map_err(AppError::Database)?
    .filter_map(|r| r.ok())
    .collect();

    Ok(TelemetrySummary {
        latencies,
        recent_failures,
    })
}

#[tauri::command]
pub fn get_raw_telemetry_payload(
    state: tauri::State<'_, MetricsState>,
) -> Result<RawTelemetryPayload, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Generic("Metrics lock poisoned".to_string()))?;

    // 1. Get all pipeline metrics
    let mut stmt = conn.prepare(
        "SELECT id, run_id, track_id, pass_name, status, duration_ms, started_at, ended_at, audio_duration_sec, error_message
         FROM pipeline_metrics
         ORDER BY started_at ASC"
    ).map_err(AppError::Database)?;

    let pipeline_metrics: Vec<PipelineMetricRow> = stmt.query_map([], |row| {
        Ok(PipelineMetricRow {
            id: row.get(0)?,
            run_id: row.get(1)?,
            track_id: row.get(2)?,
            pass_name: row.get(3)?,
            status: row.get(4)?,
            duration_ms: row.get(5)?,
            started_at: row.get(6)?,
            ended_at: row.get(7)?,
            audio_duration_sec: row.get(8)?,
            error_message: row.get(9)?,
        })
    }).map_err(AppError::Database)?
    .filter_map(|r| r.ok())
    .collect();

    // 2. Get all system events
    let mut stmt = conn.prepare(
        "SELECT id, event_type, details, duration_ms, created_at
         FROM system_events
         ORDER BY id ASC"
    ).map_err(AppError::Database)?;

    let system_events: Vec<SystemEventRow> = stmt.query_map([], |row| {
        Ok(SystemEventRow {
            id: row.get(0)?,
            event_type: row.get(1)?,
            details: row.get(2)?,
            duration_ms: row.get(3)?,
            created_at: row.get(4)?,
        })
    }).map_err(AppError::Database)?
    .filter_map(|r| r.ok())
    .collect();

    Ok(RawTelemetryPayload {
        pipeline_metrics,
        system_events,
    })
}

#[tauri::command]
pub fn get_pipeline_run_traces(
    state: tauri::State<'_, MetricsState>,
) -> Result<Vec<AggregatedPassSpan>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Generic("Metrics lock poisoned".to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT run_id, pass_name,
                MIN(started_at) as started_at,
                MAX(ended_at)   as ended_at,
                COUNT(*)        as total,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as succeeded,
                SUM(CASE WHEN status = 'failed'  THEN 1 ELSE 0 END) as failed
         FROM pipeline_metrics
         GROUP BY run_id, pass_name
         ORDER BY run_id, started_at"
    ).map_err(AppError::Database)?;

    let spans = stmt.query_map([], |row| {
        Ok(AggregatedPassSpan {
            run_id: row.get(0)?,
            pass_name: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
            total: row.get(4)?,
            succeeded: row.get(5)?,
            failed: row.get(6)?,
        })
    }).map_err(AppError::Database)?
    .filter_map(|r| r.ok())
    .collect();

    Ok(spans)
}

