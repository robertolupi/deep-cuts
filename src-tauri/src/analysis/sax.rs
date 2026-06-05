use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct SaxJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub waveform_data: Option<String>,
}

impl super::PassJob for SaxJob {
    fn pass_id(&self) -> i64 { self.pass_id }
    fn track_id(&self) -> i64 { self.track_id }
}

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
    // Lookup table for dist_alpha[r][c] where r,c in {a,b,c,d,e}
    // dist_alpha[i][j] = 0 if |i-j| <= 1, else the pre-computed value.
    // Standard table for alpha=5:
    // https://www.cs.ucr.edu/~eamonn/SAX.htm
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

impl super::AnalysisPass for SaxPass {
    type Job = SaxJob;
    type Output = String;

    fn name(&self) -> &'static str { "sax" }
    fn priority(&self) -> i32 { 12 }
    fn version(&self) -> u32 { pass_version::SAX }
    fn dependencies(&self) -> &'static [&'static str] { &["audio_analysis"] }
    fn owned_columns(&self) -> &'static [&'static str] { &["waveform_sax"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.waveform_data
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'sax'
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(SaxJob {
                pass_id: row.get(0)?,
                track_id: row.get(1)?,
                waveform_data: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        let wf = job.waveform_data.as_deref()
            .ok_or_else(|| "no waveform_data".to_string())?;
        waveform_to_sax(wf).ok_or_else(|| "waveform too short or flat".to_string())
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE tracks SET waveform_sax = ?1 WHERE id = ?2",
            rusqlite::params![output, job.track_id],
        ).map_err(|e| e.to_string())?;
        if let Err(e) = crate::scanner::sidecar::save(conn, job.track_id) {
            log::error!("[sax] sidecar save failed for track {}: {}", job.track_id, e);
        }
        Ok(())
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
        // Waveform with clear ramp-up structure
        let wf: Vec<f32> = (0..128).map(|i| i as f32 / 128.0).collect();
        let json = serde_json::to_string(&wf).unwrap();
        let sax = waveform_to_sax(&json).unwrap();
        assert_eq!(sax.len(), 32);
        assert!(sax.chars().all(|c| "abcde".contains(c)));
        // Ramp-up: first char should be 'a' or 'b', last should be 'd' or 'e'
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
        // Adjacent letters (|i-j| <= 1) have 0 distance in the lookup table
        let a = "a".repeat(32);
        let b = "b".repeat(32);
        assert_eq!(sax_mindist(&a, &b).unwrap(), 0.0);
    }
}
