use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

pub struct MetricsDbManager {
    db_path: PathBuf,
}

impl MetricsDbManager {
    pub fn new(app_handle: &AppHandle) -> Self {
        let log_dir = app_handle
            .path()
            .app_log_dir()
            .expect("Failed to resolve standard OS App Log Directory");

        if !log_dir.exists() {
            fs::create_dir_all(&log_dir).expect("Failed to create App Log Directory");
        }

        let db_path = log_dir.join("metrics.db");
        MetricsDbManager { db_path }
    }

    pub fn connect_and_migrate(&self) -> Result<Connection, Box<dyn std::error::Error>> {
        let mut conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL;")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let migrations = get_migrations();
        migrations.to_latest(&mut conn)?;

        Ok(conn)
    }
}

pub fn get_migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(
            "CREATE TABLE pipeline_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id TEXT NOT NULL,
                track_id INTEGER NOT NULL,
                pass_name TEXT NOT NULL,
                status TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                started_at INTEGER NOT NULL, -- Unix timestamp in ms
                ended_at INTEGER NOT NULL,   -- Unix timestamp in ms
                audio_duration_sec REAL,
                error_message TEXT
            );
            CREATE TABLE system_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                details TEXT,
                duration_ms INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );"
        ),
    ])
}

pub struct MetricsState(pub Mutex<Connection>);

pub fn log_pipeline_metric<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    run_id: &str,
    track_id: i64,
    pass_name: &str,
    status: &str,
    duration_ms: i64,
    started_at: i64,
    ended_at: i64,
    audio_duration_sec: Option<f64>,
    error_message: Option<&str>,
) {
    if let Some(state) = app.try_state::<MetricsState>() {
        if let Ok(conn) = state.0.lock() {
            let _ = conn.execute(
                "INSERT INTO pipeline_metrics (run_id, track_id, pass_name, status, duration_ms, started_at, ended_at, audio_duration_sec, error_message)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    run_id,
                    track_id,
                    pass_name,
                    status,
                    duration_ms,
                    started_at,
                    ended_at,
                    audio_duration_sec,
                    error_message
                ],
            );
        }
    }
}

pub fn log_system_event<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    event_type: &str,
    details: Option<&str>,
    duration_ms: Option<i64>,
) {
    if let Some(state) = app.try_state::<MetricsState>() {
        if let Ok(conn) = state.0.lock() {
            let _ = conn.execute(
                "INSERT INTO system_events (event_type, details, duration_ms)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![event_type, details, duration_ms],
            );
        }
    }
}
