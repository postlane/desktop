// SPDX-License-Identifier: BUSL-1.1

//! Per-repo scheduler provider configuration.
//! Writes `scheduler.provider` and `scheduler.fallback_order` into a repo's config.json.

use crate::app_state::AppState;
use crate::init::read_json_file;
use std::fs;
use tauri::State;

/// Updates `scheduler.provider` and `scheduler.fallback_order` in a repo's config.json.
/// `fallback_order` must be non-empty; the first entry becomes `scheduler.provider`.
pub fn update_scheduler_config_impl(
    repo_id: &str,
    fallback_order: &[String],
    state: &AppState,
) -> Result<(), String> {
    if fallback_order.is_empty() {
        return Err("fallback_order must contain at least one provider".to_string());
    }
    for provider in fallback_order {
        if !crate::scheduler_credentials::VALID_PROVIDERS.contains(&provider.as_str()) {
            return Err(format!("Unknown provider in fallback_order: '{}'", provider));
        }
    }
    let repo_path = {
        let repos = state.lock_repos()?;
        repos
            .repos
            .iter()
            .find(|r| r.id == repo_id)
            .ok_or_else(|| format!("Repo {} not found", repo_id))?
            .path
            .clone()
    };
    let config_path = std::path::PathBuf::from(&repo_path).join(".postlane/config.json");
    let mut config: serde_json::Value = read_json_file(&config_path)?;
    config["scheduler"]["provider"] = serde_json::json!(fallback_order[0]);
    config["scheduler"]["fallback_order"] = serde_json::json!(fallback_order);
    let tmp = config_path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to write config.json.tmp: {}", e))?;
    fs::rename(&tmp, &config_path)
        .map_err(|e| format!("Failed to rename config.json: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn update_scheduler_config(
    repo_id: String,
    fallback_order: Vec<String>,
    state: State<AppState>,
) -> Result<(), String> {
    update_scheduler_config_impl(&repo_id, &fallback_order, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ReposConfig;

    fn make_state_with_dir(dir: &std::path::Path) -> (AppState, tempfile::TempDir) {
        let canonical = std::fs::canonicalize(dir).expect("canonicalize");
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(
            ReposConfig {
                version: 1,
                repos: vec![crate::storage::Repo {
                    id: "r99".to_string(),
                    name: "test".to_string(),
                    path: canonical.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                }],
            },
            _tmp_repos.path().join("repos.json"),
        );
        (state, _tmp_repos)
    }

    fn write_config(dir: &std::path::Path) -> std::path::PathBuf {
        let config_path = dir.join(".postlane/config.json");
        std::fs::create_dir_all(dir.join(".postlane")).expect("create dir");
        std::fs::write(&config_path, r#"{"scheduler":{"provider":"old"}}"#).expect("write");
        config_path
    }

    #[test]
    fn test_update_scheduler_config_writes_single_provider() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path());
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result = update_scheduler_config_impl("r99", &["zernio".to_string()], &state);
        assert!(result.is_ok(), "{:?}", result);
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["scheduler"]["provider"].as_str().unwrap(), "zernio");
        let order: Vec<&str> = config["scheduler"]["fallback_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(order, vec!["zernio"]);
    }

    #[test]
    fn test_update_scheduler_config_writes_full_fallback_order() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path());
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let order_in = ["zernio".to_string(), "publer".to_string(), "outstand".to_string()];
        let result = update_scheduler_config_impl("r99", &order_in, &state);
        assert!(result.is_ok(), "{:?}", result);
        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["scheduler"]["provider"].as_str().unwrap(), "zernio");
        let order: Vec<&str> = config["scheduler"]["fallback_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(order, vec!["zernio", "publer", "outstand"]);
    }

    #[test]
    fn test_update_scheduler_config_rejects_empty_list() {
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(
            ReposConfig { version: 1, repos: vec![] },
            _tmp_repos.path().join("repos.json"),
        );
        let result = update_scheduler_config_impl("r99", &[], &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_scheduler_config_rejects_unknown_provider() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_config(dir.path());
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result =
            update_scheduler_config_impl("r99", &["unknown_xyz".to_string()], &state);
        assert!(result.is_err(), "unknown provider must be rejected");
        let err = result.unwrap_err();
        assert!(
            err.contains("unknown_xyz"),
            "error must identify the bad provider, got: {}",
            err
        );
    }

    #[test]
    fn test_update_scheduler_config_rejects_provider_in_mixed_list() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_config(dir.path());
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result = update_scheduler_config_impl(
            "r99",
            &["zernio".to_string(), "bad_provider".to_string()],
            &state,
        );
        assert!(result.is_err(), "list with unknown provider must be rejected");
    }

    #[test]
    fn test_update_scheduler_config_errors_on_missing_repo() {
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(
            ReposConfig { version: 1, repos: vec![] },
            _tmp_repos.path().join("repos.json"),
        );
        let result =
            update_scheduler_config_impl("nonexistent", &["zernio".to_string()], &state);
        assert!(result.is_err());
    }
}
