/// Debug-only commands — compiled out of release builds entirely.
/// Exposes raw DB rows for the dev inspector UI.
use std::sync::Mutex;
use rusqlite::{Connection, types::ValueRef};
use serde_json::{json, Value, Map};
use tauri::State;

/// Convert a rusqlite Row into a serde_json::Value object, reading all columns
/// dynamically so the command never needs updating when migrations add columns.
fn row_to_json(row: &rusqlite::Row, col_count: usize) -> Value {
    let mut map = Map::new();
    for i in 0..col_count {
        let name = row.as_ref().column_name(i).unwrap_or("?").to_string();
        let val: Value = match row.get_ref(i).unwrap_or(ValueRef::Null) {
            ValueRef::Null       => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(f)    => json!(f),
            ValueRef::Text(t)    => Value::String(String::from_utf8_lossy(t).into_owned()),
            ValueRef::Blob(b)    => json!({ "blob_bytes": b.len() }),
        };
        map.insert(name, val);
    }
    Value::Object(map)
}

fn query_all(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Vec<Value> {
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => { log::warn!("[debug] prepare failed '{}': {}", sql, e); return vec![]; }
    };
    let col_count = stmt.column_count();
    let result: Vec<Value> = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| Ok(row_to_json(row, col_count))) {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(e)   => { log::warn!("[debug] execute failed: {}", e); vec![] }
    };
    result
}

fn query_one(conn: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Value {
    query_all(conn, sql, params).into_iter().next().unwrap_or(Value::Null)
}

#[tauri::command]
pub fn debug_track_raw(
    track_id: i64,
    db: State<'_, Mutex<Connection>>,
) -> Result<Value, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let track = query_one(&conn, "SELECT * FROM tracks WHERE id = ?1", &[&track_id]);

    let path: Option<String> = conn.query_row(
        "SELECT path FROM tracks WHERE id = ?1", [track_id], |r| r.get(0),
    ).ok();

    let passes = query_all(&conn,
        "SELECT * FROM track_passes WHERE track_id = ?1 ORDER BY pass_name",
        &[&track_id]);

    let coords = query_one(&conn,
        "SELECT * FROM track_coords WHERE track_id = ?1",
        &[&track_id]);

    let tags = query_all(&conn,
        "SELECT t.name, tt.score, tt.discard \
         FROM track_tags tt JOIN tags t ON tt.tag_id = t.id \
         WHERE tt.track_id = ?1 ORDER BY tt.score DESC NULLS LAST",
        &[&track_id]);

    let suppressions: Vec<String> = if let Some(ref p) = path {
        let mut stmt = conn.prepare(
            "SELECT tag_name FROM user_suppressed_tags WHERE track_path = ?1"
        ).unwrap_or_else(|_| conn.prepare("SELECT 1 WHERE 0").unwrap());
        stmt.query_map([p], |r| r.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };

    let chat_sessions = query_all(&conn,
        "SELECT id, title, created_at FROM chat_sessions WHERE track_id = ?1 ORDER BY created_at DESC",
        &[&track_id]);

    Ok(json!({
        "track":         track,
        "passes":        passes,
        "coords":        coords,
        "tags":          tags,
        "suppressions":  suppressions,
        "chat_sessions": chat_sessions,
    }))
}
