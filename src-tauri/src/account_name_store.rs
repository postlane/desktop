// SPDX-License-Identifier: BUSL-1.1

//! Atomic read/write for `scheduler.account_names` in `.postlane/config.json`.

use crate::init::read_json_file;
use std::fs;
use std::path::Path;

/// Writes a display name (e.g. "@rng_dev") for `platform` into
/// `scheduler.account_names` in `config_path`. Atomic write, same contract as
/// `save_account_id_impl`.
pub fn save_account_name_impl(
    config_path: &Path,
    platform: &str,
    name: &str,
) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }
    let mut config: serde_json::Value = read_json_file(config_path)?;
    if !config["scheduler"].is_object() {
        config["scheduler"] = serde_json::json!({});
    }
    if !config["scheduler"]["account_names"].is_object() {
        config["scheduler"]["account_names"] = serde_json::json!({});
    }
    config["scheduler"]["account_names"][platform] = serde_json::json!(name);
    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.json: {}", e))?;
    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, &serialized)
        .map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&tmp_path, config_path)
        .map_err(|e| format!("Failed to rename temp config: {}", e))?;
    Ok(())
}

/// Returns `scheduler.account_names` from `config.json` as a platform → display-name map.
/// Returns an empty map when the file is absent; errors on parse failure.
pub(crate) fn get_account_names_impl(
    config_path: &Path,
) -> Result<std::collections::HashMap<String, String>, String> {
    if !config_path.exists() {
        return Ok(std::collections::HashMap::new());
    }
    let config: serde_json::Value = read_json_file(config_path)?;
    let names = match config["scheduler"]["account_names"].as_object() {
        Some(obj) => obj
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        None => std::collections::HashMap::new(),
    };
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn write_config(dir: &Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let config_path = config_dir.join("config.json");
        fs::write(&config_path, json).expect("write config.json");
        config_path
    }

    #[test]
    fn test_save_account_name_writes_name_for_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        save_account_name_impl(&config_path, "bluesky", "@rng_dev").expect("should succeed");
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_names"]["bluesky"].as_str(), Some("@rng_dev"));
    }

    #[test]
    fn test_save_account_name_preserves_existing_names_for_other_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(
            dir.path(),
            r#"{"version":1,"scheduler":{"account_names":{"x":"@existing_x"}}}"#,
        );
        save_account_name_impl(&config_path, "bluesky", "@new_bsky").expect("should succeed");
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_names"]["x"].as_str(), Some("@existing_x"));
        assert_eq!(config["scheduler"]["account_names"]["bluesky"].as_str(), Some("@new_bsky"));
    }

    #[test]
    fn test_save_account_name_errors_when_config_missing() {
        let result = save_account_name_impl(
            Path::new("/nonexistent/.postlane/config.json"),
            "bluesky",
            "@handle",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_get_account_names_impl_returns_empty_when_file_absent() {
        let result = get_account_names_impl(Path::new("/nonexistent/path/config.json"));
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_account_names_impl_returns_names_from_config() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "scheduler": {
                "account_names": { "x": "@testuser", "linkedin": "Test User" }
            }
        }"#);
        let result = get_account_names_impl(&config_path);
        assert!(result.is_ok(), "{:?}", result);
        let map = result.unwrap();
        assert_eq!(map.get("x").map(String::as_str), Some("@testuser"));
        assert_eq!(map.get("linkedin").map(String::as_str), Some("Test User"));
    }

    #[test]
    fn test_get_account_names_impl_returns_empty_when_no_account_names_key() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"scheduler": {"provider": "zernio"}}"#);
        let result = get_account_names_impl(&config_path);
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty(), "missing account_names key must return empty map");
    }
}
