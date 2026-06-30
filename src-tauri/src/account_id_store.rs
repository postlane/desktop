// SPDX-License-Identifier: BUSL-1.1

//! Thin wrappers over `config_map_field` for the `scheduler.account_ids` field.

use std::collections::HashMap;
use std::path::Path;

/// Writes `account_id` for `platform` into `scheduler.account_ids` in `config_path`.
pub(crate) fn save_account_id_impl(
    config_path: &Path,
    platform: &str,
    account_id: &str,
) -> Result<(), String> {
    crate::config_map_field::save_scheduler_field(config_path, "account_ids", platform, account_id)
}

/// Returns `scheduler.account_ids` from `config.json` as a platform → account-id map.
pub(crate) fn get_account_ids_impl(config_path: &Path) -> Result<HashMap<String, String>, String> {
    crate::config_map_field::get_scheduler_field(config_path, "account_ids")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::make_state;
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
    fn test_save_account_id_writes_x_account_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": { "provider": "zernio", "account_ids": {} }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-twitter-123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-twitter-123"));
    }

    #[test]
    fn test_save_account_id_preserves_other_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": {
                "provider": "zernio",
                "account_ids": { "x": "acc-twitter-existing" }
            }
        }"#);

        save_account_id_impl(&config_path, "bluesky", "acc-bluesky-456").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-twitter-existing"));
        assert_eq!(config["scheduler"]["account_ids"]["bluesky"].as_str(), Some("acc-bluesky-456"));
    }

    #[test]
    fn test_save_account_id_creates_account_ids_block_if_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "provider": "zernio" }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-new").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-new"));
    }

    #[test]
    fn test_save_account_id_preserves_other_config_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "base_url": "https://postlane.dev",
            "repo_type": "saas-product",
            "scheduler": { "provider": "zernio", "account_ids": {} },
            "llm": { "provider": "anthropic", "model": "claude-sonnet-4-6" }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-x").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["version"].as_i64(), Some(1));
        assert_eq!(config["base_url"].as_str(), Some("https://postlane.dev"));
        assert_eq!(config["repo_type"].as_str(), Some("saas-product"));
        assert_eq!(config["scheduler"]["provider"].as_str(), Some("zernio"));
        assert_eq!(config["llm"]["model"].as_str(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn test_save_account_id_errors_when_config_missing() {
        let result = save_account_id_impl(
            Path::new("/nonexistent/path/.postlane/config.json"),
            "x",
            "acc-123",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_save_account_id_creates_scheduler_block_when_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{ "version": 1, "platforms": ["x"] }"#);

        save_account_id_impl(&config_path, "x", "acc-123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-123"));
    }

    #[test]
    fn test_save_account_id_rejects_unregistered_repo() {
        let state = make_state(vec![]);
        let repos = state.repos.lock().expect("lock");
        let result: Result<(), String> = repos.repos.iter()
            .find(|r| r.id == "nonexistent")
            .map(|_| Ok(()))
            .unwrap_or_else(|| Err(format!("Repo '{}' not in registered repos", "nonexistent")));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in registered repos"));
    }

    #[test]
    fn test_save_account_id_errors_when_config_unparseable() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), "{ not valid json }");
        let result = save_account_id_impl(&config_path, "x", "acc-123");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("parse") || msg.contains("Failed"),
            "error must describe parse failure, got: {}",
            msg
        );
    }

    #[test]
    fn test_get_account_ids_impl_returns_empty_when_file_absent() {
        let result = get_account_ids_impl(
            Path::new("/nonexistent/path/.postlane/config.json"),
        );
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_account_ids_impl_returns_empty_when_no_account_ids_key() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"scheduler":{}}"#);
        let result = get_account_ids_impl(&config_path);
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_get_account_ids_impl_returns_map_when_present() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "scheduler": {
                "account_ids": { "x": "acc123", "bluesky": "bsky456" }
            }
        }"#);
        let result = get_account_ids_impl(&config_path);
        assert!(result.is_ok(), "{:?}", result);
        let map = result.unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("x").map(String::as_str), Some("acc123"));
        assert_eq!(map.get("bluesky").map(String::as_str), Some("bsky456"));
    }

    #[test]
    fn test_get_account_ids_impl_returns_err_on_corrupt_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), "not json");
        let result = get_account_ids_impl(&config_path);
        assert!(result.is_err(), "expected Err for corrupt JSON");
    }

    #[test]
    fn test_get_account_ids_impl_ignores_non_string_values() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "scheduler": {
                "account_ids": { "x": "acc", "bad": 123 }
            }
        }"#);
        let result = get_account_ids_impl(&config_path);
        assert!(result.is_ok(), "{:?}", result);
        let map = result.unwrap();
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("x").map(String::as_str), Some("acc"));
        assert!(!map.contains_key("bad"));
    }

    // --- §concurrent_write (HIGH-3) ---

    #[test]
    fn test_save_account_id_and_name_concurrent_writes_preserve_both_fields() {
        // Regression test for the race condition where save_account_id_impl and
        // save_account_name_impl can interleave their read-mutate-write on config.json,
        // causing one writer to clobber the other. Without a per-path Mutex, one field
        // will occasionally be absent from the final file.
        //
        // The barrier synchronises thread start so both threads enter the write loop
        // simultaneously, making the race reliably observable.
        use std::sync::{Arc, Barrier};
        use std::thread;

        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);

        let path_a = config_path.clone();
        let path_b = config_path.clone();

        let barrier = Arc::new(Barrier::new(2));
        let ba = barrier.clone();
        let bb = barrier.clone();

        let h1 = thread::spawn(move || {
            ba.wait();
            for _ in 0..50 {
                let _ = save_account_id_impl(&path_a, "x", "acc-x-123");
            }
        });
        let h2 = thread::spawn(move || {
            bb.wait();
            for _ in 0..50 {
                let _ = crate::account_name_store::save_account_name_impl(&path_b, "bluesky", "@test_handle");
            }
        });
        h1.join().expect("thread 1 panicked");
        h2.join().expect("thread 2 panicked");

        let content = std::fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(
            config["scheduler"]["account_ids"]["x"].as_str(),
            Some("acc-x-123"),
            "account_id for x must survive concurrent writes"
        );
        assert_eq!(
            config["scheduler"]["account_names"]["bluesky"].as_str(),
            Some("@test_handle"),
            "account_name for bluesky must survive concurrent writes"
        );
    }
}
