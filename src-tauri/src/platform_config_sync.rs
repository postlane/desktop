// SPDX-License-Identifier: BUSL-1.1

//! Writes the `connected_platforms` field in `.postlane/config.json` to reflect
//! the current set of connected platforms for a repo.

use std::path::Path;

/// Updates the `connected_platforms` field in `.postlane/config.json` to reflect
/// the current set of connected platforms for a repo.
/// Silent no-op (with warn log) if config.json does not exist.
pub(crate) fn sync_connected_platforms_to_config_impl(
    config_path: &Path,
    repo_id: &str,
    mastodon_active: bool,
    has_keyring_key: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    if !config_path.exists() {
        log::warn!("sync_connected_platforms: config.json not found at {:?}", config_path);
        return Ok(());
    }
    let platforms = crate::connected_platforms::list_connected_platforms_impl(
        config_path, repo_id, mastodon_active, has_keyring_key,
    );
    let mut config: serde_json::Value = crate::init::read_json_file(config_path)?;
    config["connected_platforms"] = serde_json::json!(platforms);
    crate::init::write_json_file(config_path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_config_for_sync(dir: &std::path::Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        let path = config_dir.join("config.json");
        std::fs::write(&path, json).expect("write config.json");
        path
    }

    #[test]
    fn test_sync_writes_connected_platforms_to_config() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config_for_sync(dir.path(), r#"{
            "version": 1,
            "project_id": "proj-abc",
            "scheduler": { "account_ids": { "bluesky": "myhandle" } }
        }"#);

        let result = sync_connected_platforms_to_config_impl(
            &config_path, "r1", false,
            &|key| key == "zernio/r1",
        );
        assert!(result.is_ok(), "expected Ok, got {:?}", result);

        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        let platforms = parsed["connected_platforms"].as_array().expect("array");
        assert!(platforms.iter().any(|v| v.as_str() == Some("bluesky")));
    }

    #[test]
    fn test_sync_includes_mastodon_when_active() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config_for_sync(dir.path(), r#"{"version":1}"#);

        let result = sync_connected_platforms_to_config_impl(
            &config_path, "r1", true, &|_| false,
        );
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        let platforms = parsed["connected_platforms"].as_array().expect("array");
        assert!(platforms.iter().any(|v| v.as_str() == Some("mastodon")));
    }

    #[test]
    fn test_sync_preserves_other_config_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config_for_sync(dir.path(), r#"{
            "version": 1,
            "project_id": "proj-xyz",
            "base_url": "https://postlane.dev"
        }"#);

        sync_connected_platforms_to_config_impl(
            &config_path, "r1", false, &|_| false,
        ).expect("should succeed");

        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-xyz"));
        assert_eq!(parsed["base_url"].as_str(), Some("https://postlane.dev"));
        assert_eq!(parsed["version"].as_u64(), Some(1));
    }

    #[test]
    fn test_sync_overwrites_previous_connected_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config_for_sync(dir.path(), r#"{
            "version": 1,
            "connected_platforms": ["x", "bluesky", "mastodon"],
            "scheduler": { "account_ids": { "bluesky": "myhandle" } }
        }"#);

        sync_connected_platforms_to_config_impl(
            &config_path, "r1", false,
            &|key| key == "zernio/r1",
        ).expect("should succeed");

        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        let platforms = parsed["connected_platforms"].as_array().expect("array");
        assert!(!platforms.iter().any(|v| v.as_str() == Some("x")), "x should not be present");
        assert!(!platforms.iter().any(|v| v.as_str() == Some("mastodon")), "mastodon not connected");
        assert!(platforms.iter().any(|v| v.as_str() == Some("bluesky")));
    }

    #[test]
    fn test_sync_writes_empty_array_when_nothing_connected() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config_for_sync(dir.path(), r#"{"version":1}"#);

        sync_connected_platforms_to_config_impl(
            &config_path, "r1", false, &|_| false,
        ).expect("should succeed");

        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        let platforms = parsed["connected_platforms"].as_array().expect("array");
        assert!(platforms.is_empty());
    }

    #[test]
    fn test_sync_is_no_op_when_config_missing() {
        let result = sync_connected_platforms_to_config_impl(
            std::path::Path::new("/nonexistent/.postlane/config.json"),
            "r1", false, &|_| false,
        );
        assert!(result.is_ok(), "missing config.json must not error, got {:?}", result);
    }
}
