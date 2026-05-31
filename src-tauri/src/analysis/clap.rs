use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use crate::embeddings;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub struct ClapJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub path: String,
    pub duration_seconds: i64,
    pub waveform_data: Option<String>,
}

impl super::PassJob for ClapJob {
    fn pass_id(&self) -> i64 {
        self.pass_id
    }
    fn track_id(&self) -> i64 {
        self.track_id
    }
}

pub struct ClapPass;

struct PreppedSpectrogram {
    pass_id: i64,
    track_id: i64,
    result: Result<[Vec<f32>; 3], String>,
    elapsed_ms: i64,
}

impl super::AnalysisPass for ClapPass {
    type Job = ClapJob;
    type Output = Vec<f32>;

    fn name(&self) -> &'static str {
        "clap"
    }

    fn priority(&self) -> i32 {
        20
    }

    fn version(&self) -> u32 {
        pass_version::CLAP
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["audio_analysis"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &[]
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &["audio_embeddings", "track_coords"]
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.path, t.duration_seconds, t.waveform_data
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'clap'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(ClapJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                path: row.get(2)?,
                duration_seconds: row.get(3)?,
                waveform_data: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, _job: &Self::Job) -> Result<Self::Output, String> {
        // Not called directly as we override run_pass for parallel decoding
        Err("Use run_pass for parallel execution".to_string())
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        let blob: Vec<u8> = output.iter().flat_map(|&f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT OR REPLACE INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![job.track_id, blob],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn run_pass(
        &self,
        app: &tauri::AppHandle,
        conn_arc: &Arc<Mutex<Connection>>,
    ) -> Result<(), String> {
        let config = crate::hardware::PipelineConfig::auto_tune();

        if let Err(e) =
            embeddings::configure_session(config.use_coreml, config.intra_threads, Some(app))
        {
            return Err(format!("Failed to configure ONNX session: {}", e));
        }

        let clap_pending = {
            let conn = super::lock_analysis_conn(conn_arc, self.name())?;
            let rows = self.load_jobs(&conn)?;
            for job in &rows {
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                    rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
                );
            }
            rows
        };

        if clap_pending.is_empty() {
            return Ok(());
        }

        let (tx, rx) =
            std::sync::mpsc::sync_channel::<PreppedSpectrogram>(config.decode_threads * 2);
        let clap_jobs_queue = Arc::new(Mutex::new(VecDeque::from(clap_pending)));

        let mut prep_workers = Vec::new();
        for _ in 0..config.decode_threads {
            let queue_clone = Arc::clone(&clap_jobs_queue);
            let tx_clone = tx.clone();
            let app_clone = app.clone();

            prep_workers.push(std::thread::spawn(move || loop {
                let job = {
                    match queue_clone.lock() {
                        Ok(mut q) => q.pop_front(),
                        Err(e) => {
                            log::error!("[clap] queue lock poisoned: {}", e);
                            break;
                        }
                    }
                };
                let job = match job {
                    Some(j) => j,
                    None => break,
                };

                let start = std::time::Instant::now();
                let result = (|| -> Result<[Vec<f32>; 3], String> {
                    let window_pcts = embeddings::select_clap_window_pcts(
                        job.waveform_data.as_deref(),
                        job.duration_seconds,
                    );
                    Ok([
                        embeddings::preprocess_window_at_pct(
                            &job.path,
                            window_pcts[0],
                            Some(&app_clone),
                        )?,
                        embeddings::preprocess_window_at_pct(
                            &job.path,
                            window_pcts[1],
                            Some(&app_clone),
                        )?,
                        embeddings::preprocess_window_at_pct(
                            &job.path,
                            window_pcts[2],
                            Some(&app_clone),
                        )?,
                    ])
                })();
                let elapsed_ms = start.elapsed().as_millis() as i64;

                match result {
                    Ok(mel_windows) => {
                        let _ = tx_clone.send(PreppedSpectrogram {
                            pass_id: job.pass_id,
                            track_id: job.track_id,
                            result: Ok(mel_windows),
                            elapsed_ms,
                        });
                    }
                    Err(e) => {
                        log::error!(
                            "[clap] Preprocessing failed for track {}: {}",
                            job.track_id,
                            e
                        );
                        let _ = tx_clone.send(PreppedSpectrogram {
                            pass_id: job.pass_id,
                            track_id: job.track_id,
                            result: Err(e),
                            elapsed_ms,
                        });
                    }
                }
            }));
        }
        drop(tx);

        for prepped in rx {
            let (result, elapsed_ms) = match prepped.result {
                Ok(mel_windows) => {
                    let start = std::time::Instant::now();
                    let result = embeddings::run_clap_inference_pooled(mel_windows);
                    (result, start.elapsed().as_millis() as i64)
                }
                Err(e) => (
                    Err(format!("Preprocessing failed: {}", e)),
                    prepped.elapsed_ms,
                ),
            };

            let conn = super::lock_analysis_conn(conn_arc, self.name())?;
            match result {
                Ok(embedding) => {
                    let job_placeholder = ClapJob {
                        pass_id: prepped.pass_id,
                        track_id: prepped.track_id,
                        path: String::new(),
                        duration_seconds: 0,
                        waveform_data: None,
                    };
                    self.save_result(&conn, &job_placeholder, embedding, elapsed_ms)?;
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                         pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                        rusqlite::params![
                            pass_status::DONE,
                            elapsed_ms,
                            pass_version::CLAP,
                            prepped.pass_id
                        ],
                    );
                    let _ = app.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "track_id": prepped.track_id,
                            "pass_name": self.name(),
                            "status": pass_status::DONE,
                        }),
                    );
                }
                Err(e) => {
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                         last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                        rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                    );
                    let _ = app.emit(
                        "analysis-progress",
                        serde_json::json!({
                            "track_id": prepped.track_id,
                            "pass_name": self.name(),
                            "status": pass_status::FAILED,
                        }),
                    );
                }
            }
        }

        for h in prep_workers {
            let _ = h.join();
        }

        let _ = app.emit(
            "analysis-phase-complete",
            serde_json::json!({ "pass": self.name() }),
        );
        Ok(())
    }
}

impl ClapPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "clap",
        priority: 20,
        version: pass_version::CLAP,
        dependencies: &["audio_analysis"],
        owned_columns: &[],
        owned_tables: &["audio_embeddings", "track_coords"],
        custom_reset: None,
    };
}
