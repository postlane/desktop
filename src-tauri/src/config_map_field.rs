// SPDX-License-Identifier: BUSL-1.1

//! Generic read/write for named map fields inside `scheduler` in `.postlane/config.json`.
//! Both `account_id_store` and `account_name_store` use this to eliminate their
//! shared 13-step read-lock-mutate-write loop.

use crate::init::read_json_file;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

fn acquire_config_lock(config_path: &Path) -> Arc<std::sync::Mutex<()>> {
    let key = config_path.to_string_lossy().into_owned();
    crate::platform_constants::CONFIG_JSON_LOCKS
        .entry(key)
        .or_insert_with(|| Arc::new(std::sync::Mutex::new(())))
        .clone()
}

/// Writes `value` for `platform` into `scheduler.<field>` in `config_path`.
/// Atomic write (tmp → rename). Creates `scheduler` and `<field>` blocks if absent.
/// Holds a per-path Mutex across the read-mutate-write cycle to prevent concurrent
/// writes from clobbering each other.
pub(crate) fn save_scheduler_field(
    config_path: &Path,
    field: &str,
    platform: &str,
    value: &str,
) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }

    let lock = acquire_config_lock(config_path);
    let _guard = lock.lock().map_err(|e| format!("config.json lock poisoned: {}", e))?;

    let mut config: serde_json::Value = read_json_file(config_path)?;

    if !config["scheduler"].is_object() {
        config["scheduler"] = serde_json::json!({});
    }
    if !config["scheduler"][field].is_object() {
        config["scheduler"][field] = serde_json::json!({});
    }
    config["scheduler"][field][platform] = serde_json::json!(value);

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.json: {}", e))?;

    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, &serialized)
        .map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&tmp_path, config_path)
        .map_err(|e| format!("Failed to rename temp config: {}", e))?;

    Ok(())
}

/// Returns `scheduler.<field>` from `config.json` as a platform → value map.
/// Returns an empty map when the file is absent; errors on parse failure.
pub(crate) fn get_scheduler_field(
    config_path: &Path,
    field: &str,
) -> Result<HashMap<String, String>, String> {
    if !config_path.exists() {
        return Ok(HashMap::new());
    }

    let config: serde_json::Value = read_json_file(config_path)?;

    let map = match config["scheduler"][field].as_object() {
        Some(obj) => obj
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        None => HashMap::new(),
    };

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_config(dir: &Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let path = config_dir.join("config.json");
        fs::write(&path, json).expect("write config.json");
        path
    }

    #[test]
    fn save_scheduler_field_writes_value_for_platform() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(dir.path(), r#"{"version":1}"#);
        save_scheduler_field(&path, "account_ids", "x", "acc-123").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["scheduler"]["account_ids"]["x"].as_str(), Some("acc-123"));
    }

    #[test]
    fn save_scheduler_field_preserves_other_platforms() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(
            dir.path(),
            r#"{"scheduler":{"account_ids":{"bluesky":"bsky-old"}}}"#,
        );
        save_scheduler_field(&path, "account_ids", "x", "acc-x").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["scheduler"]["account_ids"]["bluesky"].as_str(), Some("bsky-old"));
        assert_eq!(v["scheduler"]["account_ids"]["x"].as_str(), Some("acc-x"));
    }

    #[test]
    fn save_scheduler_field_creates_scheduler_and_field_blocks_if_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(dir.path(), r#"{"version":1}"#);
        save_scheduler_field(&path, "account_names", "bluesky", "@handle").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(v["scheduler"]["account_names"]["bluesky"].as_str(), Some("@handle"));
    }

    #[test]
    fn save_scheduler_field_errors_when_config_missing() {
        let result = save_scheduler_field(
            Path::new("/nonexistent/.postlane/config.json"),
            "account_ids",
            "x",
            "acc-123",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn get_scheduler_field_returns_empty_when_file_absent() {
        let result = get_scheduler_field(
            Path::new("/nonexistent/.postlane/config.json"),
            "account_ids",
        );
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn get_scheduler_field_returns_map_when_present() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(
            dir.path(),
            r#"{"scheduler":{"account_ids":{"x":"acc-x","bluesky":"acc-bsky"}}}"#,
        );
        let map = get_scheduler_field(&path, "account_ids").unwrap();
        assert_eq!(map.get("x").map(String::as_str), Some("acc-x"));
        assert_eq!(map.get("bluesky").map(String::as_str), Some("acc-bsky"));
    }

    #[test]
    fn get_scheduler_field_returns_empty_when_field_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(dir.path(), r#"{"scheduler":{"provider":"zernio"}}"#);
        let map = get_scheduler_field(&path, "account_ids").unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn get_scheduler_field_ignores_non_string_values() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_config(
            dir.path(),
            r#"{"scheduler":{"account_ids":{"x":"acc","bad":123}}}"#,
        );
        let map = get_scheduler_field(&path, "account_ids").unwrap();
        assert_eq!(map.len(), 1);
        assert!(!map.contains_key("bad"));
    }
}
