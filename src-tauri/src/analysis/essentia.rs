use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use crate::dsp;
use rusqlite::Connection;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub struct EssentiaJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub path: String,
}

impl super::PassJob for EssentiaJob {
    fn pass_id(&self) -> i64 {
        self.pass_id
    }
    fn track_id(&self) -> i64 {
        self.track_id
    }
}

pub struct EssentiaPass;

struct PreppedPatches {
    pass_id: i64,
    track_id: i64,
    patches: Vec<Vec<f32>>,
    patch_count: usize,
}

impl super::AnalysisPass for EssentiaPass {
    type Job = EssentiaJob;
    type Output = crate::classifier::ClassifierResult;

    fn name(&self) -> &'static str {
        "essentia"
    }

    fn priority(&self) -> i32 {
        50
    }

    fn version(&self) -> u32 {
        pass_version::ESSENTIA
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["audio_analysis"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &[
            "detected_genre", "detected_vocal", "detected_vocal_confidence",
            "mood_happy", "mood_sad", "mood_aggressive", "mood_relaxed",
            "mood_party", "mood_acoustic", "mood_electronic"
        ]
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &[]
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'essentia'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(EssentiaJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                path: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, _job: &Self::Job) -> Result<Self::Output, String> {
        Err("Use run_pass for parallel execution".to_string())
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE tracks SET
                detected_genre             = ?1,
                detected_vocal             = ?2,
                detected_vocal_confidence  = ?3,
                mood_happy                 = ?4,
                mood_sad                   = ?5,
                mood_aggressive            = ?6,
                mood_relaxed               = ?7,
                mood_party                 = ?8,
                mood_acoustic              = ?9,
                mood_electronic            = ?10
             WHERE id = ?11",
            rusqlite::params![
                output.genre,
                output.vocal,
                output.vocal_confidence,
                output.mood_happy,
                output.mood_sad,
                output.mood_aggressive,
                output.mood_relaxed,
                output.mood_party,
                output.mood_acoustic,
                output.mood_electronic,
                job.track_id,
            ],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn run_pass(
        &self,
        app: &tauri::AppHandle,
        conn_arc: &Arc<Mutex<Connection>>,
    ) -> Result<(), String> {
        let config = crate::hardware::PipelineConfig::auto_tune();

        let jobs = {
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

        if jobs.is_empty() {
            return Ok(());
        }

        log::info!(
            "[essentia] {} jobs, {} decode workers",
            jobs.len(),
            config.decode_threads
        );

        let (tx, rx) = std::sync::mpsc::sync_channel::<PreppedPatches>(config.decode_threads * 2);
        let queue = Arc::new(Mutex::new(VecDeque::from(jobs)));

        let mut prep_handles = Vec::new();
        for _ in 0..config.decode_threads {
            let queue_clone = Arc::clone(&queue);
            let tx_clone = tx.clone();

            prep_handles.push(std::thread::spawn(move || {
                loop {
                    let job = {
                        match queue_clone.lock() {
                            Ok(mut q) => q.pop_front(),
                            Err(e) => {
                                log::error!("[essentia] queue lock poisoned: {}", e);
                                break;
                            }
                        }
                    };
                    let job = match job {
                        Some(j) => j,
                        None => break,
                    };

                    let result = (|| -> Result<Vec<Vec<f32>>, String> {
                        let (audio, sr) = dsp::decode_audio_to_mono(&job.path)?;
                        let audio_16k = crate::spectrogram::resample_to_16k(&audio, sr)?;
                        let mid = audio_16k.len() / 2;
                        let half = 16_000 * 30;
                        let start = mid.saturating_sub(half);
                        let end = (mid + half).min(audio_16k.len());
                        let spec =
                            crate::spectrogram::compute_log_mel_spectrogram(&audio_16k[start..end])?;
                        crate::spectrogram::extract_patches(&spec)
                    })();

                    match result {
                        Ok(patches) => {
                            let patch_count = patches.len();
                            let _ = tx_clone.send(PreppedPatches {
                                pass_id: job.pass_id,
                                track_id: job.track_id,
                                patches,
                                patch_count,
                            });
                        }
                        Err(e) => {
                            log::error!(
                                "[essentia] Preprocessing failed for track {}: {}",
                                job.track_id,
                                e
                            );
                            let _ = tx_clone.send(PreppedPatches {
                                pass_id: job.pass_id,
                                track_id: job.track_id,
                                patches: vec![],
                                patch_count: 0,
                            });
                        }
                    }
                }
            }));
        }
        drop(tx);

        for prepped in rx {
            let start = std::time::Instant::now();

            let result = if prepped.patches.is_empty() {
                Err("Preprocessing failed".to_string())
            } else {
                crate::classifier::run_classifier_inference(&prepped.patches, Some(app))
            };

            let elapsed_ms = start.elapsed().as_millis() as i64;
            let conn = super::lock_analysis_conn(conn_arc, self.name())?;

            match result {
                Ok(r) => {
                    let raw_result = serde_json::json!({
                        "genre": r.genre,
                        "genre_top3": r.genre_top3,
                        "vocal": r.vocal,
                        "vocal_confidence": r.vocal_confidence,
                        "patch_count": prepped.patch_count,
                    }).to_string();
                    let job_placeholder = EssentiaJob {
                        pass_id: prepped.pass_id,
                        track_id: prepped.track_id,
                        path: String::new(),
                    };
                    self.save_result(&conn, &job_placeholder, r, elapsed_ms)?;
                    let _ = conn.execute(
                        "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                         pass_version = ?3, raw_result = ?4, last_run_at = CURRENT_TIMESTAMP WHERE id = ?5",
                        rusqlite::params![
                            pass_status::DONE,
                            elapsed_ms,
                            pass_version::ESSENTIA,
                            raw_result,
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
                    log::error!("[essentia] Track {} failed: {}", prepped.track_id, e);
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

        for h in prep_handles {
            let _ = h.join();
        }

        let _ = app.emit(
            "analysis-phase-complete",
            serde_json::json!({ "pass": self.name() }),
        );
        Ok(())
    }
}

impl EssentiaPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "essentia",
        priority: 50,
        version: pass_version::ESSENTIA,
        dependencies: &["audio_analysis"],
        owned_columns: &[
            "detected_genre", "detected_vocal", "detected_vocal_confidence",
            "mood_happy", "mood_sad", "mood_aggressive", "mood_relaxed",
            "mood_party", "mood_acoustic", "mood_electronic"
        ],
        owned_tables: &[],
        custom_reset: None,
    };
}
