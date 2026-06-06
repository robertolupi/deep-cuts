use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct SaxPass;

/// SAX breakpoints for a 5-letter alphabet over a standard normal distribution.
/// Boundaries between: a|b=-0.841, b|c=-0.253, c|d=+0.253, d|e=+0.841
const BREAKS: [f32; 4] = [-0.841, -0.253, 0.253, 0.841];
const N_SEGMENTS: usize = 32;

/// Converts a waveform envelope to a 32-character SAX string (alphabet a–e).
///
/// Returns None if the waveform is empty, non-finite, or has zero variance.
pub fn waveform_to_sax(waveform_data: &str) -> Option<String> {
    let wf: Vec<f32> = serde_json::from_str(waveform_data).ok()?;
    let vals: Vec<f32> = wf.iter().copied().filter(|v| v.is_finite() && *v > 0.0).collect();
    if vals.len() < N_SEGMENTS {
        return None;
    }

    // PAA: group into N_SEGMENTS equal chunks, take the mean of each.
    let chunk = vals.len() / N_SEGMENTS;
    let paa: Vec<f32> = (0..N_SEGMENTS)
        .map(|i| {
            let slice = &vals[i * chunk..(i + 1) * chunk];
            slice.iter().sum::<f32>() / slice.len() as f32
        })
        .collect();

    // z-normalise
    let mean = paa.iter().sum::<f32>() / N_SEGMENTS as f32;
    let variance = paa.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / N_SEGMENTS as f32;
    let std = variance.sqrt();
    if std < 1e-6 {
        // Flat — use all 'c' (midpoint letter)
        return Some("c".repeat(N_SEGMENTS));
    }
    let normed: Vec<f32> = paa.iter().map(|v| (v - mean) / std).collect();

    // Quantize to letters a–e
    let sax: String = normed
        .iter()
        .map(|&z| {
            if z < BREAKS[0] { 'a' }
            else if z < BREAKS[1] { 'b' }
            else if z < BREAKS[2] { 'c' }
            else if z < BREAKS[3] { 'd' }
            else { 'e' }
        })
        .collect();

    Some(sax)
}

/// Computes the SAX MINDIST lower-bound distance between two SAX strings.
/// Uses the standard lookup table for a 5-letter alphabet.
///
/// Returns None if the strings differ in length or are empty.
pub fn sax_mindist(a: &str, b: &str) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    const DIST: [[f64; 5]; 5] = [
        [0.0,   0.0,   0.674, 1.340, 1.985],
        [0.0,   0.0,   0.0,   0.674, 1.340],
        [0.674, 0.0,   0.0,   0.0,   0.674],
        [1.340, 0.674, 0.0,   0.0,   0.0  ],
        [1.985, 1.340, 0.674, 0.0,   0.0  ],
    ];

    let n = a.len() as f64;
    let sum_sq: f64 = a.chars().zip(b.chars()).map(|(ca, cb)| {
        let i = (ca as u8 - b'a') as usize;
        let j = (cb as u8 - b'a') as usize;
        let d = DIST[i][j];
        d * d
    }).sum();

    Some((sum_sq / n).sqrt())
}

impl super::BatchAnalysisPass for SaxPass {
    fn name(&self) -> &'static str { "sax" }
    fn priority(&self) -> i32 { 12 }
    fn version(&self) -> u32 { pass_version::SAX }
    fn dependencies(&self) -> &'static [&'static str] { &["audio_analysis"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn needs_run(&self, conn: &Connection) -> Result<bool, String> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM track_passes WHERE pass_name = 'sax' AND status = ?1",
            rusqlite::params![pass_status::PENDING],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        Ok(count > 0)
    }

    fn execute(&self, _app: &tauri::AppHandle, conn: &Connection) -> Result<String, String> {
        // ── 1. Load all pending jobs in one query ─────────────────────────────
        struct PendingJob { pass_id: i64, track_id: i64, waveform_data: Option<String> }

        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.waveform_data
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.pass_name = 'sax' AND tp.status = ?1
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let jobs: Vec<PendingJob> = stmt.query_map(rusqlite::params![pass_status::PENDING], |row| {
            Ok(PendingJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                waveform_data: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        let total = jobs.len();

        // ── 2. Compute SAX for all jobs in memory (no DB) ─────────────────────
        struct ComputeResult { pass_id: i64, track_id: i64, sax: Option<String> }

        let results: Vec<ComputeResult> = jobs.into_iter().map(|job| {
            let sax = job.waveform_data.as_deref().and_then(waveform_to_sax);
            ComputeResult { pass_id: job.pass_id, track_id: job.track_id, sax }
        }).collect();

        // ── 3. Write everything in one transaction ────────────────────────────
        let now = chrono::Utc::now().timestamp();
        let mut processed = 0usize;
        let mut skipped = 0usize;

        conn.execute("BEGIN", []).map_err(|e| e.to_string())?;

        for r in &results {
            let write_result = match &r.sax {
                Some(sax) => {
                    conn.execute(
                        "UPDATE tracks SET waveform_sax = ?1 WHERE id = ?2",
                        rusqlite::params![sax, r.track_id],
                    ).and_then(|_| conn.execute(
                        "UPDATE track_passes SET status = ?1, pass_version = ?2, last_run_at = ?3
                         WHERE id = ?4",
                        rusqlite::params![pass_status::DONE, pass_version::SAX, now, r.pass_id],
                    )).map(|_| true)
                }
                None => {
                    conn.execute(
                        "UPDATE track_passes SET status = ?1, log = 'waveform too short or flat',
                         last_run_at = ?2 WHERE id = ?3",
                        rusqlite::params![pass_status::FAILED, now, r.pass_id],
                    ).map(|_| false)
                }
            };

            match write_result {
                Ok(true)  => processed += 1,
                Ok(false) => skipped += 1,
                Err(e) => {
                    let _ = conn.execute("ROLLBACK", []);
                    return Err(format!("sax batch write failed for track {}: {}", r.track_id, e));
                }
            }
        }

        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

        // ── 4. Sidecar saves (outside transaction, best-effort) ───────────────
        for r in &results {
            if r.sax.is_some() {
                if let Err(e) = crate::scanner::sidecar::save(conn, r.track_id) {
                    log::warn!("[sax] sidecar save failed for track {}: {}", r.track_id, e);
                }
            }
        }

        Ok(format!("{} processed, {} skipped (flat/short), {} total", processed, skipped, total))
    }
}

impl SaxPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "sax",
        priority: 12,
        version: pass_version::SAX,
        dependencies: &["audio_analysis"],
        owned_columns: &["waveform_sax"],
        owned_tables: &[],
        owned_tag_sources: &[],
        custom_reset: None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_to_sax_produces_32_char_string() {
        let wf: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
        let json = serde_json::to_string(&wf).unwrap();
        let sax = waveform_to_sax(&json).unwrap();
        assert_eq!(sax.len(), 32);
        assert!(sax.chars().all(|c| "abcde".contains(c)));
        assert!(sax.starts_with(|c| c == 'a' || c == 'b'));
        assert!(sax.ends_with(|c| c == 'd' || c == 'e'));
    }

    #[test]
    fn test_waveform_to_sax_flat_returns_all_c() {
        let wf: Vec<f32> = vec![0.5; 128];
        let json = serde_json::to_string(&wf).unwrap();
        let sax = waveform_to_sax(&json).unwrap();
        assert_eq!(sax, "c".repeat(32));
    }

    #[test]
    fn test_sax_mindist_identical_strings_is_zero() {
        let sax = "abcdeedcbaabcdeedcbaabcdeedcbaab";
        assert!((sax_mindist(sax, sax).unwrap()).abs() < 1e-9);
    }

    #[test]
    fn test_sax_mindist_maximally_different() {
        let a = "a".repeat(32);
        let b = "e".repeat(32);
        let d = sax_mindist(&a, &b).unwrap();
        assert!(d > 1.0, "expected large distance, got {}", d);
    }

    #[test]
    fn test_sax_mindist_adjacent_letters_is_zero() {
        let a = "a".repeat(32);
        let b = "b".repeat(32);
        assert_eq!(sax_mindist(&a, &b).unwrap(), 0.0);
    }
}
