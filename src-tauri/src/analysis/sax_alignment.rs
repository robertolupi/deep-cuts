use crate::database::pass_status;
use crate::scanner::sidecar::pass_version;
use rusqlite::Connection;

pub struct SaxAlignmentJob {
    pub pass_id: i64,
    pub track_id: i64,
    pub waveform_sax: String,
}

impl super::PassJob for SaxAlignmentJob {
    fn pass_id(&self) -> i64 { self.pass_id }
    fn track_id(&self) -> i64 { self.track_id }
}

pub struct SaxAlignmentPass;

// ── Labels ────────────────────────────────────────────────────────────────────

const LABELS: [&str; 8] = [
    "unknown", "intro", "verse", "pre-chorus", "chorus", "bridge", "outro", "end",
];
const N_STATES: usize = LABELS.len();

// ── Emission model ─────────────────────────────────────────────────────────

// SAX letters a–e mapped to energy [0.0, 0.25, 0.5, 0.75, 1.0]
fn sax_energy(c: char) -> f32 {
    match c {
        'a' => 0.0,
        'b' => 0.25,
        'c' => 0.5,
        'd' => 0.75,
        _   => 1.0,
    }
}

// Gaussian emission: p(energy | label), evaluated at `e`.
// Each label has a preferred energy center and spread.
fn emission(label_idx: usize, energy: f32) -> f32 {
    // (center, std_dev) per label
    const PARAMS: [(f32, f32); N_STATES] = [
        (0.5,  0.40),   // unknown  — flat/uninformative
        (0.15, 0.20),   // intro    — quiet opening
        (0.45, 0.22),   // verse    — moderate energy
        (0.60, 0.18),   // pre-chorus — building
        (0.85, 0.18),   // chorus   — high energy peak
        (0.55, 0.22),   // bridge   — mid/varied
        (0.20, 0.22),   // outro    — winding down
        (0.05, 0.12),   // end      — near silence
    ];
    let (mu, sigma) = PARAMS[label_idx];
    let z = (energy - mu) / sigma;
    (-0.5 * z * z).exp()
}

// ── Transition matrix ─────────────────────────────────────────────────────

// Music-aware transition priors calibrated from Meta's recommendations.
// Index order matches LABELS.
fn build_transition() -> [[f32; N_STATES]; N_STATES] {
    // Start with small smoothing for all transitions.
    let mut t = [[0.02_f32; N_STATES]; N_STATES];

    // State indices for readability
    const UNKNOWN:    usize = 0;
    const INTRO:      usize = 1;
    const VERSE:      usize = 2;
    const PRE_CHORUS: usize = 3;
    const CHORUS:     usize = 4;
    const BRIDGE:     usize = 5;
    const OUTRO:      usize = 6;
    const END:        usize = 7;

    // Natural song-structure flow (from → to)
    t[INTRO][INTRO]         = 0.40;
    t[INTRO][VERSE]         = 0.45;
    t[INTRO][CHORUS]        = 0.10;

    t[VERSE][VERSE]         = 0.35;
    t[VERSE][PRE_CHORUS]    = 0.30;
    t[VERSE][CHORUS]        = 0.25;
    t[VERSE][BRIDGE]        = 0.05;

    t[PRE_CHORUS][PRE_CHORUS] = 0.20;
    t[PRE_CHORUS][CHORUS]   = 0.70;
    t[PRE_CHORUS][VERSE]    = 0.05;

    t[CHORUS][CHORUS]       = 0.35;
    t[CHORUS][VERSE]        = 0.30;
    t[CHORUS][BRIDGE]       = 0.15;
    t[CHORUS][OUTRO]        = 0.15;

    t[BRIDGE][BRIDGE]       = 0.20;
    t[BRIDGE][CHORUS]       = 0.55;
    t[BRIDGE][OUTRO]        = 0.15;
    t[BRIDGE][VERSE]        = 0.05;

    t[OUTRO][OUTRO]         = 0.50;
    t[OUTRO][END]           = 0.35;
    t[OUTRO][CHORUS]        = 0.05;

    t[END][END]             = 0.90;

    t[UNKNOWN][UNKNOWN]     = 0.15;
    t[UNKNOWN][INTRO]       = 0.20;
    t[UNKNOWN][VERSE]       = 0.15;
    t[UNKNOWN][CHORUS]      = 0.10;
    t[UNKNOWN][OUTRO]       = 0.15;
    t[UNKNOWN][END]         = 0.15;

    // Row-normalise
    for row in &mut t {
        let sum: f32 = row.iter().sum();
        if sum > 0.0 {
            for v in row.iter_mut() {
                *v /= sum;
            }
        }
    }
    t
}

// ── Viterbi ───────────────────────────────────────────────────────────────

fn viterbi(emissions: &[[f32; N_STATES]], transition: &[[f32; N_STATES]; N_STATES]) -> Vec<usize> {
    let t_len = emissions.len();
    let mut dp   = vec![[f32::NEG_INFINITY; N_STATES]; t_len];
    let mut back = vec![[0usize; N_STATES]; t_len];

    // Uniform initial distribution — no strong prior on where a track starts.
    let init = (1.0 / N_STATES as f32).ln();
    for s in 0..N_STATES {
        dp[0][s] = init + emissions[0][s].max(1e-9).ln();
    }

    for t in 1..t_len {
        for s in 0..N_STATES {
            let (best_prev, best_val) = (0..N_STATES)
                .map(|sp| {
                    let v = dp[t-1][sp] + transition[sp][s].max(1e-9).ln();
                    (sp, v)
                })
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .unwrap();
            dp[t][s]   = best_val + emissions[t][s].max(1e-9).ln();
            back[t][s] = best_prev;
        }
    }

    // Backtrack
    let mut path = vec![0usize; t_len];
    path[t_len-1] = (0..N_STATES)
        .max_by(|&a, &b| dp[t_len-1][a].partial_cmp(&dp[t_len-1][b]).unwrap())
        .unwrap();
    for t in (1..t_len).rev() {
        path[t-1] = back[t][path[t]];
    }
    path
}

// ── Post-processing ───────────────────────────────────────────────────────

// One letter per label, matching LABELS index order.
const LETTERS: [char; 8] = ['U', 'I', 'V', 'P', 'C', 'B', 'O', 'E'];

// Compact a flat label sequence into a structural alphabet string.
// Each section is represented by its letter repeated once per occurrence (max 4).
// e.g. [intro, intro, verse, verse, verse, chorus, outro] → "IIVVVCO"
// Leading "unknown" runs → intro; trailing "unknown" runs → outro.
fn compact(path: &[usize]) -> String {
    if path.is_empty() {
        return String::new();
    }

    // Remap unknown at boundaries
    let mut labels: Vec<usize> = path.to_vec();
    let max_leading = 4;
    let mut leading = 0;
    while leading < labels.len() && labels[leading] == 0 && leading < max_leading {
        labels[leading] = 1; // intro
        leading += 1;
    }
    let mut trailing = labels.len();
    while trailing > 0 && labels[trailing - 1] == 0 {
        labels[trailing - 1] = 6; // outro
        trailing -= 1;
    }

    // RLE then emit letter repeated count times (capped at 4)
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for &s in &labels {
        if let Some(last) = runs.last_mut() {
            if last.0 == s { last.1 += 1; continue; }
        }
        runs.push((s, 1));
    }

    runs.iter()
        .flat_map(|&(idx, count)| std::iter::repeat(LETTERS[idx]).take(count.min(4)))
        .collect()
}

// ── Public algorithm entry-point ──────────────────────────────────────────

pub struct AlignmentResult {
    /// Structural alphabet string, e.g. `"IIVVVVPCCCCO"`. Letters repeat per section count (max 4).
    pub compacted: String,
    /// Comma-separated label per segment for per-segment coloring, e.g. `"intro,intro,verse,verse,chorus,outro"`.
    pub segments: String,
}

pub fn align_sax(waveform_sax: &str) -> Option<AlignmentResult> {
    let chars: Vec<char> = waveform_sax.chars().collect();
    let n = chars.len();
    if n == 0 {
        return None;
    }
    let n_seg = 16usize;
    let chunk = (n as f32 / n_seg as f32).ceil() as usize;

    let mut emissions = [[0.0_f32; N_STATES]; 16];
    for seg in 0..n_seg {
        let start = seg * chunk;
        let end   = ((seg + 1) * chunk).min(n);
        let avg_energy = if start < n {
            chars[start..end].iter().map(|&c| sax_energy(c)).sum::<f32>() / (end - start) as f32
        } else {
            0.5 // pad with mid energy
        };
        for s in 0..N_STATES {
            emissions[seg][s] = emission(s, avg_energy);
        }
    }

    let transition = build_transition();
    let path = viterbi(&emissions, &transition);

    let segments = path.iter().map(|&s| LABELS[s]).collect::<Vec<_>>().join(",");
    let compacted = compact(&path);

    Some(AlignmentResult { compacted, segments })
}

// ── Analysis pass ─────────────────────────────────────────────────────────

pub struct SaxAlignmentOutput {
    pub compacted: String,
    pub segments: String,
}

impl super::AnalysisPass for SaxAlignmentPass {
    type Job = SaxAlignmentJob;
    type Output = SaxAlignmentOutput;

    fn name(&self) -> &'static str { "sax_alignment" }
    fn priority(&self) -> i32 { 13 }
    fn version(&self) -> u32 { pass_version::SAX_ALIGNMENT }
    fn dependencies(&self) -> &'static [&'static str] { &["sax"] }
    fn owned_columns(&self) -> &'static [&'static str] { &["sax_alignment", "sax_alignment_segments"] }
    fn owned_tables(&self) -> &'static [&'static str] { &[] }

    fn load_jobs(&self, conn: &Connection) -> Result<Vec<Self::Job>, String> {
        let mut stmt = conn.prepare(
            "SELECT tp.id, tp.track_id, t.waveform_sax
             FROM track_passes tp
             JOIN tracks t ON t.id = tp.track_id
             WHERE tp.status = ?1 AND tp.pass_name = 'sax_alignment'
               AND t.waveform_sax IS NOT NULL
             ORDER BY tp.id ASC",
        ).map_err(|e| e.to_string())?;

        let rows = stmt.query_map([pass_status::PENDING], |row| {
            Ok(SaxAlignmentJob {
                pass_id:      row.get(0)?,
                track_id:     row.get(1)?,
                waveform_sax: row.get::<_, String>(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

        Ok(rows)
    }

    fn execute_job(&self, _app: &tauri::AppHandle, job: &Self::Job) -> Result<Self::Output, String> {
        let result = align_sax(&job.waveform_sax)
            .ok_or_else(|| "empty waveform_sax".to_string())?;
        Ok(SaxAlignmentOutput { compacted: result.compacted, segments: result.segments })
    }

    fn save_result(
        &self,
        conn: &Connection,
        job: &Self::Job,
        output: Self::Output,
        _duration_ms: i64,
    ) -> Result<(), String> {
        conn.execute(
            "UPDATE tracks SET sax_alignment = ?1, sax_alignment_segments = ?2 WHERE id = ?3",
            rusqlite::params![output.compacted, output.segments, job.track_id],
        ).map_err(|e| e.to_string())?;
        if let Err(e) = crate::scanner::sidecar::save(conn, job.track_id) {
            log::error!("[sax_alignment] sidecar save failed for track {}: {}", job.track_id, e);
        }
        Ok(())
    }
}

impl SaxAlignmentPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "sax_alignment",
        priority: 13,
        version: pass_version::SAX_ALIGNMENT,
        dependencies: &["sax"],
        owned_columns: &["sax_alignment", "sax_alignment_segments"],
        owned_tables: &[],
        owned_tag_sources: &[],
        custom_reset: None,
    };
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_sax_returns_nonempty_for_valid_input() {
        let sax = "aaaaaaaacccccccceeeeeeeebbbbbbbb";
        let result = align_sax(sax).unwrap();
        assert!(!result.compacted.is_empty());
        assert!(result.compacted.chars().all(|c| "UIVPCBOE".contains(c)),
            "expected alphabet chars only, got: {}", result.compacted);
        assert_eq!(result.segments.split(',').count(), 16, "expected 16 segments");
    }

    #[test]
    fn test_align_sax_quiet_intro_loud_chorus_quiet_outro() {
        let sax = "aaaabbbbddddeeeeeeeeeeeedddbbbaaa";
        let result = align_sax(sax).unwrap();
        assert!(result.compacted.starts_with('I'), "expected I start, got: {}", result.compacted);
        assert!(result.compacted.contains('C'), "expected C (chorus), got: {}", result.compacted);
    }

    #[test]
    fn test_align_sax_empty_input() {
        assert!(align_sax("").is_none());
    }

    #[test]
    fn test_segments_has_valid_labels() {
        let sax = "aaaaaaaacccccccceeeeeeeebbbbbbbb";
        let result = align_sax(sax).unwrap();
        for seg in result.segments.split(',') {
            assert!(LABELS.contains(&seg), "unexpected label: {seg}");
        }
    }

    #[test]
    fn test_compact_alphabet() {
        // 8 chorus states → "CCCC" (capped at 4)
        let path = vec![4usize; 8];
        assert_eq!(compact(&path), "CCCC");
        // 2 intro + 1 verse → "IIV"
        let path2 = vec![1, 1, 2];
        assert_eq!(compact(&path2), "IIV");
    }

    #[test]
    fn test_leading_unknown_maps_to_intro() {
        let mut path = vec![0usize; 3]; // unknown × 3
        path.extend_from_slice(&[4, 4, 4, 4, 6, 6]); // chorus × 4, outro × 2
        let result = compact(&path);
        assert!(result.starts_with('I'), "got: {result}");
    }
}
