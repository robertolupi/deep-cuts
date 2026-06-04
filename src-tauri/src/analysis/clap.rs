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
    window_pcts: [f64; 3],
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
            "DELETE FROM audio_embeddings WHERE track_id = ?1",
            rusqlite::params![job.track_id],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO audio_embeddings (track_id, embedding) VALUES (?1, ?2)",
            rusqlite::params![job.track_id, blob],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn run_pass(
        &self,
        app: &tauri::AppHandle,
        conn_arc: &Arc<Mutex<Connection>>,
        run_id: &str,
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
                let window_pcts = embeddings::select_clap_window_pcts(
                    job.waveform_data.as_deref(),
                    job.duration_seconds,
                );
                let result = (|| -> Result<[Vec<f32>; 3], String> {
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
                            window_pcts,
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
                            window_pcts,
                        });
                    }
                }
            }));
        }
        drop(tx);

        for prepped in rx {
            let start_time_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

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
            let ended_time_ms = start_time_ms + elapsed_ms;

            // Fetch audio duration:
            let audio_dur = {
                if let Ok(c) = conn_arc.lock() {
                    c.query_row(
                        "SELECT duration_seconds FROM tracks WHERE id = ?1",
                        rusqlite::params![prepped.track_id],
                        |row| row.get::<_, Option<f64>>(0)
                    ).unwrap_or(None)
                } else {
                    None
                }
            };

            let conn = super::lock_analysis_conn(conn_arc, self.name())?;
            match result {
                Ok(embedding) => {
                    let norm: f64 = {
                        let sq: f32 = embedding.iter().map(|x| x * x).sum();
                        ((sq.sqrt() as f64) * 10_000.0).round() / 10_000.0
                    };
                    let raw_result = serde_json::json!({
                        "windows_pct": prepped.window_pcts,
                        "embedding_norm": norm,
                        "embedding_dim": embedding.len(),
                    }).to_string();
                    let job_placeholder = ClapJob {
                        pass_id: prepped.pass_id,
                        track_id: prepped.track_id,
                        path: String::new(),
                        duration_seconds: 0,
                        waveform_data: None,
                    };
                    match self.save_result(&conn, &job_placeholder, embedding, elapsed_ms) {
                        Err(e) => {
                            log::error!(
                                "[clap] save_result failed for track_id={}: {}",
                                prepped.track_id, e
                            );
                            let _ = conn.execute(
                                "UPDATE track_passes SET status = ?1, log = ?2, duration_ms = ?3, last_run_at = CURRENT_TIMESTAMP WHERE id = ?4",
                                rusqlite::params![pass_status::FAILED, e, elapsed_ms, prepped.pass_id],
                            );
                            let _ = app.emit("analysis-progress", serde_json::json!({
                                "track_id": prepped.track_id,
                                "pass_name": self.name(),
                                "status": pass_status::FAILED,
                            }));
                            crate::metrics_database::log_pipeline_metric(
                                app,
                                run_id,
                                prepped.track_id,
                                self.name(),
                                "failed",
                                elapsed_ms,
                                start_time_ms,
                                ended_time_ms,
                                audio_dur,
                                Some(&e)
                            );
                        }
                        Ok(()) => {
                            let _ = conn.execute(
                                "UPDATE track_passes SET status = ?1, duration_ms = ?2,
                                 pass_version = ?3, raw_result = ?4, last_run_at = CURRENT_TIMESTAMP WHERE id = ?5",
                                rusqlite::params![
                                    pass_status::DONE,
                                    elapsed_ms,
                                    pass_version::CLAP,
                                    raw_result,
                                    prepped.pass_id
                                ],
                            );
                            let _ = app.emit("analysis-progress", serde_json::json!({
                                "track_id": prepped.track_id,
                                "pass_name": self.name(),
                                "status": pass_status::DONE,
                            }));
                            crate::metrics_database::log_pipeline_metric(
                                app,
                                run_id,
                                prepped.track_id,
                                self.name(),
                                "success",
                                elapsed_ms,
                                start_time_ms,
                                ended_time_ms,
                                audio_dur,
                                None
                            );
                        }
                    }
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
                    crate::metrics_database::log_pipeline_metric(
                        app,
                        run_id,
                        prepped.track_id,
                        self.name(),
                        "failed",
                        elapsed_ms,
                        start_time_ms,
                        ended_time_ms,
                        audio_dur,
                        Some(&e)
                    );
                }
            }
        }

        for h in prep_workers {
            let _ = h.join();
        }

        // ── Batch concept tagging ─────────────────────────────────────────────
        // Run over all stored audio embeddings (not just this batch) so z-scores
        // reflect the full library distribution.
        log::info!("[clap] Starting concept tagging pass…");
        {
            let conn = super::lock_analysis_conn(conn_arc, self.name())?;
            if let Err(e) = run_concept_tagging(&conn, app) {
                log::error!("[clap] Concept tagging failed: {}", e);
            }
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
        owned_tag_sources: &["clap"],
        custom_reset: None,
    };
}

// ── Concept tagging ───────────────────────────────────────────────────────────

/// Maps AudioSet label → (namespace, tag_label) in the existing tag taxonomy.
const CONCEPT_MAP: &[(&str, &str, &str)] = &[
    // Instruments
    ("Acoustic guitar",                      "inst",  "acoustic guitar"),
    ("Electric guitar",                      "inst",  "electric guitar"),
    ("Bass guitar",                          "inst",  "bass guitar"),
    ("Double bass",                          "inst",  "double bass"),
    ("Steel guitar, slide guitar",           "inst",  "slide guitar"),
    ("Plucked string instrument",            "inst",  "plucked string"),
    ("Bowed string instrument",              "inst",  "bowed string"),
    ("String section",                       "inst",  "strings"),
    ("Violin, fiddle",                       "inst",  "violin"),
    ("Cello",                                "inst",  "cello"),
    ("Piano",                                "inst",  "piano"),
    ("Electric piano",                       "inst",  "electric piano"),
    ("Keyboard (musical)",                   "inst",  "keyboard"),
    ("Hammond organ",                        "inst",  "hammond organ"),
    ("Electronic organ",                     "inst",  "electronic organ"),
    ("Organ",                                "inst",  "organ"),
    ("Synthesizer",                          "inst",  "synthesizer"),
    ("Drum kit",                             "inst",  "drums"),
    ("Drum machine",                         "inst",  "drum machine"),
    ("Bass drum",                            "inst",  "bass drum"),
    ("Snare drum",                           "inst",  "snare"),
    ("Hi-hat",                               "inst",  "hi-hat"),
    ("Cymbal",                               "inst",  "cymbal"),
    ("Percussion",                           "inst",  "percussion"),
    ("Mallet percussion",                    "inst",  "mallet percussion"),
    ("Vibraphone",                           "inst",  "vibraphone"),
    ("Trumpet",                              "inst",  "trumpet"),
    ("Brass instrument",                     "inst",  "brass"),
    ("Wind instrument, woodwind instrument", "inst",  "woodwind"),
    ("Flute",                                "inst",  "flute"),
    ("Saxophone",                            "inst",  "saxophone"),
    ("Harmonica",                            "inst",  "harmonica"),
    ("Harpsichord",                          "inst",  "harpsichord"),
    ("Tapping (guitar technique)",           "inst",  "guitar tapping"),
    ("Singing bowl",                         "inst",  "singing bowl"),
    // Vocals
    ("Male singing",                         "vocal", "male"),
    ("Female singing",                       "vocal", "female"),
    ("Child singing",                        "vocal", "child"),
    ("Choir",                                "vocal", "choir"),
    ("Singing",                              "vocal", "singing"),
    ("Vocal music",                          "vocal", "vocals"),
    ("Synthetic singing",                    "vocal", "synthetic"),
    ("Beatboxing",                           "vocal", "beatbox"),
    ("Opera",                                "vocal", "opera"),
    ("Chant",                                "vocal", "chant"),
    ("Humming",                              "vocal", "humming"),
    ("Whistling",                            "vocal", "whistling"),
    ("Yodeling",                             "vocal", "yodeling"),
    ("Rapping",                              "vocal", "rap"),
    // Feel
    ("Angry music",                          "feel",  "angry"),
    ("Happy music",                          "feel",  "happy"),
    ("Sad music",                            "feel",  "sad"),
    ("Scary music",                          "feel",  "scary"),
    ("Tender music",                         "feel",  "tender"),
    ("Exciting music",                       "feel",  "exciting"),
    ("Funny music",                          "feel",  "funny"),
];

const CONCEPT_TEMPLATES: [&str; 3] = [
    "a song featuring {}",
    "music with {}",
    "{}",
];

const ZSCORE_THRESHOLD: f32 = 1.5;
const MAX_TAGS_PER_TRACK: usize = 15;

/// Embed a single concept by averaging over the three prompt templates.
fn embed_concept(concept: &str, app: &tauri::AppHandle) -> Result<Vec<f32>, String> {
    let mut sum = vec![0.0f32; 512];
    for tmpl in &CONCEPT_TEMPLATES {
        let text = tmpl.replace("{}", concept);
        let emb = embeddings::run_clap_text_embed(&text, Some(app))?;
        for (s, v) in sum.iter_mut().zip(emb.iter()) {
            *s += v;
        }
    }
    let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for s in &mut sum {
            *s /= norm;
        }
    }
    Ok(sum)
}

/// Run CLAP concept tagging over all tracks that have audio embeddings.
/// Z-scores similarities per concept across the library and writes tags
/// with source='clap' for tracks that exceed the threshold.
pub fn run_concept_tagging(conn: &Connection, app: &tauri::AppHandle) -> Result<(), String> {
    log::info!("[clap] Loading audio embeddings for concept tagging…");

    // Load all (track_id, embedding) pairs
    let mut stmt = conn.prepare(
        "SELECT track_id, embedding FROM audio_embeddings"
    ).map_err(|e| e.to_string())?;

    let rows: Vec<(i64, Vec<f32>)> = stmt
        .query_map([], |row| {
            let track_id: i64 = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((track_id, blob))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter_map(|(tid, blob)| {
            if blob.len() == 512 * 4 {
                let mut v: Vec<f32> = blob.chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 { for x in &mut v { *x /= norm; } }
                Some((tid, v))
            } else {
                None
            }
        })
        .collect();

    if rows.is_empty() {
        log::info!("[clap] No audio embeddings found — skipping concept tagging");
        return Ok(());
    }

    let track_ids: Vec<i64> = rows.iter().map(|(tid, _)| *tid).collect();
    let audio_mat: Vec<&Vec<f32>> = rows.iter().map(|(_, v)| v).collect();
    let n = track_ids.len();
    let m = CONCEPT_MAP.len();

    log::info!("[clap] Embedding {} concepts for {} tracks…", m, n);

    // sim_matrix[i * m + j] = similarity of track i to concept j
    let mut sim_matrix = vec![0.0f32; n * m];
    for (j, (concept_label, _, _)) in CONCEPT_MAP.iter().enumerate() {
        match embed_concept(concept_label, app) {
            Ok(cvec) => {
                for (i, audio_v) in audio_mat.iter().enumerate() {
                    let dot: f32 = audio_v.iter().zip(cvec.iter()).map(|(a, b)| a * b).sum();
                    sim_matrix[i * m + j] = dot;
                }
            }
            Err(e) => {
                log::warn!("[clap] Failed to embed concept '{}': {}", concept_label, e);
            }
        }
    }

    // Z-score per concept (column)
    let mut z_matrix = sim_matrix.clone();
    for j in 0..m {
        let col: Vec<f32> = (0..n).map(|i| sim_matrix[i * m + j]).collect();
        let mean = col.iter().sum::<f32>() / n as f32;
        let variance = col.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / n as f32;
        let std = variance.sqrt() + 1e-9;
        for i in 0..n {
            z_matrix[i * m + j] = (sim_matrix[i * m + j] - mean) / std;
        }
    }

    // Wipe all existing clap-source tags
    conn.execute("DELETE FROM track_tags WHERE source = 'clap'", [])
        .map_err(|e| e.to_string())?;

    // Write new tags
    let mut total_tags = 0usize;
    for (i, &track_id) in track_ids.iter().enumerate() {
        // Collect all concepts above threshold, sorted by z-score descending
        let mut above: Vec<(usize, f32)> = (0..m)
            .filter_map(|j| {
                let z = z_matrix[i * m + j];
                if z >= ZSCORE_THRESHOLD { Some((j, z)) } else { None }
            })
            .collect();
        above.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (j, _) in above.into_iter().take(MAX_TAGS_PER_TRACK) {
            let (_, ns, label) = CONCEPT_MAP[j];
            super::upsert_track_tag(conn, track_id, ns, label, "clap")?;
            total_tags += 1;
        }
    }

    log::info!("[clap] Concept tagging done — {} tags written across {} tracks", total_tags, n);
    Ok(())
}
