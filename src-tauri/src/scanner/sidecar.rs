use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SUFFIX: &str = ".dc.json";

/// Current algorithm/model version for each analysis pass.
pub mod pass_version {
    pub const AUDIO_ANALYSIS: u32 = 1;
    pub const CLAP: u32 = 2;
    pub const ESSENTIA: u32 = 1;
    pub const BPM_CORRECTION: u32 = 1;
    pub const BPM_REFINEMENT: u32 = 1;
    pub const QWEN: u32 = 3;
    pub const DESCRIPTION_EMBED: u32 = 1;
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SidecarData {
    /// Schema version for the sidecar format itself.
    pub version: i32,
    /// Per-pass algorithm/model version at the time this sidecar was written.
    #[serde(default)]
    pub pass_versions: HashMap<String, u32>,
    /// Map of pass_name -> last_run_at timestamp string ("YYYY-MM-DD HH:MM:SS")
    #[serde(default)]
    pub pass_run_times: HashMap<String, String>,
    /// Flattened dynamic ML fields driven dynamically by PASS_REGISTRY
    #[serde(flatten)]
    pub ml_metadata: HashMap<String, serde_json::Value>,
}

pub fn path_for(track_path: &str) -> String {
    format!("{}{}", track_path, SUFFIX)
}

/// Reads and parses the sidecar file. Returns None on any error.
pub fn load(track_path: &str) -> Option<SidecarData> {
    let content = std::fs::read_to_string(path_for(track_path)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Writes a sidecar file next to `track_id`'s audio file.
pub fn save(conn: &Connection, track_id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let path: String =
        conn.query_row("SELECT path FROM tracks WHERE id = ?1", [track_id], |row| {
            row.get(0)
        })?;

    let mut ml_metadata = HashMap::new();

    // Dynamically serialize owned columns and tables from PASS_REGISTRY specs
    for spec in crate::analysis::PASS_REGISTRY {
        // A. Owned columns
        if !spec.owned_columns.is_empty() {
            let cols = spec.owned_columns.join(", ");
            let query = format!("SELECT {} FROM tracks WHERE id = ?1", cols);
            let mut stmt = conn.prepare(&query)?;
            let row = stmt.query_row([track_id], |r| {
                let mut vals = Vec::new();
                for i in 0..spec.owned_columns.len() {
                    let val: rusqlite::types::Value = r.get(i)?;
                    vals.push(val);
                }
                Ok(vals)
            })?;

            for (col, val) in spec.owned_columns.iter().zip(row.into_iter()) {
                let json_val = match val {
                    rusqlite::types::Value::Null => serde_json::Value::Null,
                    rusqlite::types::Value::Integer(i) => serde_json::Value::Number(i.into()),
                    rusqlite::types::Value::Real(f) => {
                        if let Some(n) = serde_json::Number::from_f64(f) {
                            serde_json::Value::Number(n)
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
                    rusqlite::types::Value::Blob(_) => serde_json::Value::Null,
                };
                if !json_val.is_null() {
                    ml_metadata.insert(col.to_string(), json_val);
                }
            }
        }

        // B. Owned tables (blobs deserialised to f32 vectors)
        for table in spec.owned_tables {
            let blob_opt: Option<Vec<u8>> = conn
                .query_row(
                    &format!("SELECT embedding FROM {} WHERE track_id = ?1", table),
                    [track_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(blob) = blob_opt {
                let floats: Vec<f32> = blob
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                if !floats.is_empty() {
                    let json_arr = serde_json::Value::Array(
                        floats.into_iter().map(|f| serde_json::json!(f)).collect(),
                    );
                    let key = if *table == "audio_embeddings" {
                        "clap_embedding"
                    } else if *table == "description_embeddings" {
                        "description_embedding"
                    } else {
                        &format!("{}_embedding", table)
                    };
                    ml_metadata.insert(key.to_string(), json_arr);
                }
            }
        }
    }

    // Record the pass_version and last_run_at for every DONE pass directly from the DB column
    let mut pass_versions: HashMap<String, u32> = HashMap::new();
    let mut pass_run_times: HashMap<String, String> = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT pass_name, pass_version, last_run_at FROM track_passes WHERE track_id = ?1 AND status = 2",
    )?;
    let rows: Vec<(String, u32, Option<String>)> = stmt
        .query_map([track_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();
    for (pass_name, version, last_run) in rows {
        pass_versions.insert(pass_name.clone(), version);
        if let Some(t) = last_run {
            pass_run_times.insert(pass_name, t);
        }
    }

    let sidecar = SidecarData {
        version: 1,
        pass_versions,
        pass_run_times,
        ml_metadata,
    };
    let json = serde_json::to_string_pretty(&sidecar)?;
    std::fs::write(path_for(&path), json)?;
    Ok(())
}

fn sidecar_pass_version(sidecar: &SidecarData, pass: &str) -> u32 {
    *sidecar.pass_versions.get(pass).unwrap_or(&0)
}

fn parse_sqlite_timestamp(ts: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc().timestamp())
}

/// Reads the sidecar and restores its ML fields into the database.
pub fn restore(
    conn: &Connection,
    track_id: i64,
    track_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(sidecar) = load(track_path) else {
        return Ok(());
    };

    let track_last_modified: i64 = conn.query_row(
        "SELECT last_modified FROM tracks WHERE id = ?1",
        [track_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let m = &sidecar.ml_metadata;

    // Dynamically restore fields driven by PASS_REGISTRY specs
    for spec in crate::analysis::PASS_REGISTRY {
        let sidecar_version = sidecar_pass_version(&sidecar, spec.name);

        let sidecar_run_time = sidecar.pass_run_times.get(spec.name).cloned();
        let is_up_to_date = if let Some(ref run_time_str) = sidecar_run_time {
            if let Some(run_time_epoch) = parse_sqlite_timestamp(run_time_str) {
                track_last_modified <= run_time_epoch
            } else {
                false
            }
        } else {
            // No timestamp in sidecar — treat as stale so all passes are reset and re-run.
            false
        };

        if sidecar_version >= spec.version && is_up_to_date {
            // A. Restore columns
            if !spec.owned_columns.is_empty() {
                let mut set_clauses = Vec::new();
                let mut params = Vec::new();
                for (i, col) in spec.owned_columns.iter().enumerate() {
                    set_clauses.push(format!("{} = COALESCE(?{}, {})", col, i + 1, col));
                    let val = m.get(*col).cloned().unwrap_or(serde_json::Value::Null);
                    params.push(val);
                }
                let query = format!(
                    "UPDATE tracks SET {} WHERE id = ?{}",
                    set_clauses.join(", "),
                    spec.owned_columns.len() + 1
                );

                let mut stmt = conn.prepare(&query)?;
                let rusqlite_vals: Vec<rusqlite::types::Value> = params
                    .into_iter()
                    .map(|v| match v {
                        serde_json::Value::Null => rusqlite::types::Value::Null,
                        serde_json::Value::Bool(b) => rusqlite::types::Value::Integer(if b { 1 } else { 0 }),
                        serde_json::Value::Number(num) => {
                            if let Some(i) = num.as_i64() {
                                rusqlite::types::Value::Integer(i)
                            } else if let Some(f) = num.as_f64() {
                                rusqlite::types::Value::Real(f)
                            } else {
                                rusqlite::types::Value::Null
                            }
                        }
                        serde_json::Value::String(s) => rusqlite::types::Value::Text(s),
                        _ => rusqlite::types::Value::Null,
                    })
                    .collect();

                let mut sql_params: Vec<&dyn rusqlite::ToSql> = Vec::new();
                for val in &rusqlite_vals {
                    sql_params.push(val);
                }
                sql_params.push(&track_id);

                stmt.execute(rusqlite::params_from_iter(sql_params))?;
            }

            // B. Restore tables (float arrays serialised back to byte blobs)
            for table in spec.owned_tables {
                let key = if *table == "audio_embeddings" {
                    "clap_embedding"
                } else if *table == "description_embeddings" {
                    "description_embedding"
                } else {
                    &format!("{}_embedding", table)
                };

                if let Some(serde_json::Value::Array(arr)) = m.get(key) {
                    let floats_res: Result<Vec<f32>, _> = arr
                        .iter()
                        .map(|v| v.as_f64().map(|f| f as f32).ok_or("Invalid float"))
                        .collect();
                    if let Ok(floats) = floats_res {
                        if !floats.is_empty() {
                            let blob: Vec<u8> = floats.iter().flat_map(|&f| f.to_le_bytes()).collect();
                            conn.execute(
                                &format!("INSERT OR REPLACE INTO {} (track_id, embedding) VALUES (?1, ?2)", table),
                                rusqlite::params![track_id, blob],
                            )?;
                        }
                    }
                }
            }

            // C. Update track pass status to DONE, writing the last_run_at back to DB
            let db_last_run = sidecar_run_time.clone().unwrap_or_else(|| {
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
            });

            conn.execute(
                "UPDATE track_passes SET status = 2, pass_version = ?1, last_run_at = ?2
                 WHERE track_id = ?3 AND pass_name = ?4",
                rusqlite::params![spec.version, db_last_run, track_id, spec.name],
            )?;
        }
    }

    Ok(())
}

/// Writes sidecar files for every track in the database.
pub fn export_all(conn: &Connection) -> Result<usize, Box<dyn std::error::Error>> {
    let mut stmt = conn.prepare("SELECT id FROM tracks")?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut count = 0;
    for id in ids {
        if save(conn, id).is_ok() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;
    use std::path::PathBuf;

    fn temp_track(label: &str) -> (PathBuf, String) {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        dir.push(format!("target/sidecar_test_{}", label));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("song.mp3");
        std::fs::write(&path, b"dummy").unwrap();
        let path_str = path.to_string_lossy().into_owned();
        (dir, path_str)
    }

    fn cleanup(dir: &PathBuf, track_path: &str) {
        let _ = std::fs::remove_file(track_path);
        let _ = std::fs::remove_file(path_for(track_path));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_save_creates_dc_json_file() {
        let (dir, track_path) = temp_track("save");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        save(&conn, 1).unwrap();

        let sidecar_file = path_for(&track_path);
        assert!(
            std::path::Path::new(&sidecar_file).exists(),
            "expected .dc.json file at {}",
            sidecar_file
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let (dir, track_path) = temp_track("roundtrip");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds,
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data,
                                 silence_regions, has_long_silence) \
              VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 128.5, 'G', 'minor', 0.87, -14.2, 6.1, '[0.1,0.2]',
                     '[[12.0,15.5]]', 0)",
            [&track_path],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 2, ?1)",
            rusqlite::params![pass_version::AUDIO_ANALYSIS],
        )
        .unwrap();

        save(&conn, 1).unwrap();
        let loaded = load(&track_path).expect("sidecar should be loadable after save");
        assert_eq!(loaded.version, 1);
        let m = &loaded.ml_metadata;
        assert_eq!(m.get("bpm").and_then(|v| v.as_f64()), Some(128.5));
        assert_eq!(m.get("key").and_then(|v| v.as_str()), Some("G"));
        assert_eq!(m.get("scale").and_then(|v| v.as_str()), Some("minor"));
        assert_eq!(m.get("key_strength").and_then(|v| v.as_f64()), Some(0.87));
        assert_eq!(m.get("loudness_lufs").and_then(|v| v.as_f64()), Some(-14.2));
        assert_eq!(m.get("loudness_range").and_then(|v| v.as_f64()), Some(6.1));
        assert_eq!(m.get("waveform_data").and_then(|v| v.as_str()), Some("[0.1,0.2]"));
        assert_eq!(m.get("silence_regions").and_then(|v| v.as_str()), Some("[[12.0,15.5]]"));
        assert_eq!(m.get("has_long_silence").and_then(|v| v.as_i64()), Some(0));

        assert_eq!(
            loaded.pass_versions.get("audio_analysis").copied(),
            Some(pass_version::AUDIO_ANALYSIS)
        );

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_writes_ml_fields_back() {
        let (dir, track_path) = temp_track("restore_fields");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds,
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data,
                                 silence_regions, has_long_silence) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 120.0, 'C', 'major', 0.9, -12.0, 5.0, '[0.5]',
                     '[[8.0,20.5]]', 1)",
            [&track_path],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version, last_run_at) \
             VALUES (1, 'audio_analysis', 10, 2, ?1, '2000-01-01 00:00:01')",
            rusqlite::params![pass_version::AUDIO_ANALYSIS],
        )
        .unwrap();

        save(&conn, 1).unwrap();
        conn.execute(
            "UPDATE tracks SET bpm = NULL, key = NULL, scale = NULL, key_strength = NULL,
             loudness_lufs = NULL, loudness_range = NULL, waveform_data = NULL,
             silence_regions = NULL, has_long_silence = 0 WHERE id = 1",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE track_passes SET status = 0 WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [],
        ).unwrap();
        restore(&conn, 1, &track_path).unwrap();

        let (bpm, key, scale, silence_regions, has_long_silence): (
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<String>,
            i64,
        ) = conn
            .query_row(
                "SELECT bpm, key, scale, silence_regions, has_long_silence FROM tracks WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(bpm, Some(120.0));
        assert_eq!(key.as_deref(), Some("C"));
        assert_eq!(scale.as_deref(), Some("major"));
        assert_eq!(silence_regions.as_deref(), Some("[[8.0,20.5]]"));
        assert_eq!(has_long_silence, 1);

        let (status, version): (i64, u32) = conn.query_row(
            "SELECT status, pass_version FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(status, 2);
        assert_eq!(version, pass_version::AUDIO_ANALYSIS);

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_skips_stale_pass_version() {
        let (dir, track_path) = temp_track("stale_version");

        let stale =
            r#"{"version":1,"pass_versions":{"audio_analysis":0},"bpm":99.0}"#;
        std::fs::write(path_for(&track_path), stale).unwrap();

        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        ).unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 0, 0)",
            [],
        )
        .unwrap();

        restore(&conn, 1, &track_path).unwrap();

        let bpm: Option<f64> = conn
            .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(bpm, None);

        let status: i64 = conn.query_row(
            "SELECT status FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(status, 0);

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_old_sidecar_without_pass_versions() {
        let (dir, track_path) = temp_track("old_sidecar");

        let old_format = r#"{"version":1,"bpm":120.0,"key":"A","scale":"major"}"#;
        std::fs::write(path_for(&track_path), old_format).unwrap();

        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        ).unwrap();

        let result = restore(&conn, 1, &track_path);
        assert!(result.is_ok());

        let bpm: Option<f64> = conn
            .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(bpm, None);

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_corrupt_sidecar_returns_none() {
        let (dir, track_path) = temp_track("corrupt");

        let cases = [
            ("truncated", "{\"version\":1,\"ml_metadata\":{"),
            ("plain_text", "not json"),
            ("empty", ""),
        ];

        for (label, content) in cases {
            std::fs::write(path_for(&track_path), content).unwrap();
            assert!(
                load(&track_path).is_none(),
                "[{label}] expected None for corrupt sidecar"
            );
        }

        let _ = std::fs::remove_file(path_for(&track_path));
        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_is_safe_on_missing_sidecar() {
        let (dir, track_path) = temp_track("no_sidecar");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        let result = restore(&conn, 1, &track_path);
        assert!(result.is_ok());

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_export_all_writes_one_file_per_track() {
        let (dir, track_path) = temp_track("export_all");
        let conn = setup_test_db();

        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        let count = export_all(&conn).unwrap();
        assert_eq!(count, 1);
        assert!(std::path::Path::new(&path_for(&track_path)).exists());

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_skips_when_track_modified_after_run() {
        let (dir, track_path) = temp_track("stale_run");
        let run_time_str = "2026-06-04 10:00:00";
        let run_time_epoch = parse_sqlite_timestamp(run_time_str).unwrap();

        let sidecar_content = format!(
            r#"{{"version":1,"pass_versions":{{"audio_analysis":1}},"pass_run_times":{{"audio_analysis":"{}"}},"bpm":120.0}}"#,
            run_time_str
        );
        std::fs::write(path_for(&track_path), sidecar_content).unwrap();

        // 1. Stale cache (track modified after pass run) -> skip restore
        {
            let conn = setup_test_db();
            conn.execute(
                "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
                [dir.to_string_lossy().as_ref()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
                 VALUES (1, 1, ?1, 'song.mp3', 5, ?2, 0)",
                rusqlite::params![&track_path, run_time_epoch + 10],
            ).unwrap();
            conn.execute(
                "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
                 VALUES (1, 'audio_analysis', 10, 0, 0)",
                [],
            )
            .unwrap();

            restore(&conn, 1, &track_path).unwrap();

            let bpm: Option<f64> = conn
                .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
                .unwrap();
            assert_eq!(bpm, None);
            let status: i64 = conn
                .query_row("SELECT status FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(status, 0);
        }

        // 2. Valid cache (track modified before pass run) -> restore
        {
            let conn = setup_test_db();
            conn.execute(
                "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
                [dir.to_string_lossy().as_ref()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
                 VALUES (1, 1, ?1, 'song.mp3', 5, ?2, 0)",
                rusqlite::params![&track_path, run_time_epoch - 10],
            ).unwrap();
            conn.execute(
                "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
                 VALUES (1, 'audio_analysis', 10, 0, 0)",
                [],
            )
            .unwrap();

            restore(&conn, 1, &track_path).unwrap();

            let bpm: Option<f64> = conn
                .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
                .unwrap();
            assert_eq!(bpm, Some(120.0));
            let status: i64 = conn
                .query_row("SELECT status FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'", [], |r| r.get(0))
                .unwrap();
            assert_eq!(status, 2);
        }

        cleanup(&dir, &track_path);
    }

    #[test]
    fn test_restore_skips_when_sidecar_has_no_pass_run_times() {
        let (dir, track_path) = temp_track("no_run_times");

        // Old-format sidecar with no pass_run_times field at all
        let sidecar_content = r#"{"version":1,"pass_versions":{"audio_analysis":1},"bpm":99.0}"#;
        std::fs::write(path_for(&track_path), sidecar_content).unwrap();

        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO watched_directories (id, name, path) VALUES (1, 'T', ?1)",
            [dir.to_string_lossy().as_ref()],
        ).unwrap();
        conn.execute(
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 1000, 0)",
            [&track_path],
        ).unwrap();
        conn.execute(
            "INSERT INTO track_passes (track_id, pass_name, priority, status, pass_version) \
             VALUES (1, 'audio_analysis', 10, 0, 0)",
            [],
        ).unwrap();

        restore(&conn, 1, &track_path).unwrap();

        // Pass must remain pending and bpm must stay NULL
        let bpm: Option<f64> = conn
            .query_row("SELECT bpm FROM tracks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(bpm, None);
        let status: i64 = conn
            .query_row("SELECT status FROM track_passes WHERE track_id = 1 AND pass_name = 'audio_analysis'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, 0);

        cleanup(&dir, &track_path);
    }
}

