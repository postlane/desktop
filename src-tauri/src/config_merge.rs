// SPDX-License-Identifier: BUSL-1.1

use crate::init::{atomic_write, read_json_file};
use std::path::Path;

/// Fields permitted in `config.local.json` (dot-notation for nested fields).
/// Any field outside this list is rejected with a parse error naming the offending field.
const PERMITTED_LOCAL_FIELDS: &[&str] = &[
    "profile_id",
    "scheduler.provider",
    "scheduler.api_key_hint",
    "scheduler.fallback_order",
];

fn validate_local_config(local: &serde_json::Value) -> Result<(), String> {
    let obj = local
        .as_object()
        .ok_or_else(|| "config.local.json must be a JSON object".to_string())?;

    for (key, value) in obj {
        let direct = PERMITTED_LOCAL_FIELDS.contains(&key.as_str());
        let prefix = format!("{}.", key);
        let has_nested = PERMITTED_LOCAL_FIELDS
            .iter()
            .any(|f| f.starts_with(&prefix));

        if direct {
            // top-level permitted field — allowed as-is
        } else if has_nested {
            let sub_obj = value.as_object().ok_or_else(|| {
                format!("config.local.json: '{}' must be an object", key)
            })?;
            for sub_key in sub_obj.keys() {
                let path = format!("{}.{}", key, sub_key);
                if !PERMITTED_LOCAL_FIELDS.contains(&path.as_str()) {
                    return Err(format!(
                        "config.local.json: unrecognised field '{}'",
                        path
                    ));
                }
            }
        } else {
            return Err(format!(
                "config.local.json: unrecognised field '{}'",
                key
            ));
        }
    }
    Ok(())
}

fn apply_local_overrides(merged: &mut serde_json::Value, local: &serde_json::Value) {
    if let Some(v) = local.get("profile_id") {
        merged["profile_id"] = v.clone();
    }
    if let Some(sched) = local.get("scheduler").and_then(|s| s.as_object()) {
        if !merged["scheduler"].is_object() {
            merged["scheduler"] = serde_json::json!({});
        }
        for key in ["provider", "api_key_hint", "fallback_order"] {
            if let Some(v) = sched.get(key) {
                merged["scheduler"][key] = v.clone();
            }
        }
    }
}

/// Reads `.postlane/config.json`, optionally merges `.postlane/config.local.json` on top.
///
/// If `config.local.json` is absent the shared `config.json` is returned as-is
/// (backward-compatibility with v1 installs).  If it is present, every field is
/// validated against [`PERMITTED_LOCAL_FIELDS`]; unrecognised fields are rejected
/// with an error that names the offending field.
pub fn read_merged_repo_config(repo_path: &Path) -> Result<serde_json::Value, String> {
    let config_path = repo_path.join(".postlane").join("config.json");
    let mut merged: serde_json::Value = read_json_file(&config_path)?;

    let local_path = repo_path.join(".postlane").join("config.local.json");
    if !local_path.exists() {
        return Ok(merged);
    }

    let local: serde_json::Value = read_json_file(&local_path)?;

    validate_local_config(&local)?;
    apply_local_overrides(&mut merged, &local);

    Ok(merged)
}

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

    // Collect existing providers, preferring fallback_order over legacy provider field.
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
/// `read_fallback_order_from_value` treats the repo as unconfigured, giving the user the
/// "No scheduler configured" error rather than "all schedulers at limit or no credentials".
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

    fn setup(name: &str, config: &str, local: Option<&str>) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("postlane_config_merge_{}", name));
        let _ = fs::remove_dir_all(&dir);
        let postlane = dir.join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.json"), config).expect("write config.json");
        if let Some(l) = local {
            fs::write(postlane.join("config.local.json"), l).expect("write config.local.json");
        }
        dir
    }

    #[test]
    fn test_local_overrides_shared_field() {
        let dir = setup(
            "override",
            r#"{"version":1,"scheduler":{"provider":"buffer"}}"#,
            Some(r#"{"scheduler":{"provider":"zernio"}}"#),
        );
        let merged = read_merged_repo_config(&dir).expect("should succeed");
        assert_eq!(merged["scheduler"]["provider"].as_str(), Some("zernio"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_local_override_does_not_wipe_other_shared_fields() {
        let dir = setup(
            "no_wipe",
            r#"{"version":1,"base_url":"https://example.com","scheduler":{"provider":"buffer","account_ids":{}}}"#,
            Some(r#"{"scheduler":{"provider":"zernio"}}"#),
        );
        let merged = read_merged_repo_config(&dir).expect("should succeed");
        assert_eq!(merged["base_url"].as_str(), Some("https://example.com"));
        assert_eq!(merged["scheduler"]["provider"].as_str(), Some("zernio"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_missing_local_config_reads_shared_only() {
        let dir = setup(
            "no_local",
            r#"{"version":1,"scheduler":{"provider":"buffer"}}"#,
            None,
        );
        let merged = read_merged_repo_config(&dir).expect("should succeed without config.local.json");
        assert_eq!(merged["scheduler"]["provider"].as_str(), Some("buffer"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unknown_field_in_local_returns_error_naming_field() {
        let dir = setup(
            "unknown_field",
            r#"{"version":1}"#,
            Some(r#"{"api_key":"secret"}"#),
        );
        let result = read_merged_repo_config(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("api_key"), "Error must name the offending field, got: {}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unknown_nested_field_in_local_returns_error_naming_field() {
        let dir = setup(
            "unknown_nested",
            r#"{"version":1}"#,
            Some(r#"{"scheduler":{"secret_key":"abc"}}"#),
        );
        let result = read_merged_repo_config(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("scheduler.secret_key"),
            "Error must name the nested field, got: {}",
            err
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_permitted_local_fields_constant_covers_all_spec_fields() {
        assert!(PERMITTED_LOCAL_FIELDS.contains(&"profile_id"));
        assert!(PERMITTED_LOCAL_FIELDS.contains(&"scheduler.provider"));
        assert!(PERMITTED_LOCAL_FIELDS.contains(&"scheduler.api_key_hint"));
        assert!(PERMITTED_LOCAL_FIELDS.contains(&"scheduler.fallback_order"));
    }

    // ── write_scheduler_provider_to_local_config ─────────────────────────────

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
    fn test_local_fallback_order_merges_into_config() {
        let dir = setup(
            "fallback_merge",
            r#"{"version":1,"scheduler":{"provider":"buffer"}}"#,
            Some(r#"{"scheduler":{"fallback_order":["zernio","upload_post"]}}"#),
        );
        let merged = read_merged_repo_config(&dir).expect("should succeed");
        let order = merged["scheduler"]["fallback_order"].as_array().expect("fallback_order");
        assert_eq!(order[0].as_str(), Some("zernio"));
        assert_eq!(order[1].as_str(), Some("upload_post"));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── remove_scheduler_provider_from_local_config ──────────────────────────

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
        )
        .expect("write initial");

        remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");

        let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("buffer"), "remaining provider promoted");
        assert!(v["scheduler"]["fallback_order"].is_null(), "fallback_order removed when one left");
    }

    #[test]
    fn test_remove_provider_from_three_keeps_fallback_order() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(
            postlane.join("config.local.json"),
            r#"{"scheduler":{"fallback_order":["zernio","buffer","publer"]}}"#,
        )
        .expect("write initial");

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
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"), "unrelated provider unchanged");
    }

    #[test]
    fn test_remove_from_missing_file_is_ok() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");

        let result = remove_scheduler_provider_from_local_config(dir.path(), "zernio");
        assert!(result.is_ok(), "should not error when file is absent");
    }
}
