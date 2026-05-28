// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
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

    let mut repos = state.lock_repos()?;

    repos.repos.push(repo.clone());

    write_repos(&state.repos_path, &repos)
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
    let mut repos = state.lock_repos()?;

    let repo_index = repos
        .repos
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repos.repos.remove(repo_index);

    write_repos(&state.repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    Ok(())
}

#[tauri::command]
pub fn remove_repo(id: String, state: State<AppState>) -> Result<(), String> {
    remove_repo_impl(&id, &state)
}

pub fn set_repo_active_impl(id: &str, active: bool, state: &AppState) -> Result<(), String> {
    let mut repos = state.lock_repos()?;

    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repo.active = active;

    write_repos(&state.repos_path, &repos)
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
            let repos = state.lock_repos()?;
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
    let repos = state.lock_repos()?;

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

pub(crate) fn update_repo_path_impl(id: &str, new_path: &str, state: &AppState) -> Result<(), String> {
    let canonical_path = fs::canonicalize(new_path)
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

    let mut repos = state.lock_repos()?;

    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repo.path = canonical_str.to_string();

    write_repos(&state.repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    Ok(())
}

#[tauri::command]
pub fn update_repo_path(
    id: String,
    new_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    update_repo_path_impl(&id, &new_path, &state)
}

pub fn start_repo_watcher(
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
                if let Err(e) = crate::draft_schedule::pre_populate_schedule_if_needed(changed) {
                    log::warn!("Failed to pre-populate schedule for {:?}: {}", changed, e);
                }
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

    /// Build a state whose repos.json lives in `dir`, seeded with the given repos.
    fn make_state_at(dir: &std::path::Path, repos: Vec<crate::storage::Repo>) -> AppState {
        AppState::new_with_path(
            ReposConfig { version: 1, repos },
            dir.join("repos.json"),
        )
    }

    /// Create `.git/` and `.postlane/config.json` inside `dir`.
    fn scaffold_git_repo(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join(".git")).expect("create .git");
        std::fs::create_dir_all(dir.join(".postlane")).expect("create .postlane");
        std::fs::write(dir.join(".postlane/config.json"), r#"{"project":"test"}"#)
            .expect("write config.json");
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

    fn make_repo(id: &str, active: bool, path: &str) -> crate::storage::Repo {
        crate::storage::Repo {
            id: id.to_string(),
            name: "test-repo".to_string(),
            path: path.to_string(),
            active,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // --- add_repo_impl ---

    #[test]
    fn test_add_repo_returns_err_for_nonexistent_path() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = add_repo_impl("/nonexistent/path/xyz_not_real", &state);
        assert!(result.is_err(), "must err for nonexistent path");
    }

    #[test]
    fn test_add_repo_returns_err_when_no_git_dir() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = add_repo_impl(tmp.path().to_str().unwrap(), &state);
        assert!(result.is_err(), "must err without .git dir");
        assert!(result.unwrap_err().contains("Not a git repository"));
    }

    #[test]
    fn test_add_repo_returns_err_when_no_config_json() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git");
        let state = make_state_at(tmp.path(), vec![]);
        let result = add_repo_impl(tmp.path().to_str().unwrap(), &state);
        assert!(result.is_err(), "must err without config.json");
        assert!(result.unwrap_err().contains("config.json not found"));
    }

    #[test]
    fn test_add_repo_adds_repo_to_state() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        scaffold_git_repo(tmp.path());
        let state = make_state_at(tmp.path(), vec![]);
        let result = add_repo_impl(tmp.path().to_str().unwrap(), &state);
        assert!(result.is_ok(), "{:?}", result);
        let repo = result.unwrap();
        assert!(repo.active, "new repo must be active");
        let repos = state.repos.lock().unwrap();
        assert_eq!(repos.repos.len(), 1, "state must contain the new repo");
        assert_eq!(repos.repos[0].id, repo.id);
    }

    // --- remove_repo_impl ---

    #[test]
    fn test_remove_repo_returns_err_for_unknown_id() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = remove_repo_impl("no-such-id", &state);
        assert!(result.is_err(), "must err for unknown id");
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_remove_repo_removes_from_state() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", true, "/some/path");
        let state = make_state_at(tmp.path(), vec![repo]);
        remove_repo_impl("r1", &state).expect("remove must succeed");
        let repos = state.repos.lock().unwrap();
        assert!(repos.repos.is_empty(), "repo must be removed from state");
    }

    // --- set_repo_active_impl ---

    #[test]
    fn test_set_repo_active_returns_err_for_unknown_id() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = set_repo_active_impl("no-such-id", true, &state);
        assert!(result.is_err(), "must err for unknown id");
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_set_repo_active_sets_active_true() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", false, "/some/path");
        let state = make_state_at(tmp.path(), vec![repo]);
        set_repo_active_impl("r1", true, &state).expect("must succeed");
        let repos = state.repos.lock().unwrap();
        assert!(repos.repos[0].active, "repo must now be active");
    }

    #[test]
    fn test_set_repo_active_sets_active_false() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", true, "/some/path");
        let state = make_state_at(tmp.path(), vec![repo]);
        set_repo_active_impl("r1", false, &state).expect("must succeed");
        let repos = state.repos.lock().unwrap();
        assert!(!repos.repos[0].active, "repo must now be inactive");
    }

    // --- check_repo_health_impl ---

    #[test]
    fn test_check_repo_health_returns_empty_for_no_repos() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = check_repo_health_impl(&state).expect("must succeed");
        assert!(result.is_empty(), "no repos → empty health list");
    }

    #[test]
    fn test_check_repo_health_reachable_when_config_exists() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        scaffold_git_repo(tmp.path()); // creates .postlane/config.json
        let canonical = std::fs::canonicalize(tmp.path()).expect("canonicalize");
        let repo = make_repo("r1", true, canonical.to_str().unwrap());
        let state = make_state_at(tmp.path(), vec![repo]);
        let result = check_repo_health_impl(&state).expect("must succeed");
        assert_eq!(result.len(), 1);
        assert!(result[0].reachable, "repo with config.json must be reachable");
        assert_eq!(result[0].id, "r1");
    }

    #[test]
    fn test_check_repo_health_not_reachable_when_config_absent() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        // No .postlane/config.json
        let canonical = std::fs::canonicalize(tmp.path()).expect("canonicalize");
        let repo = make_repo("r2", true, canonical.to_str().unwrap());
        let state = make_state_at(tmp.path(), vec![repo]);
        let result = check_repo_health_impl(&state).expect("must succeed");
        assert_eq!(result.len(), 1);
        assert!(!result[0].reachable, "repo without config.json must not be reachable");
    }

    // --- update_repo_path_impl ---

    #[test]
    fn test_update_repo_path_returns_err_for_nonexistent_path() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state_at(tmp.path(), vec![]);
        let result = update_repo_path_impl("r1", "/nonexistent/path/xyz", &state);
        assert!(result.is_err(), "must err for nonexistent path");
    }

    #[test]
    fn test_update_repo_path_returns_err_when_no_git_dir() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        // no .git in tmp
        let state = make_state_at(tmp.path(), vec![]);
        let result = update_repo_path_impl("r1", tmp.path().to_str().unwrap(), &state);
        assert!(result.is_err(), "must err without .git dir");
    }

    #[test]
    fn test_update_repo_path_returns_err_when_no_config_json() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git");
        // no .postlane/config.json
        let state = make_state_at(tmp.path(), vec![]);
        let result = update_repo_path_impl("r1", tmp.path().to_str().unwrap(), &state);
        assert!(result.is_err(), "must err without config.json");
    }

    #[test]
    fn test_update_repo_path_returns_err_for_unknown_repo_id() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        scaffold_git_repo(tmp.path());
        // state has no repos
        let state = make_state_at(tmp.path(), vec![]);
        let result = update_repo_path_impl("no-such-id", tmp.path().to_str().unwrap(), &state);
        assert!(result.is_err(), "must err for unknown repo id");
    }

    #[test]
    fn test_update_repo_path_updates_path_in_state() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        scaffold_git_repo(tmp.path());
        let canonical = std::fs::canonicalize(tmp.path()).expect("canonicalize");
        let repo = crate::storage::Repo {
            id: "r1".to_string(),
            name: "r".to_string(),
            path: "/old/path".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let state = make_state_at(tmp.path(), vec![repo]);
        update_repo_path_impl("r1", canonical.to_str().unwrap(), &state)
            .expect("update should succeed");
        let repos = state.repos.lock().unwrap();
        assert_eq!(
            repos.repos[0].path,
            canonical.to_str().unwrap(),
            "path must be updated in state"
        );
    }
}
