use rusqlite::Connection;
use serde::{Deserialize, Serialize};

pub const SUFFIX: &str = ".dc.json";

/// ML-derived fields persisted alongside each audio file.
/// When adding a new analysis pass, add its output fields here and update:
///   - save()    — add to the SELECT query and struct construction
///   - restore() — add to the UPDATE statement
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SidecarMlMetadata {}

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

    // SELECT ml columns here as they are added (see SidecarMlMetadata above).
    let ml_metadata = SidecarMlMetadata {};

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
    let Some(_sidecar) = load(track_path) else {
        return Ok(());
    };

    // UPDATE tracks SET <new_col> = ?1 WHERE id = ?2
    // Add columns here as they are added to SidecarMlMetadata.
    let _ = (conn, track_id);
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
            "INSERT INTO tracks (id, watched_directory_id, path, filename, size_bytes, last_modified, duration_seconds) \
             VALUES (1, 1, ?1, 'song.mp3', 5, 0, 0)",
            [&track_path],
        )
        .unwrap();

        save(&conn, 1).unwrap();
        let loaded = load(&track_path).expect("sidecar should be loadable after save");
        assert_eq!(loaded.version, 1);

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
