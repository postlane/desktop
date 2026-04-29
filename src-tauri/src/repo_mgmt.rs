// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::init::postlane_dir;
use crate::storage::{write_repos, Repo};
use crate::types::RepoHealthStatus;
use std::fs;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

pub fn record_repo_connected(state: &AppState, consent: bool, name: &str) {
    state.telemetry.record(consent, "repo_connected", serde_json::json!({"name": name}));
}

pub fn add_repo_impl(path: &str, state: &AppState) -> Result<Repo, String> {
    let canonical_path = fs::canonicalize(path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;

    let git_dir = canonical_path.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    let config_path = canonical_path.join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found. Run `postlane init` first.".to_string());
    }

    let id = Uuid::new_v4().to_string();

    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid folder name")?
        .to_string();

    let repo = Repo {
        id: id.clone(),
        name: name.clone(),
        path: canonical_str.to_string(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    repos.repos.push(repo.clone());

    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_repo_connected(state, consent, &name);
    Ok(repo)
}

#[tauri::command]
pub fn add_repo(
    path: String,
    state: State<AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Repo, String> {
    let repo = add_repo_impl(&path, &state)?;
    start_repo_watcher(&repo.id, &repo.path, &state, app_handle);
    Ok(repo)
}

pub fn remove_repo_impl(id: &str, state: &AppState) -> Result<(), String> {
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo_index = repos
        .repos
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repos.repos.remove(repo_index);

    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    Ok(())
}

#[tauri::command]
pub fn remove_repo(id: String, state: State<AppState>) -> Result<(), String> {
    remove_repo_impl(&id, &state)
}

pub fn set_repo_active_impl(id: &str, active: bool, state: &AppState) -> Result<(), String> {
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repo.active = active;

    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    Ok(())
}

#[tauri::command]
pub fn set_repo_active(
    id: String,
    active: bool,
    state: State<AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    set_repo_active_impl(&id, active, &state)?;

    if active {
        let repo_path = {
            let repos = state
                .repos
                .lock()
                .map_err(|e| format!("Failed to lock repos: {}", e))?;
            repos
                .repos
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.path.clone())
                .ok_or_else(|| format!("Repo '{}' not found after activation", id))?
        };
        start_repo_watcher(&id, &repo_path, &state, app_handle);
    } else {
        crate::watcher::stop_watcher(&id, &state.watchers);
    }

    Ok(())
}

pub fn check_repo_health_impl(state: &AppState) -> Result<Vec<RepoHealthStatus>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut statuses = Vec::new();

    for repo in &repos.repos {
        let config_path = PathBuf::from(&repo.path)
            .join(".postlane")
            .join("config.json");

        let reachable = config_path.exists();

        statuses.push(RepoHealthStatus {
            id: repo.id.clone(),
            reachable,
            path: repo.path.clone(),
        });
    }

    Ok(statuses)
}

#[tauri::command]
pub fn check_repo_health(state: State<AppState>) -> Result<Vec<RepoHealthStatus>, String> {
    check_repo_health_impl(&state)
}

#[tauri::command]
pub fn update_repo_path(
    id: String,
    new_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(&new_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;

    let git_dir = canonical_path.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    let config_path = canonical_path.join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found at new path".to_string());
    }

    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repo.path = canonical_str.to_string();

    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    Ok(())
}

fn start_repo_watcher(
    repo_id: &str,
    repo_path: &str,
    state: &AppState,
    app_handle: tauri::AppHandle,
) {
    use crate::nav_commands::MetaChangedPayload;
    use tauri::Emitter;

    let id = repo_id.to_string();
    let path = std::path::Path::new(repo_path);

    if let Err(e) = crate::watcher::watch_repo(
        id.clone(),
        path,
        &state.watchers,
        move |changed_paths| {
            for changed in &changed_paths {
                let post_folder = changed
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_string();
                let payload = MetaChangedPayload {
                    repo_id: id.clone(),
                    post_folder,
                };
                if let Err(emit_err) = app_handle.emit("meta-changed", payload.clone()) {
                    log::warn!("Failed to emit meta-changed: {}", emit_err);
                }
                crate::tray::refresh_tray(&app_handle);
            }
        },
    ) {
        log::warn!("Failed to start watcher for repo {}: {}", repo_id, e);
    }
}

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
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos
            .repos
            .iter()
            .find(|r| r.id == repo_id)
            .ok_or_else(|| format!("Repo {} not found", repo_id))?
            .path
            .clone()
    };
    let config_path = std::path::PathBuf::from(&repo_path).join(".postlane/config.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;
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
    use crate::app_state::AppState;
    use crate::storage::ReposConfig;

    fn make_empty_state() -> AppState {
        AppState::new(ReposConfig { version: 1, repos: vec![] })
    }

    fn make_test_state_with_dir(dir: &std::path::Path) -> AppState {
        let canonical = std::fs::canonicalize(dir).expect("canonicalize");
        AppState::new(ReposConfig {
            version: 1,
            repos: vec![crate::storage::Repo {
                id: "r99".to_string(), name: "test".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    fn write_test_config(dir: &std::path::Path) -> std::path::PathBuf {
        let config_path = dir.join(".postlane/config.json");
        std::fs::create_dir_all(dir.join(".postlane")).expect("create dir");
        std::fs::write(&config_path, r#"{"scheduler":{"provider":"old"}}"#).expect("write");
        config_path
    }

    #[test]
    fn test_update_scheduler_config_writes_single_provider() {
        let dir = std::env::temp_dir().join("postlane_test_cfg_single");
        let config_path = write_test_config(&dir);
        let state = make_test_state_with_dir(&dir);
        let result = update_scheduler_config_impl("r99", &["zernio".to_string()], &state);
        assert!(result.is_ok(), "{:?}", result);
        let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["scheduler"]["provider"].as_str().unwrap(), "zernio");
        let order: Vec<&str> = config["scheduler"]["fallback_order"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(order, vec!["zernio"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_update_scheduler_config_writes_full_fallback_order() {
        let dir = std::env::temp_dir().join("postlane_test_cfg_multi");
        let config_path = write_test_config(&dir);
        let state = make_test_state_with_dir(&dir);
        let order_in = ["zernio".to_string(), "publer".to_string(), "outstand".to_string()];
        let result = update_scheduler_config_impl("r99", &order_in, &state);
        assert!(result.is_ok(), "{:?}", result);
        let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(config["scheduler"]["provider"].as_str().unwrap(), "zernio");
        let order: Vec<&str> = config["scheduler"]["fallback_order"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
        assert_eq!(order, vec!["zernio", "publer", "outstand"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_update_scheduler_config_rejects_empty_list() {
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = update_scheduler_config_impl("r99", &[], &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_scheduler_config_rejects_unknown_provider() {
        let dir = std::env::temp_dir().join("postlane_test_cfg_unknown_prov");
        write_test_config(&dir);
        let state = make_test_state_with_dir(&dir);
        let result = update_scheduler_config_impl("r99", &["unknown_xyz".to_string()], &state);
        assert!(result.is_err(), "unknown provider must be rejected");
        let err = result.unwrap_err();
        assert!(err.contains("unknown_xyz"), "error must identify the bad provider, got: {}", err);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_update_scheduler_config_rejects_provider_in_mixed_list() {
        let dir = std::env::temp_dir().join("postlane_test_cfg_mixed_prov");
        write_test_config(&dir);
        let state = make_test_state_with_dir(&dir);
        let result = update_scheduler_config_impl(
            "r99",
            &["zernio".to_string(), "bad_provider".to_string()],
            &state,
        );
        assert!(result.is_err(), "list with unknown provider must be rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_update_scheduler_config_errors_on_missing_repo() {
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = update_scheduler_config_impl("nonexistent", &["zernio".to_string()], &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_record_repo_connected_queues_when_consent_given() {
        let state = make_empty_state();
        record_repo_connected(&state, true, "my-repo");
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    }

    #[test]
    fn test_record_repo_connected_no_op_when_consent_not_given() {
        let state = make_empty_state();
        record_repo_connected(&state, false, "my-repo");
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
    }
}
