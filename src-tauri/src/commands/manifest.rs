use crate::error::AppError;
use crate::models::ModelManifest;
use rusqlite::Connection;
use semver::Version;
use std::sync::Mutex;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct AppManifestResponse {
    pub manifest: ModelManifest,
    pub update_available: bool,
}

pub fn get_update_settings_impl(conn: &Connection) -> Result<bool, rusqlite::Error> {
    let enabled: String = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'check_for_updates'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "true".to_string());
    
    Ok(enabled == "true")
}

pub fn set_update_settings_impl(conn: &Connection, enabled: bool) -> Result<(), rusqlite::Error> {
    let val = if enabled { "true" } else { "false" };
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('check_for_updates', ?)",
        [val],
    )?;
    Ok(())
}

#[tauri::command]
pub fn get_update_settings(
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<bool, AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    
    get_update_settings_impl(&conn).map_err(AppError::from)
}

#[tauri::command]
pub fn set_update_settings(
    conn_state: tauri::State<'_, Mutex<Connection>>,
    enabled: bool,
) -> Result<(), AppError> {
    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;
    
    set_update_settings_impl(&conn, enabled).map_err(AppError::from)
}

#[tauri::command]
pub fn fetch_app_manifest(
    app: tauri::AppHandle,
    conn_state: tauri::State<'_, Mutex<Connection>>,
) -> Result<AppManifestResponse, AppError> {
    // 1. Check if updates are enabled
    let check_enabled = get_update_settings(conn_state.clone()).unwrap_or(true);
    
    // Fallback parser helper
    let get_fallback = || {
        let manifest = ModelManifest::fallback();
        AppManifestResponse {
            manifest,
            update_available: false,
        }
    };

    if !check_enabled {
        log::info!("[manifest] Update checks disabled by user setting. Returning fallback.");
        return Ok(get_fallback());
    }

    let conn = conn_state
        .lock()
        .map_err(|_| AppError::Config("Database lock poisoned".to_string()))?;

    // 2. Check cache (24 hours = 86400 seconds)
    let cache_info: Option<(String, i64)> = conn
        .query_row(
            "SELECT (SELECT value FROM app_settings WHERE key = 'manifest_cached_json'),
                    (SELECT value FROM app_settings WHERE key = 'manifest_last_fetched')",
            [],
            |row| {
                let json: Option<String> = row.get(0)?;
                let last_fetched: Option<String> = row.get(1)?;
                if let (Some(j), Some(lf)) = (json, last_fetched) {
                    if let Ok(ts) = lf.parse::<i64>() {
                        return Ok(Some((j, ts)));
                    }
                }
                Ok(None)
            },
        )
        .unwrap_or(None);

    let now = chrono::Utc::now().timestamp();
    let mut manifest_json = None;

    if let Some((cached_json, last_ts)) = cache_info {
        if now - last_ts < 86400 {
            log::info!("[manifest] Cache hit (last check was {}s ago).", now - last_ts);
            manifest_json = Some(cached_json);
        }
    }

    // 3. Cache miss: Fetch from live GitHub URL
    if manifest_json.is_none() {
        log::info!("[manifest] Cache miss. Fetching live manifest from {}", ModelManifest::MANIFEST_URL);
        match ureq::get(ModelManifest::MANIFEST_URL)
            .timeout(std::time::Duration::from_secs(5))
            .call()
        {
            Ok(resp) => {
                if let Ok(body) = resp.into_string() {
                    // Test parsing before caching to avoid corrupting DB
                    if ModelManifest::parse(&body).is_ok() {
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('manifest_cached_json', ?)",
                            [&body],
                        );
                        let _ = conn.execute(
                            "INSERT OR REPLACE INTO app_settings (key, value) VALUES ('manifest_last_fetched', ?)",
                            [&now.to_string()],
                        );
                        manifest_json = Some(body);
                        log::info!("[manifest] Live manifest fetched and cached successfully.");
                    } else {
                        log::warn!("[manifest] Fetched manifest is invalid JSON. Falling back.");
                    }
                }
            }
            Err(err) => {
                log::warn!("[manifest] Failed to fetch live manifest: {}. Using fallback/cached.", err);
            }
        }
    }

    // 4. Resolve manifest struct
    let manifest = match manifest_json {
        Some(json) => ModelManifest::parse(&json).unwrap_or_else(|_| ModelManifest::fallback()),
        None => {
            // Retrieve old cache if network failed, or use fallback
            let old_cached: Option<String> = conn
                .query_row(
                    "SELECT value FROM app_settings WHERE key = 'manifest_cached_json'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            match old_cached {
                Some(json) => ModelManifest::parse(&json).unwrap_or_else(|_| ModelManifest::fallback()),
                None => ModelManifest::fallback(),
            }
        }
    };

    // 5. Compare semantic version
    let current_version = app.package_info().version.to_string();
    let min_version = manifest.min_app_version.clone();
    
    let update_available = match (Version::parse(&current_version), Version::parse(&min_version)) {
        (Ok(curr), Ok(min)) => curr < min,
        _ => false,
    };

    if update_available {
        log::info!("[manifest] Out of date detected! current={}, min={}", current_version, min_version);
    } else {
        log::info!("[manifest] App is up to date. current={}, min={}", current_version, min_version);
    }

    Ok(AppManifestResponse {
        manifest,
        update_available,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::setup_test_db;

    #[test]
    fn test_update_settings_roundtrip() {
        let conn = setup_test_db();

        // Initial default should be true
        let default_val = get_update_settings_impl(&conn).unwrap();
        assert!(default_val);

        // Disable checks
        set_update_settings_impl(&conn, false).unwrap();
        let disabled_val = get_update_settings_impl(&conn).unwrap();
        assert!(!disabled_val);

        // Re-enable checks
        set_update_settings_impl(&conn, true).unwrap();
        let enabled_val = get_update_settings_impl(&conn).unwrap();
        assert!(enabled_val);
    }
}
