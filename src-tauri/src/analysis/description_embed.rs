use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct DescriptionJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub is_music: Option<i64>,
    pub description: Option<String>,
    pub ai_genre: Option<String>,
    pub ai_mood: Option<String>,
    pub ai_instruments: Option<String>,
}

impl super::PassJob for DescriptionJob {
    fn pass_id(&self) -> i64 {
        self.pass_id
    }
    fn track_id(&self) -> i64 {
        self.track_id
    }
}

pub struct DescriptionEmbedPass;

impl super::AnalysisPass for DescriptionEmbedPass {
    type Job = DescriptionJob;
    type Output = (Option<Vec<f32>>, String);

    fn name(&self) -> &'static str {
        "description_embed"
    }

    fn priority(&self) -> i32 {
        40
    }

    fn version(&self) -> u32 {
        pass_version::DESCRIPTION_EMBED
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["qwen"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &[]
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &["description_embeddings"]
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.is_music, t.description, t.ai_genre, t.ai_mood, t.ai_instruments
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'description_embed'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(DescriptionJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                is_music: row.get(2)?,
                description: row.get(3)?,
                ai_genre: row.get(4)?,
                ai_mood: row.get(5)?,
                ai_instruments: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        // If not music, skip entirely
        if let Some(0) = job.is_music {
            log::info!(
                "[description_embed] Track {} marked as non-music. Skipping embedding.",
                job.track_id
            );
            let raw = serde_json::json!({"skipped": true, "reason": "non_music"}).to_string();
            return Ok((None, raw));
        }

        let desc = match &job.description {
            Some(d) if !d.trim().is_empty() => d,
            _ => {
                let raw = serde_json::json!({"skipped": true, "reason": "no_description"}).to_string();
                return Ok((None, raw));
            }
        };

        // Build concatenated text for richer semantic signal
        let mut embed_text = String::new();
        if let Some(g) = &job.ai_genre {
            if !g.trim().is_empty() {
                embed_text.push_str(&format!("Genre: {}. ", g));
            }
        }
        if let Some(m) = &job.ai_mood {
            if !m.trim().is_empty() {
                embed_text.push_str(&format!("Mood: {}. ", m));
            }
        }
        if let Some(i) = &job.ai_instruments {
            if !i.trim().is_empty() {
                embed_text.push_str(&format!("Instruments: {}. ", i));
            }
        }
        embed_text.push_str(desc);

        let raw = serde_json::json!({
            "skipped": false,
            "embed_text_len": embed_text.len(),
            "embed_text": embed_text,
        }).to_string();

        let embedding = crate::embeddings::run_sentence_embed(&embed_text, Some(app))?;
        Ok((Some(embedding), raw))
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        let (embedding, _) = output;
        if let Some(emb) = embedding {
            let blob: Vec<u8> = emb.iter().flat_map(|&f| f.to_le_bytes()).collect();
            conn.execute(
                "DELETE FROM description_embeddings WHERE track_id = ?1",
                rusqlite::params![job.track_id],
            ).map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO description_embeddings (track_id, embedding) VALUES (?1, ?2)",
                rusqlite::params![job.track_id, blob],
            ).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn raw_result_json(&self, output: &Self::Output) -> Option<String> {
        Some(output.1.clone())
    }
}

impl DescriptionEmbedPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "description_embed",
        priority: 60,
        version: pass_version::DESCRIPTION_EMBED,
        dependencies: &["qwen"],
        owned_columns: &[],
        owned_tables: &["description_embeddings"],
        custom_reset: None,
    };
}
