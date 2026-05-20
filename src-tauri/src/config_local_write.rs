// SPDX-License-Identifier: BUSL-1.1

//! Writes and removes scheduler provider entries in `.postlane/config.local.json`.

use crate::init::{atomic_write, read_json_file};
use std::path::Path;

/// Adds `provider` to the scheduler fallback list in `.postlane/config.local.json`.
/// Creates the file if absent. When only one provider exists, writes `scheduler.provider`;
/// when two or more are configured, upgrades to `scheduler.fallback_order` so the
/// credential router can try each in order.
pub fn write_scheduler_provider_to_local_config(repo_path: &Path, provider: &str) -> Result<(), String> {
    let local_path = repo_path.join(".postlane").join("config.local.json");

    let mut local: serde_json::Value = if local_path.exists() {
        read_json_file(&local_path)?
    } else {
        serde_json::json!({})
    };

    if !local["scheduler"].is_object() {
        local["scheduler"] = serde_json::json!({});
    }

    let mut order: Vec<String> = if let Some(arr) = local["scheduler"]["fallback_order"].as_array() {
        arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
    } else if let Some(p) = local["scheduler"]["provider"].as_str() {
        if p.is_empty() { vec![] } else { vec![p.to_string()] }
    } else {
        vec![]
    };

    if !order.contains(&provider.to_string()) {
        order.push(provider.to_string());
    }

    if order.len() > 1 {
        local["scheduler"]["fallback_order"] = serde_json::json!(&order);
        local["scheduler"].as_object_mut().map(|s| s.remove("provider"));
    } else {
        local["scheduler"]["provider"] =
            serde_json::json!(order.first().map(String::as_str).unwrap_or(""));
    }

    let json = serde_json::to_string_pretty(&local)
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    atomic_write(&local_path, json.as_bytes())
        .map_err(|e| format!("Failed to write config.local.json: {}", e))
}

/// Removes `provider` from the scheduler fallback list in `.postlane/config.local.json`.
/// When the removed provider was the only one, sets `scheduler.provider` to `""` so that
/// `read_fallback_order_from_value` treats the repo as unconfigured.
/// Returns `Ok` without error if the file does not exist or the provider is not present.
pub fn remove_scheduler_provider_from_local_config(
    repo_path: &Path,
    provider: &str,
) -> Result<(), String> {
    let local_path = repo_path.join(".postlane").join("config.local.json");
    if !local_path.exists() {
        return Ok(());
    }

    let mut local: serde_json::Value = read_json_file(&local_path)?;

    if !local["scheduler"].is_object() {
        return Ok(());
    }

    let mut order: Vec<String> =
        if let Some(arr) = local["scheduler"]["fallback_order"].as_array() {
            arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        } else if let Some(p) = local["scheduler"]["provider"].as_str() {
            if p.is_empty() { vec![] } else { vec![p.to_string()] }
        } else {
            vec![]
        };

    order.retain(|p| p != provider);

    if let Some(sched) = local["scheduler"].as_object_mut() {
        sched.remove("fallback_order");
        sched.remove("provider");
    }

    if order.len() > 1 {
        local["scheduler"]["fallback_order"] = serde_json::json!(&order);
    } else {
        local["scheduler"]["provider"] =
            serde_json::json!(order.first().map(String::as_str).unwrap_or(""));
    }

    let json = serde_json::to_string_pretty(&local)
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    atomic_write(&local_path, json.as_bytes())
        .map_err(|e| format!("Failed to write config.local.json: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_provider_creates_local_config_when_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");

        write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
    }

    #[test]
    fn test_write_provider_updates_existing_local_config_preserving_other_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.local.json"), r#"{"profile_id":"abc","scheduler":{"provider":""}}"#)
            .expect("write initial");

        write_scheduler_provider_to_local_config(dir.path(), "publer").expect("write");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("publer"));
        assert_eq!(v["profile_id"].as_str(), Some("abc"), "profile_id must be preserved");
    }

    #[test]
    fn test_write_second_provider_creates_fallback_order() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"upload_post"}}"#)
            .expect("write initial");

        write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        let order = v["scheduler"]["fallback_order"].as_array().expect("fallback_order array");
        assert_eq!(order[0].as_str(), Some("upload_post"));
        assert_eq!(order[1].as_str(), Some("zernio"));
        assert!(v["scheduler"]["provider"].is_null(), "single provider field removed when fallback_order present");
    }

    #[test]
    fn test_write_duplicate_provider_is_not_appended() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
            .expect("write initial");

        write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"), "single provider kept");
        assert!(v["scheduler"]["fallback_order"].is_null(), "no fallback_order for single provider");
    }

    #[test]
    fn test_remove_only_provider_clears_to_empty() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
            .expect("write initial");

        remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some(""), "provider must be cleared");
    }

    #[test]
    fn test_remove_provider_from_fallback_downgrades_to_single() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(
            postlane.join("config.local.json"),
            r#"{"scheduler":{"fallback_order":["zernio","buffer"]}}"#,
        ).expect("write initial");

        remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("buffer"));
        assert!(v["scheduler"]["fallback_order"].is_null());
    }

    #[test]
    fn test_remove_provider_from_three_keeps_fallback_order() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(
            postlane.join("config.local.json"),
            r#"{"scheduler":{"fallback_order":["zernio","buffer","publer"]}}"#,
        ).expect("write initial");

        remove_scheduler_provider_from_local_config(dir.path(), "buffer").expect("remove");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        let order = v["scheduler"]["fallback_order"].as_array().expect("fallback_order");
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].as_str(), Some("zernio"));
        assert_eq!(order[1].as_str(), Some("publer"));
    }

    #[test]
    fn test_remove_nonexistent_provider_is_noop() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
            .expect("write initial");

        remove_scheduler_provider_from_local_config(dir.path(), "buffer").expect("remove");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
    }

    #[test]
    fn test_remove_from_missing_file_is_ok() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");

        let result = remove_scheduler_provider_from_local_config(dir.path(), "zernio");
        assert!(result.is_ok());
    }
}
