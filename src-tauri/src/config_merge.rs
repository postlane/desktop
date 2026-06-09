// SPDX-License-Identifier: BUSL-1.1

pub use crate::config_local_write::{
    remove_scheduler_provider_from_local_config, write_scheduler_provider_to_local_config,
};
use crate::init::read_json_file;
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
    // Workspace configs use a flat layout ({root}/config.json).
    // Legacy repo configs use {root}/.postlane/config.json.
    let postlane_dir = repo_path.join(".postlane");
    let (config_path, local_path) = if postlane_dir.is_dir() {
        (postlane_dir.join("config.json"), postlane_dir.join("config.local.json"))
    } else {
        (repo_path.join("config.json"), repo_path.join("config.local.json"))
    };
    let mut merged: serde_json::Value = read_json_file(&config_path)?;

    if !local_path.exists() {
        return Ok(merged);
    }

    let local: serde_json::Value = read_json_file(&local_path)?;

    validate_local_config(&local)?;
    apply_local_overrides(&mut merged, &local);

    Ok(merged)
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

    #[test]
    fn test_read_merged_repo_config_handles_workspace_flat_layout() {
        // Workspace configs have no .postlane/ — config.json sits directly at the root.
        let dir = tempfile::TempDir::new().expect("tmp dir");
        fs::write(
            dir.path().join("config.json"),
            r#"{"project_id":"ws-proj","scheduler":{"account_ids":{"bluesky":"postlane"}},"schema_version":4}"#,
        ).expect("write config.json");
        fs::write(
            dir.path().join("config.local.json"),
            r#"{"scheduler":{"provider":"upload_post"}}"#,
        ).expect("write config.local.json");

        let merged = read_merged_repo_config(dir.path()).expect("flat layout must succeed");
        assert_eq!(merged["project_id"].as_str(), Some("ws-proj"));
        assert_eq!(merged["scheduler"]["provider"].as_str(), Some("upload_post"));
        assert_eq!(
            merged["scheduler"]["account_ids"]["bluesky"].as_str(),
            Some("postlane"),
        );
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

}
