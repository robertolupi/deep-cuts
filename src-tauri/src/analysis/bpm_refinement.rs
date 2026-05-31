use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

struct BpmJob {
    pass_id: i64,
    track_id: i64,
    bpm_raw: Option<f64>,
    detected_genre: Option<String>,
}

pub fn run_bpm_refinement_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<BpmJob> = {
        let conn = match super::lock_analysis_conn(conn_arc, "bpm_refinement") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "bpm_refinement", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.bpm_raw, t.detected_genre
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'bpm_refinement'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                super::emit_pipeline_error(
                    app,
                    "bpm_refinement",
                    format!("Failed to prepare pending jobs query: {}", e),
                );
                return;
            }
        };
        let rows: Vec<BpmJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(BpmJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    bpm_raw: row.get(2)?,
                    detected_genre: row.get(3)?,
                })
            })
            .map(|r| r.filter_map(|x| x.ok()).collect())
            .unwrap_or_default();
        for job in &rows {
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, last_run_at = CURRENT_TIMESTAMP WHERE id = ?2",
                rusqlite::params![pass_status::IN_PROGRESS, job.pass_id],
            );
        }
        rows
    };

    let start_phase = std::time::Instant::now();
    let mut corrected = 0usize;
    let mut nulled = 0usize;

    log::info!(
        "[bpm_refinement] loaded {} jobs, computing corrections",
        jobs.len()
    );

    // Compute all corrections first (pure CPU, no lock needed)
    // bpm_refinement always re-corrects from bpm_raw so the two passes are independent
    let corrections: Vec<crate::bpm::CorrectResult> = jobs
        .iter()
        .map(|job| crate::bpm::correct_bpm(job.bpm_raw, job.detected_genre.as_deref()))
        .collect();

    log::info!("[bpm_refinement] corrections computed, acquiring DB lock for transaction");
    {
        let conn = match super::lock_analysis_conn(conn_arc, "bpm_refinement") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "bpm_refinement", e);
                return;
            }
        };
        log::debug!("[bpm_refinement] lock acquired, beginning transaction");
        let begin_result = conn.execute("BEGIN", []);
        log::debug!("[bpm_refinement] BEGIN result: {:?}", begin_result);
        for (job, result) in jobs.iter().zip(corrections.iter()) {
            match result {
                crate::bpm::CorrectResult::Corrected(new_bpm) => {
                    corrected += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = ?1 WHERE id = ?2",
                        rusqlite::params![new_bpm, job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Null => {
                    nulled += 1;
                    let _ = conn.execute(
                        "UPDATE tracks SET bpm = NULL WHERE id = ?1",
                        rusqlite::params![job.track_id],
                    );
                }
                crate::bpm::CorrectResult::Unchanged => {}
            }
            let _ = conn.execute(
                "UPDATE track_passes SET status = ?1, duration_ms = 0,
                 pass_version = ?2, last_run_at = CURRENT_TIMESTAMP WHERE id = ?3",
                rusqlite::params![pass_status::DONE, pass_version::BPM_REFINEMENT, job.pass_id],
            );
        }
        let commit_result = conn.execute("COMMIT", []);
        log::info!("[bpm_refinement] COMMIT result: {:?}", commit_result);
    } // lock released before any emit

    log::info!(
        "[bpm_refinement] {} tracks: {} corrected, {} nulled in {:.1}s",
        jobs.len(),
        corrected,
        nulled,
        start_phase.elapsed().as_secs_f32()
    );
    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({
            "pass": "bpm_refinement", "corrected": corrected, "nulled": nulled,
        }),
    );
}
