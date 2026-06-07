use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

// ── Job ─────────────────────────────────────────────────────────────────────

pub struct BoundaryRefineJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub duration_seconds: i64,
    pub waveform_data: String,
    pub sax_alignment_segments: String,
}

impl super::PassJob for BoundaryRefineJob {
    fn pass_id(&self) -> i64 { self.pass_id }
    fn track_id(&self) -> i64 { self.track_id }
}

pub struct BoundaryRefinePass;

// ── Algorithm ──────────────────────────────────────────────────────────────
//
// Frozen `augment+8peaks_5s` config, ported exactly from
// tools/refine_salami_boundaries.py (validation F1 6.52/28.99, holdout 4.65/29.00
// at ±0.5s/±3s). The novelty signal is intentionally the existing
// `waveform_data` energy envelope proxy — that is what the result was validated on.

/// Baseline boundaries = edges of the 16-bin sax_alignment_segments where adjacent
/// labels differ, placed at `i * duration / n`.
fn baseline_boundaries(labels: &[&str], duration: f64) -> Vec<f64> {
    let n = labels.len();
    let bin_dur = duration / n as f64;
    (1..n)
        .filter(|&i| labels[i - 1] != labels[i])
        .map(|i| i as f64 * bin_dur)
        .collect()
}

/// Novelty peaks as (time, magnitude), sorted by descending magnitude.
/// Novelty = |first-difference| of the energy envelope; a local maximum at index
/// `i` maps to time `(i + 1) * duration / n`.
fn ranked_novelty_peaks(env: &[f64], duration: f64) -> Vec<(f64, f64)> {
    let n = env.len();
    if n < 3 {
        return Vec::new();
    }
    // nov[i] = |env[i+1] - env[i]| for i in 0..n-1  (len n-1)
    let nov: Vec<f64> = (1..n).map(|i| (env[i] - env[i - 1]).abs()).collect();
    let mut out: Vec<(f64, f64)> = Vec::new();
    for i in 1..nov.len().saturating_sub(1) {
        if nov[i] >= nov[i - 1] && nov[i] > nov[i + 1] {
            let t = (i as f64 + 1.0) * (duration / n as f64);
            out.push((t, nov[i]));
        }
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Add the strongest novelty peaks that are at least `min_gap` from every existing
/// boundary, up to `n_add` new boundaries. Returns the merged, sorted boundary set.
fn augment_with_peaks(
    base: &[f64],
    ranked_peaks: &[(f64, f64)],
    n_add: usize,
    min_gap: f64,
) -> Vec<f64> {
    let mut bounds: Vec<f64> = base.to_vec();
    let mut added = 0usize;
    for &(t, _) in ranked_peaks {
        if added >= n_add {
            break;
        }
        if bounds.iter().all(|&b| (t - b).abs() >= min_gap) {
            bounds.push(t);
            added += 1;
        }
    }
    bounds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    bounds
}

/// Compute the refined boundary set for one track. Returns sorted boundary times
/// in seconds, or `None` when inputs are insufficient (no segments / no duration).
pub fn refine_boundaries(
    duration: f64,
    waveform_data: &str,
    sax_alignment_segments: &str,
) -> Option<Vec<f64>> {
    if duration <= 0.0 {
        return None;
    }
    let labels: Vec<&str> = sax_alignment_segments.split(',').collect();
    if labels.len() != 16 {
        return None;
    }
    let base = baseline_boundaries(&labels, duration);

    // Energy envelope from waveform_data (JSON array of floats).
    let env: Vec<f64> = serde_json::from_str(waveform_data).ok()?;
    let ranked = ranked_novelty_peaks(&env, duration);

    Some(augment_with_peaks(&base, &ranked, 8, 5.0))
}

// ── Analysis pass ──────────────────────────────────────────────────────────

pub struct BoundaryRefineOutput {
    /// JSON array of boundary times in seconds, sorted.
    pub boundaries_json: String,
}

impl super::AnalysisPass for BoundaryRefinePass {
    type Job = BoundaryRefineJob;
    type Output = BoundaryRefineOutput;

    fn name(&self) -> &'static str { "boundary_refine" }
    fn priority(&self) -> i32 { 14 }
    fn version(&self) -> u32 { pass_version::BOUNDARY_REFINE }
    fn dependencies(&self) -> &'static [&'static str] { &["sax_alignment"] }
    fn owned_columns(&self) -> &'static [&'static str] { &["sax_alignment_boundaries"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.duration_seconds, t.waveform_data, t.sax_alignment_segments
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'boundary_refine'
               AND t.waveform_data IS NOT NULL
               AND t.sax_alignment_segments IS NOT NULL
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(BoundaryRefineJob {
                pass_id:                row.get(0)?,
                track_id:               row.get(1)?,
                duration_seconds:       row.get::<_, i64>(2)?,
                waveform_data:          row.get::<_, String>(3)?,
                sax_alignment_segments: row.get::<_, String>(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        let bounds = refine_boundaries(
            job.duration_seconds as f64,
            &job.waveform_data,
            &job.sax_alignment_segments,
        )
        .ok_or_else(|| "insufficient inputs for boundary refinement".to_string())?;
        let boundaries_json = serde_json::to_string(&bounds).map_err(|e| e.to_string())?;
        Ok(BoundaryRefineOutput { boundaries_json })
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE tracks SET sax_alignment_boundaries = ?1 WHERE id = ?2",
            rusqlite::params![output.boundaries_json, job.track_id],
        ).map_err(|e| e.to_string())?;
        if let Err(e) = crate::scanner::sidecar::save(conn, job.track_id) {
            log::error!("[boundary_refine] sidecar save failed for track {}: {}", job.track_id, e);
        }
        Ok(())
    }
}

impl BoundaryRefinePass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "boundary_refine",
        priority: 14,
        version: pass_version::BOUNDARY_REFINE,
        dependencies: &["sax_alignment"],
        owned_columns: &["sax_alignment_boundaries"],
        owned_tables: &[],
        owned_tag_sources: &[],
        custom_reset: None,
    };
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(labels: &[&str]) -> String {
        labels.join(",")
    }

    #[test]
    fn test_baseline_boundaries_at_label_edges() {
        // 16 bins, two segments: first 8 = intro, last 8 = chorus.
        let mut labels = vec!["intro"; 8];
        labels.extend(vec!["chorus"; 8]);
        let duration = 160.0;
        let base = baseline_boundaries(&labels, duration);
        // single edge at bin 8 → 8 * (160/16) = 80.0
        assert_eq!(base.len(), 1);
        assert!((base[0] - 80.0).abs() < 1e-9);
    }

    #[test]
    fn test_refine_requires_16_segments() {
        // 4 segments → reject
        let four = segs(&["intro", "verse", "chorus", "outro"]);
        assert!(refine_boundaries(120.0, "[0.1,0.2,0.3]", &four).is_none());
    }

    #[test]
    fn test_refine_rejects_zero_duration() {
        let labels: Vec<&str> = vec!["intro"; 16];
        assert!(refine_boundaries(0.0, "[0.1,0.2,0.3]", &segs(&labels)).is_none());
    }

    #[test]
    fn test_augment_respects_min_gap_and_cap() {
        let base = vec![10.0, 50.0];
        // peaks: one strong near an existing boundary (rejected), several far apart.
        let ranked = vec![
            (10.5, 9.0), // within 5s of 10.0 → rejected
            (30.0, 8.0), // accepted
            (31.0, 7.0), // within 5s of 30.0 (once added) → rejected
            (70.0, 6.0), // accepted
            (90.0, 5.0), // accepted
        ];
        let out = augment_with_peaks(&base, &ranked, 8, 5.0);
        // base (2) + 3 accepted = 5, sorted
        assert_eq!(out, vec![10.0, 30.0, 50.0, 70.0, 90.0]);
    }

    #[test]
    fn test_augment_caps_at_n_add() {
        let base = vec![0.0];
        let ranked: Vec<(f64, f64)> = (1..20).map(|i| (i as f64 * 10.0, 20.0 - i as f64)).collect();
        let out = augment_with_peaks(&base, &ranked, 8, 5.0);
        // base (1) + at most 8 added
        assert_eq!(out.len(), 9);
    }

    #[test]
    fn test_refine_full_pipeline_produces_sorted_superset_of_baseline() {
        // 16 segments with two label changes → 2 baseline boundaries.
        let mut labels = vec!["intro"; 5];
        labels.extend(vec!["verse"; 6]);
        labels.extend(vec!["chorus"; 5]);
        let duration = 160.0;
        // An envelope with clear energy steps to generate novelty peaks.
        let env: Vec<f64> = vec![
            0.1, 0.1, 0.1, 0.6, 0.6, 0.2, 0.2, 0.9, 0.9, 0.3, 0.3, 0.8, 0.8, 0.1, 0.1, 0.5,
        ];
        let wf = serde_json::to_string(&env).unwrap();
        let out = refine_boundaries(duration, &wf, &segs(&labels)).unwrap();

        // Sorted ascending.
        for w in out.windows(2) {
            assert!(w[0] <= w[1], "boundaries must be sorted: {:?}", out);
        }
        // Baseline edges at bins 5 and 11 must be present.
        let bin = duration / 16.0;
        for edge in [5.0 * bin, 11.0 * bin] {
            assert!(
                out.iter().any(|&b| (b - edge).abs() < 1e-9),
                "expected baseline edge {} in {:?}",
                edge,
                out
            );
        }
    }
}
