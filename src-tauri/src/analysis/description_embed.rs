use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Emitter;

pub fn run_description_embed_phase(app: &tauri::AppHandle, conn_arc: &Arc<Mutex<Connection>>) {
    let jobs: Vec<super::SpoolJob> = {
        let conn = match super::lock_analysis_conn(conn_arc, "description_embed") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "description_embed", e);
                return;
            }
        };
        let mut stmt = match conn.prepare(
            "SELECT tp.id, tp.track_id, t.path
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'description_embed'
             ORDER BY tp.id ASC",
        ) {
            Ok(s) => s,
            Err(e) => {
                super::emit_pipeline_error(
                    app,
                    "description_embed",
                    format!("Failed to query pending jobs: {}", e),
                );
                return;
            }
        };
        let rows: Vec<super::SpoolJob> = stmt
            .query_map([pass_status::PENDING], |row| {
                Ok(super::SpoolJob {
                    pass_id: row.get(0)?,
                    track_id: row.get(1)?,
                    path: row.get(2)?,
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

    if jobs.is_empty() {
        return;
    }

    log::info!(
        "[description_embed] loaded {} jobs, starting sentence embeddings",
        jobs.len()
    );

    for job in jobs {
        let start = std::time::Instant::now();

        let result = (|| -> Result<Option<Vec<f32>>, String> {
            // Retrieve description and other Qwen columns
            let track_data: (
                Option<i64>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
            ) = {
                let conn = super::lock_analysis_conn(conn_arc, "description_embed")?;
                conn.query_row(
                    "SELECT is_music, description, ai_genre, ai_mood, ai_instruments FROM tracks WHERE id = ?1",
                    [job.track_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
                ).map_err(|e| e.to_string())?
            };

            let is_music = track_data.0;
            let description = track_data.1;
            let ai_genre = track_data.2;
            let ai_mood = track_data.3;
            let ai_instruments = track_data.4;

            // If not music, skip entirely
            if let Some(0) = is_music {
                log::info!(
                    "[description_embed] Track {} marked as non-music. Skipping embedding.",
                    job.track_id
                );
                return Ok(None);
            }

            let desc = match description {
                Some(d) if !d.trim().is_empty() => d,
                _ => return Ok(None), // no description, mark done with no embedding
            };

            // Build concatenated text for richer semantic signal
            let mut embed_text = String::new();
            if let Some(g) = ai_genre {
                if !g.trim().is_empty() {
                    embed_text.push_str(&format!("Genre: {}. ", g));
                }
            }
            if let Some(m) = ai_mood {
                if !m.trim().is_empty() {
                    embed_text.push_str(&format!("Mood: {}. ", m));
                }
            }
            if let Some(i) = ai_instruments {
                if !i.trim().is_empty() {
                    embed_text.push_str(&format!("Instruments: {}. ", i));
                }
            }
            embed_text.push_str(&desc);

            let embedding = crate::embeddings::run_sentence_embed(&embed_text, Some(app))?;
            Ok(Some(embedding))
        })();

        let elapsed_ms = start.elapsed().as_millis() as i64;
        let conn = match super::lock_analysis_conn(conn_arc, "description_embed") {
            Ok(conn) => conn,
            Err(e) => {
                super::emit_pipeline_error(app, "description_embed", e);
                return;
            }
        };

        match result {
            Ok(emb_opt) => {
                if let Some(embedding) = emb_opt {
                    let blob: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO description_embeddings (track_id, embedding) VALUES (?1, ?2)",
                        rusqlite::params![job.track_id, blob],
                    );
                }

                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                     pass_version = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![
                        pass_status::DONE,
                        elapsed_ms,
                        pass_version::DESCRIPTION_EMBED,
                        job.pass_id
                    ],
                );

                // Save sidecar
                if let Err(e) = crate::scanner::sidecar::save(&conn, job.track_id) {
                    log::error!(
                        "[description_embed] Failed to save sidecar metadata for track {}: {}",
                        job.track_id,
                        e
                    );
                }

                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "description_embed",
                        "status": pass_status::DONE,
                    }),
                );
            }
            Err(e) => {
                log::error!("[description_embed] Track {} failed: {}", job.track_id, e);
                let _ = conn.execute(
                    "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3,
                     last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                    rusqlite::params![pass_status::FAILED, e, elapsed_ms, job.track_id],
                );
                let _ = app.emit(
                    "analysis-progress",
                    serde_json::json!({
                        "track_id": job.track_id,
                        "pass_name": "description_embed",
                        "status": pass_status::FAILED,
                    }),
                );
            }
        }
    }

    let _ = app.emit(
        "analysis-phase-complete",
        serde_json::json!({ "pass": "description_embed" }),
    );
}
