use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StructureClusterInfo {
    pub id: i64,
    pub label: String,
    pub regex: String,
    pub track_count: i64,
}

/// @concept SAX
/// @skill add-ipc-command
/// Tauri IPC commands for retrieving SAX structural clusters and track-count distribution.
#[tauri::command]
pub fn get_structure_clusters(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<StructureClusterInfo>, String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, label, regex, track_count
         FROM structure_clusters
         ORDER BY track_count DESC",
    ).map_err(|e| e.to_string())?;

    let clusters = stmt.query_map([], |row| {
        Ok(StructureClusterInfo {
            id:          row.get(0)?,
            label:       row.get(1)?,
            regex:       row.get(2)?,
            track_count: row.get(3)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(clusters)
}
