use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrackSetStats {
    pub track_count: i64,
    pub total_duration_seconds: f64,
    pub avg_bpm: Option<f64>,
    pub bpm_stddev: Option<f64>,
    pub most_common_key: Option<String>,
    pub key_variety: f64,
    pub pct_vocals: f64,
    pub pct_analysed: f64,
    pub avg_loudness_lufs: Option<f64>,

    pub avg_mood_happy: Option<f64>,
    pub avg_mood_sad: Option<f64>,
    pub avg_mood_aggressive: Option<f64>,
    pub avg_mood_relaxed: Option<f64>,
    pub avg_mood_party: Option<f64>,
    pub avg_mood_acoustic: Option<f64>,
    pub avg_mood_electronic: Option<f64>,

    pub major_count: i64,
    pub minor_count: i64,

    pub vocal_count: i64,
    pub instrumental_count: i64,
    pub unknown_vocal_count: i64,

    pub coverage_essentia: f64,
    pub coverage_mood: f64,
    pub coverage_qwen: f64,
    pub coverage_qwen_description: f64,
    pub coverage_qwen_instruments: f64,
    pub coverage_qwen_mood: f64,
    pub coverage_qwen_genre: f64,
    pub coverage_clap: f64,
    pub coverage_umap: f64,
    pub coverage_acoustid: f64,

    pub bpm_values: Vec<f64>,
    pub duration_values: Vec<f64>,
    pub loudness_values: Vec<f64>,
    pub key_distribution: Vec<LabelCount>,
    pub genre_distribution: Vec<LabelCount>,
    pub instrument_distribution: Vec<LabelCount>,
}

// ── Pure Rust numeric helpers ────────────────────────────────────────────────

fn mean(vals: &[f64]) -> Option<f64> {
    if vals.is_empty() { return None; }
    Some(vals.iter().sum::<f64>() / vals.len() as f64)
}

fn stddev(vals: &[f64]) -> Option<f64> {
    if vals.len() < 2 { return None; }
    let m = vals.iter().sum::<f64>() / vals.len() as f64;
    let var = vals.iter().map(|x| (x - m).powi(2)).sum::<f64>() / vals.len() as f64;
    Some(var.sqrt())
}

// ── SQL helpers ──────────────────────────────────────────────────────────────

/// Returns a WHERE clause that always ends with a condition, so callers can safely
/// append `AND ...` without worrying about generating a bare `WHERE AND`.
fn build_where(track_ids: &Option<Vec<i64>>) -> String {
    match track_ids {
        None => "WHERE 1=1".to_string(),
        Some(ids) if ids.is_empty() => "WHERE 1=0".to_string(),
        Some(ids) => {
            let list: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            format!("WHERE id IN ({})", list.join(","))
        }
    }
}

fn fetch_f64s(conn: &Connection, sql: &str) -> Result<Vec<f64>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows: Vec<f64> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn fetch_label_counts(conn: &Connection, sql: &str) -> Result<Vec<LabelCount>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows: Vec<LabelCount> = stmt
        .query_map([], |r| Ok(LabelCount { label: r.get(0)?, count: r.get(1)? }))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn fetch_strings(conn: &Connection, sql: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

// ── Core computation ─────────────────────────────────────────────────────────

fn compute_stats(conn: &Connection, track_ids: &Option<Vec<i64>>) -> Result<TrackSetStats, String> {
    let wc = build_where(track_ids);

    // Main scalar aggregates (no math functions needed)
    let sql_main = format!(
        "SELECT
            COUNT(*),
            COALESCE(SUM(CAST(duration_seconds AS REAL)), 0.0),
            AVG(CASE WHEN loudness_lufs IS NOT NULL AND loudness_lufs > -100 THEN loudness_lufs END),
            COALESCE(SUM(CASE WHEN detected_vocal = 'voice'        THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN detected_vocal = 'instrumental' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN detected_vocal IS NULL          THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN key IS NOT NULL                 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN mood_happy IS NOT NULL          THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN description IS NOT NULL         THEN 1 ELSE 0 END), 0),
            AVG(mood_happy), AVG(mood_sad), AVG(mood_aggressive),
            AVG(mood_relaxed), AVG(mood_party), AVG(mood_acoustic), AVG(mood_electronic),
            COALESCE(SUM(CASE WHEN scale = 'major'                 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN scale = 'minor'                 THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN acoustid_status = 'done'        THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN description IS NOT NULL AND description != '' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ai_instruments IS NOT NULL AND ai_instruments != '' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ai_mood IS NOT NULL AND ai_mood != '' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN ai_genre IS NOT NULL AND ai_genre != '' THEN 1 ELSE 0 END), 0)
        FROM tracks {wc}"
    );

    let (
        track_count, total_duration, avg_loudness,
        vocal_count, instrumental_count, unknown_vocal_count,
        essentia_count, mood_count, qwen_count,
        avg_happy, avg_sad, avg_aggressive, avg_relaxed, avg_party, avg_acoustic, avg_electronic,
        major_count, minor_count,
        acoustid_done_count,
        qwen_desc_count, qwen_instr_count, qwen_mood_count, qwen_genre_count,
    ) = conn.query_row(&sql_main, [], |r| Ok((
        r.get::<_, i64>(0)?,
        r.get::<_, f64>(1)?,
        r.get::<_, Option<f64>>(2)?,
        r.get::<_, i64>(3)?,
        r.get::<_, i64>(4)?,
        r.get::<_, i64>(5)?,
        r.get::<_, i64>(6)?,
        r.get::<_, i64>(7)?,
        r.get::<_, i64>(8)?,
        r.get::<_, Option<f64>>(9)?,
        r.get::<_, Option<f64>>(10)?,
        r.get::<_, Option<f64>>(11)?,
        r.get::<_, Option<f64>>(12)?,
        r.get::<_, Option<f64>>(13)?,
        r.get::<_, Option<f64>>(14)?,
        r.get::<_, Option<f64>>(15)?,
        r.get::<_, i64>(16)?,
        r.get::<_, i64>(17)?,
        r.get::<_, i64>(18)?,
        r.get::<_, i64>(19)?,
        r.get::<_, i64>(20)?,
        r.get::<_, i64>(21)?,
        r.get::<_, i64>(22)?,
    ))).map_err(|e| e.to_string())?;

    let pct = |n: i64| if track_count > 0 { n as f64 / track_count as f64 * 100.0 } else { 0.0 };

    // CLAP coverage
    let clap_sql = match track_ids {
        None => "SELECT COUNT(*) FROM audio_embeddings".to_string(),
        Some(ids) if ids.is_empty() => "SELECT 0".to_string(),
        Some(ids) => {
            let list: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            format!("SELECT COUNT(*) FROM audio_embeddings WHERE track_id IN ({})", list.join(","))
        }
    };
    let clap_count: i64 = conn.query_row(&clap_sql, [], |r| r.get(0)).unwrap_or(0);

    // UMAP coverage
    let umap_sql = match track_ids {
        None => "SELECT COUNT(*) FROM track_coords".to_string(),
        Some(ids) if ids.is_empty() => "SELECT 0".to_string(),
        Some(ids) => {
            let list: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
            format!("SELECT COUNT(*) FROM track_coords WHERE track_id IN ({})", list.join(","))
        }
    };
    let umap_count: i64 = conn.query_row(&umap_sql, [], |r| r.get(0)).unwrap_or(0);

    // Most common key
    let most_common_key: Option<String> = conn.query_row(
        &format!(
            "SELECT key || ' ' || scale, COUNT(*) c FROM tracks {wc}
             AND key IS NOT NULL AND scale IS NOT NULL
             GROUP BY key, scale ORDER BY c DESC LIMIT 1"
        ),
        [], |r| r.get(0),
    ).ok();

    // Key variety
    let distinct_keys: i64 = conn.query_row(
        &format!("SELECT COUNT(DISTINCT key || '|' || scale) FROM tracks {wc} AND key IS NOT NULL"),
        [], |r| r.get(0),
    ).unwrap_or(0);

    // BPM values — used for mean/stddev; raw values returned for frontend histogram
    let bpm_values = fetch_f64s(conn, &format!(
        "SELECT bpm FROM tracks {wc} AND bpm IS NOT NULL AND bpm > 0"
    ))?;
    let avg_bpm = mean(&bpm_values);
    let bpm_stddev = stddev(&bpm_values);

    let duration_values = fetch_f64s(conn, &format!(
        "SELECT CAST(duration_seconds AS REAL) / 60.0 FROM tracks {wc} AND duration_seconds > 0"
    ))?;

    let loudness_values = fetch_f64s(conn, &format!(
        "SELECT loudness_lufs FROM tracks {wc} AND loudness_lufs IS NOT NULL AND loudness_lufs > -100"
    ))?;

    // Key distribution
    let key_distribution = fetch_label_counts(conn, &format!(
        "SELECT key, COUNT(*) FROM tracks {wc}
         AND key IS NOT NULL GROUP BY key ORDER BY COUNT(*) DESC"
    ))?;

    // Genre distribution
    let genre_distribution = fetch_label_counts(conn, &format!(
        "SELECT detected_genre, COUNT(*) FROM tracks {wc}
         AND detected_genre IS NOT NULL
         GROUP BY detected_genre ORDER BY COUNT(*) DESC LIMIT 20"
    ))?;

    // Instrument distribution — parsed from comma-separated ai_instruments
    let raw_instruments = fetch_strings(conn, &format!(
        "SELECT ai_instruments FROM tracks {wc}
         AND ai_instruments IS NOT NULL AND ai_instruments != ''"
    ))?;
    let mut instr_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for line in &raw_instruments {
        for part in line.split(',') {
            let inst = part.trim().to_lowercase();
            if !inst.is_empty() {
                *instr_counts.entry(inst).or_insert(0) += 1;
            }
        }
    }
    let mut instrument_distribution: Vec<LabelCount> = instr_counts
        .into_iter()
        .map(|(label, count)| LabelCount { label, count })
        .collect();
    instrument_distribution.sort_by(|a, b| b.count.cmp(&a.count));
    instrument_distribution.truncate(15);

    Ok(TrackSetStats {
        track_count,
        total_duration_seconds: total_duration,
        avg_bpm,
        bpm_stddev,
        most_common_key,
        key_variety: distinct_keys as f64 / 24.0,
        pct_vocals: pct(vocal_count),
        pct_analysed: pct(essentia_count),
        avg_loudness_lufs: avg_loudness,

        avg_mood_happy: avg_happy,
        avg_mood_sad: avg_sad,
        avg_mood_aggressive: avg_aggressive,
        avg_mood_relaxed: avg_relaxed,
        avg_mood_party: avg_party,
        avg_mood_acoustic: avg_acoustic,
        avg_mood_electronic: avg_electronic,

        major_count,
        minor_count,

        vocal_count,
        instrumental_count,
        unknown_vocal_count,

        coverage_essentia: pct(essentia_count),
        coverage_mood: pct(mood_count),
        coverage_qwen: pct(qwen_count),
        coverage_qwen_description: pct(qwen_desc_count),
        coverage_qwen_instruments: pct(qwen_instr_count),
        coverage_qwen_mood: pct(qwen_mood_count),
        coverage_qwen_genre: pct(qwen_genre_count),
        coverage_clap: pct(clap_count),
        coverage_umap: pct(umap_count),
        coverage_acoustid: pct(acoustid_done_count),

        bpm_values,
        duration_values,
        loudness_values,
        key_distribution,
        genre_distribution,
        instrument_distribution,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    // ── Pure numeric helpers ─────────────────────────────────────────────────

    #[test]
    fn test_mean_empty() {
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn test_mean_single() {
        assert_eq!(mean(&[5.0]), Some(5.0));
    }

    #[test]
    fn test_mean_values() {
        let v = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((mean(&v).unwrap() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_stddev_insufficient() {
        assert_eq!(stddev(&[]), None);
        assert_eq!(stddev(&[42.0]), None);
    }

    #[test]
    fn test_stddev_known() {
        // Population stddev of [2, 4, 4, 4, 5, 5, 7, 9] = 2.0
        let v = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((stddev(&v).unwrap() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_stddev_uniform() {
        let v = vec![7.0; 10];
        assert!((stddev(&v).unwrap() - 0.0).abs() < 1e-10);
    }

    // ── build_where ──────────────────────────────────────────────────────────

    #[test]
    fn test_build_where_none() {
        assert_eq!(build_where(&None), "WHERE 1=1");
    }

    #[test]
    fn test_build_where_empty_ids() {
        assert_eq!(build_where(&Some(vec![])), "WHERE 1=0");
    }

    #[test]
    fn test_build_where_ids() {
        let result = build_where(&Some(vec![1, 2, 3]));
        assert_eq!(result, "WHERE id IN (1,2,3)");
    }

    // ── compute_stats integration ────────────────────────────────────────────

    fn seed_track(conn: &Connection, id: i64, dir_id: i64, bpm: Option<f64>, key: Option<&str>,
                  scale: Option<&str>, loudness: Option<f64>, vocal: Option<&str>,
                  genre: Option<&str>, mood_happy: Option<f64>, instruments: Option<&str>) {
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes,
                last_modified, duration_seconds, bpm, key, scale, loudness_lufs,
                detected_vocal, detected_genre, mood_happy, ai_instruments)
             VALUES (?1,?2,?3,?4,100,100,180,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                id, dir_id,
                format!("/music/track{id}.mp3"), format!("track{id}.mp3"),
                bpm, key, scale, loudness, vocal, genre, mood_happy, instruments
            ],
        ).unwrap();
    }

    fn setup_db_with_tracks() -> Connection {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'Test', '/music')",
            [],
        ).unwrap();

        seed_track(&conn, 1, 1, Some(120.0), Some("C"), Some("major"), Some(-14.0), Some("voice"),   Some("Electronic"), Some(0.8), Some("drums, synth"));
        seed_track(&conn, 2, 1, Some(140.0), Some("G"), Some("minor"), Some(-18.0), Some("instrumental"), Some("Electronic"), Some(0.2), Some("bass, synth"));
        seed_track(&conn, 3, 1, Some(90.0),  Some("C"), Some("major"), Some(-12.0), Some("voice"),   Some("Hip-Hop"),    Some(0.6), Some("drums"));
        seed_track(&conn, 4, 1, None,         None,      None,          None,         None,           None,              None,      None);

        conn
    }

    #[test]
    fn test_full_library_stats() {
        let conn = setup_db_with_tracks();
        let stats = compute_stats(&conn, &None).unwrap();

        assert_eq!(stats.track_count, 4);
        // BPM mean over 3 tracks with BPM (120+140+90)/3
        assert!((stats.avg_bpm.unwrap() - 116.666_666).abs() < 0.01);
        // Loudness avg over 3 tracks with finite loudness
        assert!((stats.avg_loudness_lufs.unwrap() - (-14.666_666)).abs() < 0.01);
        // key_distribution: C should appear twice
        let c_count = stats.key_distribution.iter().find(|k| k.label == "C").map(|k| k.count).unwrap_or(0);
        assert_eq!(c_count, 2);
        // genre: Electronic x2, Hip-Hop x1
        let elec = stats.genre_distribution.iter().find(|g| g.label == "Electronic").unwrap();
        assert_eq!(elec.count, 2);
        // vocal breakdown
        assert_eq!(stats.vocal_count, 2);
        assert_eq!(stats.instrumental_count, 1);
        assert_eq!(stats.unknown_vocal_count, 1);
        // major/minor
        assert_eq!(stats.major_count, 2);
        assert_eq!(stats.minor_count, 1);
        // coverage: 3/4 have key → 75%
        assert!((stats.coverage_essentia - 75.0).abs() < 0.01);
        // BPM values returned for histogram
        assert_eq!(stats.bpm_values.len(), 3);
        // instrument parsing: drums appears in track 1 and 3
        let drums = stats.instrument_distribution.iter().find(|i| i.label == "drums").unwrap();
        assert_eq!(drums.count, 2);
    }

    #[test]
    fn test_filtered_stats() {
        let conn = setup_db_with_tracks();
        // Query only tracks 1 and 2
        let stats = compute_stats(&conn, &Some(vec![1, 2])).unwrap();

        assert_eq!(stats.track_count, 2);
        assert!((stats.avg_bpm.unwrap() - 130.0).abs() < 0.01);
        assert_eq!(stats.bpm_values.len(), 2);
    }

    #[test]
    fn test_empty_filter_returns_zero_counts() {
        let conn = setup_db_with_tracks();
        let stats = compute_stats(&conn, &Some(vec![])).unwrap();

        assert_eq!(stats.track_count, 0);
        assert_eq!(stats.avg_bpm, None);
        assert!(stats.bpm_values.is_empty());
    }

    #[test]
    fn test_loudness_neg_inf_excluded() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', '/t')", [],
        ).unwrap();
        // One track with a finite LUFS, one with -Inf stored as a very negative float
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds, loudness_lufs)
             VALUES (1, 1, '/t/a.mp3', 'a.mp3', 100, 100, 60, -14.0),
                    (2, 1, '/t/b.mp3', 'b.mp3', 100, 100, 60, -999999.0)",
            [],
        ).unwrap();
        let stats = compute_stats(&conn, &None).unwrap();
        // -999999 should be excluded (< -100 threshold), average should be just -14
        assert!((stats.avg_loudness_lufs.unwrap() - (-14.0)).abs() < 0.01);
        assert_eq!(stats.loudness_values.len(), 1);
    }

    #[test]
    fn test_pct_analysed_and_coverage() {
        let conn = setup_db_with_tracks();
        let stats = compute_stats(&conn, &None).unwrap();
        // 3 of 4 tracks have key → 75%
        assert!((stats.pct_analysed - 75.0).abs() < 0.01);
        assert!((stats.coverage_essentia - 75.0).abs() < 0.01);
        // 0 CLAP embeddings seeded
        assert!((stats.coverage_clap - 0.0).abs() < 0.01);
    }
}

#[tauri::command]
pub fn get_track_stats(
    track_ids: Option<Vec<i64>>,
    conn: tauri::State<'_, Mutex<rusqlite::Connection>>,
) -> Result<TrackSetStats, String> {
    let conn = conn.lock().map_err(|e| e.to_string())?;
    compute_stats(&conn, &track_ids)
}
