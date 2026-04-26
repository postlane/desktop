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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::ReposConfig;

    fn make_empty_state() -> AppState {
        AppState::new(ReposConfig { version: 1, repos: vec![] })
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
