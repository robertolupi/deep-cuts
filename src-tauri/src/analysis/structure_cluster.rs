use crate::scanner::sidecar::pass_version;
use rayon::prelude::*;
use rusqlite::Connection;
use std::collections::HashMap;

// ── Hyperparameters ────────────────────────────────────────────────────────
// A unique skeleton pattern must appear in at least this many tracks to get
// its own named cluster.  Rarer patterns are labelled as noise (cluster -1).
const MIN_CLUSTER_SIZE: usize = 10;


// ── Layer A: String utilities ──────────────────────────────────────────────

/// Collapse adjacent identical characters.
/// "IIVVPCCCCO" → "IVPCO"
pub fn skeleton(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut out = Vec::new();
    for c in s.chars() {
        if out.last() != Some(&c) {
            out.push(c);
        }
    }
    out.into_iter().collect()
}

// ── Layer C: Cluster naming ────────────────────────────────────────────────

/// Derive a human-readable label and a structure-filter-compatible regex
/// from a skeleton string.
///
/// Algorithm:
///   1. Peel a single-char prefix (typically 'I' or 'E') if it doesn't repeat
///   2. Peel a single-char suffix (typically 'O' or 'V') if it doesn't repeat
///      and differs from the prefix
///   3. Find the shortest repeating unit in the middle
///   4. Format: label "I·VPC×3·O", regex "^I+(V+P+C+){3,}O+$"
pub fn name_skeleton(sk: &str) -> (String, String) {
    if sk.is_empty() {
        return ("?".to_string(), "^.*$".to_string());
    }

    let chars: Vec<char> = sk.chars().collect();
    let n = chars.len();

    // 1. Peel prefix: only structural intro/end markers ('I' or 'E') that appear
    //    once at the start (i.e. not repeated and different from next char).
    let mut start = 0;
    let prefix = if n >= 2 && (chars[0] == 'I' || chars[0] == 'E') && chars[0] != chars[1] {
        start = 1;
        Some(chars[0])
    } else {
        None
    };

    // 2 & 3. Find the best suffix + repeating unit combination.
    //
    // Strategy: try with and without peeling the last char as a suffix.
    // Prefer whichever gives a higher repetition count (cleaner block structure).
    // Tie-break: prefer the version with a peeled suffix (shorter, cleaner middle).
    //
    // 'O' (outro) is always a candidate suffix. Any other terminal char is also
    // tried. We only peel if the last char differs from the second-to-last
    // (i.e. it isn't part of a run).
    let full_middle: String = chars[start..].iter().collect();
    let (full_unit, full_reps) = find_repeating_unit(&full_middle);

    let (unit, reps, suffix) = if chars.len() > start + 1
        && chars[n - 1] != chars[n - 2]
    {
        let candidate_suffix = chars[n - 1];
        let trimmed_middle: String = chars[start..n - 1].iter().collect();
        let (trim_unit, trim_reps) = find_repeating_unit(&trimmed_middle);
        // Prefer peeled version if it gives equal or more repetitions
        if trim_reps >= full_reps && !trimmed_middle.is_empty() {
            (trim_unit, trim_reps, Some(candidate_suffix))
        } else {
            (full_unit, full_reps, None)
        }
    } else {
        (full_unit, full_reps, None)
    };

    // 4. Format label and regex
    let label = format_label(prefix, &unit, reps, suffix);
    let regex  = format_regex(prefix, &unit, reps, suffix);

    (label, regex)
}

/// Find the shortest string that tiles `s` exactly.
/// Returns (unit, count). If no clean repetition exists, returns (s, 1).
fn find_repeating_unit(s: &str) -> (String, usize) {
    let n = s.len();
    if n == 0 {
        return (String::new(), 0);
    }
    for unit_len in 1..=(n / 2) {
        if n % unit_len != 0 {
            continue;
        }
        let unit = &s[..unit_len];
        let reps = n / unit_len;
        if unit.repeat(reps) == s {
            return (unit.to_string(), reps);
        }
    }
    (s.to_string(), 1)
}

fn format_label(prefix: Option<char>, unit: &str, reps: usize, suffix: Option<char>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(p) = prefix {
        parts.push(p.to_string());
    }
    if !unit.is_empty() {
        if reps > 1 {
            parts.push(format!("{}×{}", unit, reps));
        } else {
            parts.push(unit.to_string());
        }
    }
    if let Some(s) = suffix {
        parts.push(s.to_string());
    }
    parts.join("·")
}

fn format_regex(prefix: Option<char>, unit: &str, reps: usize, suffix: Option<char>) -> String {
    let mut rx = String::from("^");
    if let Some(p) = prefix {
        rx.push(p);
        rx.push('+');
    }
    if !unit.is_empty() {
        if reps > 1 {
            // Each letter in the unit becomes Letter+, wrapped in a counted group
            let inner: String = unit.chars().map(|c| format!("{}+", c)).collect();
            rx.push_str(&format!("({}){{{},}}", inner, reps));
        } else {
            // Single pass: just Letter+ for each char
            for c in unit.chars() {
                rx.push(c);
                rx.push('+');
            }
        }
    }
    if let Some(s) = suffix {
        rx.push(s);
        rx.push('+');
    }
    rx.push('$');
    rx
}

// ── Layer D: The pass ──────────────────────────────────────────────────────

pub struct StructureClusterPass;

impl<R: tauri::Runtime> super::BatchAnalysisPass<R> for StructureClusterPass {
    fn name(&self) -> &'static str { "structure_cluster" }
    fn priority(&self) -> i32 { 55 }
    fn version(&self) -> u32 { pass_version::STRUCTURE_CLUSTER }
    fn dependencies(&self) -> &'static [&'static str] { &["sax_alignment", "essentia"] }
    fn owned_tables(&self) -> &'static [&'static str] { &["structure_clusters"] }

    fn needs_run(&self, conn: &Connection) -> Result<bool, String> {
        // Run if the clusters table is empty OR any music track is unclassified
        let cluster_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM structure_clusters",
            [],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        if cluster_count == 0 {
            return Ok(true);
        }

        let unclassified: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tracks
             WHERE sax_alignment IS NOT NULL
               AND is_music = 1
               AND structure_cluster_id IS NULL",
            [],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;

        Ok(unclassified > 0)
    }

    fn execute(&self, _app: &tauri::AppHandle<R>, conn: &Connection) -> Result<crate::analysis::BatchPassResult, String> {
        // Load all pending track passes for structure_cluster that are in progress
        let mut pending_stmt = conn.prepare(
            "SELECT track_id FROM track_passes WHERE pass_name = 'structure_cluster' AND status = ?1"
        ).map_err(|e| e.to_string())?;
        let pending_ids: Vec<i64> = pending_stmt.query_map([crate::database::pass_status::IN_PROGRESS], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(|e| e.to_string())?;
        drop(pending_stmt);

        let pending_set: std::collections::HashSet<i64> = pending_ids.into_iter().collect();

        // ── 1. Load all music tracks with sax_alignment ───────────────────────
        struct TrackRow { id: i64, alignment: String }

        let mut stmt = conn.prepare(
            "SELECT id, sax_alignment FROM tracks
             WHERE sax_alignment IS NOT NULL
               AND is_music = 1",
        ).map_err(|e| e.to_string())?;

        let tracks: Vec<TrackRow> = stmt.query_map([], |row| {
            Ok(TrackRow { id: row.get(0)?, alignment: row.get(1)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

        drop(stmt);

        let mut succeeded_track_ids = Vec::new();
        let mut skipped_tracks = Vec::new();
        let failed_tracks = Vec::new();

        let n = tracks.len();
        if n < MIN_CLUSTER_SIZE {
            conn.execute("BEGIN", []).map_err(|e| e.to_string())?;
            conn.execute("DELETE FROM structure_clusters", [])
                .map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
            conn.execute("UPDATE tracks SET structure_cluster_id = NULL WHERE is_music = 1", [])
                .map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

            for track_id in pending_set {
                skipped_tracks.push((track_id, format!("Not enough data: only {} music tracks with alignment available", n)));
            }
            return Ok(crate::analysis::BatchPassResult {
                succeeded_track_ids,
                failed_tracks,
                skipped_tracks,
                summary: format!("only {} music tracks with alignment — skipping clustering", n),
            });
        }

        // ── 2. Compute skeletons in parallel ──────────────────────────────────
        crate::analysis::check_analysis_status()?;
        log::info!("[structure_cluster] computing {} skeletons (parallel)", n);
        let skeleton_strs: Vec<String> = tracks.par_iter()
            .map(|t| skeleton(&t.alignment))
            .collect();

        // ── 3. Count tracks per unique skeleton ───────────────────────────────
        let mut sk_counts: HashMap<&str, usize> = HashMap::new();
        for sk in &skeleton_strs {
            *sk_counts.entry(sk.as_str()).or_insert(0) += 1;
        }

        // Sort skeletons descending by track count, filter by minimum size.
        let mut ranked: Vec<(&str, usize)> = sk_counts
            .iter()
            .filter(|(_, &cnt)| cnt >= MIN_CLUSTER_SIZE)
            .map(|(&sk, &cnt)| (sk, cnt))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

        let sk_to_cluster: HashMap<&str, i32> = ranked
            .iter()
            .enumerate()
            .map(|(id, (sk, _))| (*sk, id as i32))
            .collect();

        let n_clusters = ranked.len();
        let labels: Vec<i32> = skeleton_strs.iter()
            .map(|sk| sk_to_cluster.get(sk.as_str()).copied().unwrap_or(-1))
            .collect();
        let n_noise = labels.iter().filter(|&&l| l < 0).count();
        log::info!("[structure_cluster] {} clusters from {} unique skeletons, {} noise tracks",
                   n_clusters, sk_counts.len(), n_noise);

        // ── 4. Name each cluster from its skeleton ────────────────────────────
        struct ClusterInfo { label: String, regex: String, track_count: usize }
        let mut cluster_infos: HashMap<i32, ClusterInfo> = HashMap::new();

        for (id, (sk, count)) in ranked.iter().enumerate() {
            let (label, regex) = name_skeleton(sk);
            cluster_infos.insert(id as i32, ClusterInfo { label, regex, track_count: *count });
        }

        // ── 6. Write back in one transaction ──────────────────────────────────
        crate::analysis::check_analysis_status()?;
        conn.execute("BEGIN", []).map_err(|e| e.to_string())?;

        // Clear old cluster data
        conn.execute("DELETE FROM structure_clusters", [])
            .map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
        conn.execute("UPDATE tracks SET structure_cluster_id = NULL WHERE is_music = 1", [])
            .map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;

        // Insert cluster metadata
        for (&cid, info) in &cluster_infos {
            conn.execute(
                "INSERT INTO structure_clusters (id, label, regex, track_count) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![cid, info.label, info.regex, info.track_count as i64],
            ).map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
        }

        // Update tracks
        for (i, &label) in labels.iter().enumerate() {
            let cluster_val: Option<i32> = if label >= 0 { Some(label) } else { None };
            conn.execute(
                "UPDATE tracks SET structure_cluster_id = ?1 WHERE id = ?2",
                rusqlite::params![cluster_val, tracks[i].id],
            ).map_err(|e| { let _ = conn.execute("ROLLBACK", []); e.to_string() })?;
        }

        conn.execute("COMMIT", []).map_err(|e| e.to_string())?;

        // Determine which pending track passes succeeded and which were skipped (non-applicable)
        let applicable_set: std::collections::HashSet<i64> = tracks.iter().map(|t| t.id).collect();
        for track_id in pending_set {
            if applicable_set.contains(&track_id) {
                succeeded_track_ids.push(track_id);
            } else {
                skipped_tracks.push((track_id, "Not applicable: non-music or missing sax_alignment".to_string()));
            }
        }

        let summary = format!(
            "{} clusters, {} noise, {} music tracks classified",
            n_clusters, n_noise, tracks.len() - n_noise
        );

        Ok(crate::analysis::BatchPassResult {
            succeeded_track_ids,
            failed_tracks,
            skipped_tracks,
            summary,
        })
    }
}

impl StructureClusterPass {
    pub const SPEC: super::PassSpec = super::PassSpec {
        name: "structure_cluster",
        priority: 55,
        version: pass_version::STRUCTURE_CLUSTER,
        dependencies: &["sax_alignment", "essentia"],
        owned_columns: &["structure_cluster_id"],
        // structure_clusters has no track_id column — cleared via custom_reset instead
        owned_tables: &[],
        owned_tag_sources: &[],
        custom_reset: Some(|conn| {
            conn.execute("DELETE FROM structure_clusters", [])
                .map(|_| ())
                .map_err(|e| e.to_string())
        }),
    };
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── skeleton ──────────────────────────────────────────────────────────

    #[test]
    fn test_skeleton_empty() {
        assert_eq!(skeleton(""), "");
    }

    #[test]
    fn test_skeleton_single_char() {
        assert_eq!(skeleton("I"), "I");
    }

    #[test]
    fn test_skeleton_all_same() {
        assert_eq!(skeleton("IIIII"), "I");
    }

    #[test]
    fn test_skeleton_alternating() {
        // skeleton collapses *adjacent identical* chars only; alternating stays the same
        assert_eq!(skeleton("IVIVIV"), "IVIVIV");
    }

    #[test]
    fn test_skeleton_realistic() {
        assert_eq!(skeleton("IIVVPCCCCO"), "IVPCO");
    }

    #[test]
    fn test_skeleton_no_repeats() {
        assert_eq!(skeleton("IVPCO"), "IVPCO");
    }

    // ── name_skeleton ─────────────────────────────────────────────────────

    #[test]
    fn test_name_skeleton_simple() {
        let (label, regex) = name_skeleton("IVPCO");
        assert_eq!(label, "I·VPC·O");
        assert_eq!(regex, "^I+V+P+C+O+$");
    }

    #[test]
    fn test_name_skeleton_repeating_x2() {
        let (label, regex) = name_skeleton("IVPCVPCO");
        assert_eq!(label, "I·VPC×2·O");
        assert_eq!(regex, "^I+(V+P+C+){2,}O+$");
    }

    #[test]
    fn test_name_skeleton_repeating_x3() {
        let (label, regex) = name_skeleton("IVPCVPCVPCO");
        assert_eq!(label, "I·VPC×3·O");
        assert_eq!(regex, "^I+(V+P+C+){3,}O+$");
    }

    #[test]
    fn test_name_skeleton_no_suffix() {
        let (label, regex) = name_skeleton("IVPCVPC");
        assert_eq!(label, "I·VPC×2");
        assert_eq!(regex, "^I+(V+P+C+){2,}$");
    }

    #[test]
    fn test_name_skeleton_no_prefix() {
        // V is not a structural prefix marker (only I/E qualify), so no prefix peeled
        let (label, regex) = name_skeleton("VPCVPCO");
        assert_eq!(label, "VPC×2·O");
        assert_eq!(regex, "^(V+P+C+){2,}O+$");
    }

    #[test]
    fn test_name_skeleton_trailing_v() {
        let (label, regex) = name_skeleton("IVPCV");
        assert_eq!(label, "I·VPC·V");
        assert_eq!(regex, "^I+V+P+C+V+$");
    }

    #[test]
    fn test_name_skeleton_single_char() {
        let (label, regex) = name_skeleton("I");
        assert_eq!(label, "I");
        assert_eq!(regex, "^I+$");
    }

    #[test]
    fn test_name_skeleton_empty() {
        let (label, _regex) = name_skeleton("");
        assert_eq!(label, "?");
    }
}
