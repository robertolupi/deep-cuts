use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct BpmJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub bpm_raw: Option<f64>,
    pub genre: Option<String>,
}

impl super::PassJob for BpmJob {
    fn pass_id(&self) -> i64 {
        self.pass_id
    }
    fn track_id(&self) -> i64 {
        self.track_id
    }
}

pub struct BpmCorrectionPass;

impl super::AnalysisPass for BpmCorrectionPass {
    type Job = BpmJob;
    type Output = (crate::bpm::CorrectResult, String);

    fn name(&self) -> &'static str {
        "bpm_correction"
    }

    fn priority(&self) -> i32 {
        15
    }

    fn version(&self) -> u32 {
        pass_version::BPM_CORRECTION
    }

    fn dependencies(&self) -> &'static [&'static str] {
        &["audio_analysis"]
    }

    fn owned_columns(&self) -> &'static [&'static str] {
        &["bpm"]
    }

    fn owned_tables(&self) -> &'static [&'static str] {
        &[]
    }

    fn custom_reset(&self, conn: &Connection) -> Result<(), String> {
        crate::bpm::reset_bpm_data(conn)
    }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.bpm_raw, t.genre
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'bpm_correction'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(BpmJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                bpm_raw: row.get(2)?,
                genre: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        let result = crate::bpm::correct_bpm(job.bpm_raw, job.genre.as_deref());
        let raw_result = crate::bpm::format_bpm_result_json(job.bpm_raw, job.genre.as_deref(), &result);
        Ok((result, raw_result))
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        let (result, _) = output;
        crate::bpm::save_bpm_result(conn, job.track_id, &result)
    }

    fn raw_result_json(&self, output: &Self::Output) -> Option<String> {
        Some(output.1.clone())
    }
}

impl BpmCorrectionPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "bpm_correction",
        priority: 15,
        version: pass_version::BPM_CORRECTION,
        dependencies: &["audio_analysis"],
        owned_columns: &["bpm"],
        owned_tables: &[],
        custom_reset: Some(|conn| {
            crate::bpm::reset_bpm_data(conn)
        }),
    };
}
