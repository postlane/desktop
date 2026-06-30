// SPDX-License-Identifier: BUSL-1.1

//! Thin wrappers over `config_map_field` for the `scheduler.account_names` field.

use std::collections::HashMap;
use std::path::Path;

/// Writes a display name for `platform` into `scheduler.account_names` in `config_path`.
pub(crate) fn save_account_name_impl(
    config_path: &Path,
    platform: &str,
    name: &str,
) -> Result<(), String> {
    crate::config_map_field::save_scheduler_field(config_path, "account_names", platform, name)
}

/// Returns `scheduler.account_names` from `config.json` as a platform → display-name map.
pub(crate) fn get_account_names_impl(config_path: &Path) -> Result<HashMap<String, String>, String> {
    crate::config_map_field::get_scheduler_field(config_path, "account_names")
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
