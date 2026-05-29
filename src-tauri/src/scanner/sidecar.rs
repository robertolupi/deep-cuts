use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const SUFFIX: &str = ".dc.json";

/// ML-derived fields persisted alongside each audio file.
/// When adding a new analysis pass, add its output fields here and update:
///   - save()    — add to the SELECT query and struct construction
///   - restore() — add to the UPDATE statement
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SidecarMlMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bpm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_strength: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loudness_lufs: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loudness_range: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waveform_data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SidecarData {
    pub version: i32,
    #[serde(default)]
    pub ml_metadata: SidecarMlMetadata,
}

pub fn path_for(track_path: &str) -> String {
    format!("{}{}", track_path, SUFFIX)
}

/// Reads and parses the sidecar file for `track_path`. Returns None on any error
/// (missing file, invalid JSON, wrong shape) so callers can treat absence as a no-op.
pub fn load(track_path: &str) -> Option<SidecarData> {
    let content = std::fs::read_to_string(path_for(track_path)).ok()?;
    serde_json::from_str(&content).ok()
}

/// Writes a sidecar file next to `track_id`'s audio file, persisting its ML metadata.
pub fn save(conn: &Connection, track_id: i64) -> Result<(), Box<dyn std::error::Error>> {
    let path: String = conn.query_row(
        "SELECT path FROM tracks WHERE id = ?1",
        [track_id],
        |row| row.get(0),
    )?;

    let ml_metadata: SidecarMlMetadata = conn.query_row(
        "SELECT bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data
         FROM tracks WHERE id = ?1",
        [track_id],
        |row| Ok(SidecarMlMetadata {
            bpm: row.get(0)?,
            key: row.get(1)?,
            scale: row.get(2)?,
            key_strength: row.get(3)?,
            loudness_lufs: row.get(4)?,
            loudness_range: row.get(5)?,
            waveform_data: row.get(6)?,
        }),
    )?;

    let sidecar = SidecarData { version: 1, ml_metadata };
    let json = serde_json::to_string_pretty(&sidecar)?;
    std::fs::write(path_for(&path), json)?;
    Ok(())
}

/// Reads the sidecar for `track_path` and restores its ML fields into the tracks table.
/// Currently a no-op because SidecarMlMetadata has no fields yet; the scaffold is in place
/// so that adding a field to SidecarMlMetadata + the UPDATE below is all that's needed.
pub fn restore(
    conn: &Connection,
    track_id: i64,
    track_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(sidecar) = load(track_path) else {
        return Ok(());
    };

    let m = &sidecar.ml_metadata;
    conn.execute(
        "UPDATE tracks SET
            bpm = COALESCE(?1, bpm),
            key = COALESCE(?2, key),
            scale = COALESCE(?3, scale),
            key_strength = COALESCE(?4, key_strength),
            loudness_lufs = COALESCE(?5, loudness_lufs),
            loudness_range = COALESCE(?6, loudness_range),
            waveform_data = COALESCE(?7, waveform_data)
         WHERE id = ?8",
        rusqlite::params![
            m.bpm, m.key, m.scale, m.key_strength,
            m.loudness_lufs, m.loudness_range, m.waveform_data,
            track_id,
        ],
    )?;
    Ok(())
}

/// Writes sidecar files for every track in the database. Returns the count of files written.
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
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 128.5, 'G', 'minor', 0.87, -14.2, 6.1, '[0.1,0.2]')",
            [&track_path],
        )
        .unwrap();

        save(&conn, 1).unwrap();
        let loaded = load(&track_path).expect("sidecar should be loadable after save");
        assert_eq!(loaded.version, 1);
        let m = &loaded.ml_metadata;
        assert_eq!(m.bpm, Some(128.5));
        assert_eq!(m.key.as_deref(), Some("G"));
        assert_eq!(m.scale.as_deref(), Some("minor"));
        assert_eq!(m.key_strength, Some(0.87));
        assert_eq!(m.loudness_lufs, Some(-14.2));
        assert_eq!(m.loudness_range, Some(6.1));
        assert_eq!(m.waveform_data.as_deref(), Some("[0.1,0.2]"));

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
                                 bpm, key, scale, key_strength, loudness_lufs, loudness_range, waveform_data) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 180, 120.0, 'C', 'major', 0.9, -12.0, 5.0, '[0.5]')",
            [&track_path],
        )
        .unwrap();

        // Save to disk, then clear the DB row, then restore from disk
        save(&conn, 1).unwrap();
        conn.execute(
            "UPDATE tracks SET bpm = NULL, key = NULL, scale = NULL, key_strength = NULL,
             loudness_lufs = NULL, loudness_range = NULL, waveform_data = NULL WHERE id = 1",
            [],
        ).unwrap();
        restore(&conn, 1, &track_path).unwrap();

        let (bpm, key, scale): (Option<f64>, Option<String>, Option<String>) = conn.query_row(
            "SELECT bpm, key, scale FROM tracks WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        ).unwrap();
        assert_eq!(bpm, Some(120.0));
        assert_eq!(key.as_deref(), Some("C"));
        assert_eq!(scale.as_deref(), Some("major"));

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

        // No sidecar file exists — restore must return Ok without panicking.
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
}
