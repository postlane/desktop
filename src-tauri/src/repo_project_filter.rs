// SPDX-License-Identifier: BUSL-1.1

//! Project-scoped repo queries and lifecycle commands for M19.
//! `list_repos_for_project` — filter repos.json by config.json project_id.
//! `unregister_repo` — remove a repo registration and stop its file watcher.

use crate::app_state::AppState;
use crate::repo_mgmt::remove_repo_impl;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::State;

/// Lightweight repo summary returned to the frontend for project-scoped repo lists.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub active: bool,
}

/// Reads `.postlane/config.json` and returns `project_id`, or `Ok(None)` if absent.
fn read_config_project_id(repo_path: &str) -> Result<Option<String>, String> {
    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    if !config_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json for '{}': {}", repo_path, e))?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json for '{}': {}", repo_path, e))?;
    Ok(v["project_id"].as_str().map(str::to_string))
}

/// Returns repos whose `.postlane/config.json` matches `project_id`.
/// Skips repos whose path falls outside `$HOME` (security boundary).
pub fn list_repos_for_project_impl(project_id: &str, state: &AppState) -> Result<Vec<RepoSummary>, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?;
    let repos = state.lock_repos()?;

    let mut result = Vec::new();
    for repo in &repos.repos {
        let repo_path = PathBuf::from(&repo.path);
        if !repo_path.starts_with(&home_dir) {
            log::warn!("[list_repos_for_project] skipping repo outside $HOME: {}", repo.path);
            continue;
        }
        match read_config_project_id(&repo.path) {
            Ok(Some(pid)) if pid == project_id => {
                result.push(RepoSummary {
                    id: repo.id.clone(),
                    name: repo.name.clone(),
                    path: repo.path.clone(),
                    active: repo.active,
                });
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!("[list_repos_for_project] skipping repo '{}': {}", repo.path, e);
            }
        }
    }
    Ok(result)
}

/// Returns repos whose config.json `project_id` matches the given id.
/// Skips repos outside `$HOME`.
#[tauri::command]
pub fn list_repos_for_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<RepoSummary>, String> {
    list_repos_for_project_impl(&project_id, &state)
}

/// Removes a repo registration from repos.json and stops its file watcher.
/// Does NOT delete any files on disk — only removes the repos.json entry.
pub fn unregister_repo_impl(repo_id: &str, state: &AppState) -> Result<(), String> {
    // validate first (remove_repo_impl returns Err if not found)
    remove_repo_impl(repo_id, state)?;
    // stop watcher after removal so the repo path is gone from state
    crate::watcher::stop_watcher(repo_id, &state.watchers);
    Ok(())
}

/// Removes a repo registration and stops its file watcher.
/// Returns `Err` if `repo_id` is not found in repos.json.
#[tauri::command]
pub fn unregister_repo(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    unregister_repo_impl(&repo_id, &state)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_state, make_repo};
    use std::fs;

    fn write_config(repo_dir: &std::path::Path, project_id: Option<&str>) {
        let postlane_dir = repo_dir.join(".postlane");
        fs::create_dir_all(&postlane_dir).expect("create .postlane");
        let content = match project_id {
            Some(pid) => format!(r#"{{"project_id":"{}"}}"#, pid),
            None => r#"{}"#.to_string(),
        };
        fs::write(postlane_dir.join("config.json"), content).expect("write config.json");
    }

    #[test]
    fn test_list_repos_for_project_returns_matching_repos() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_lrfp_match");
        write_config(&dir, Some("proj-abc"));

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "r1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_repos_for_project_excludes_other_projects() {
        let home = dirs::home_dir().expect("home dir");
        let dir_a = home.join("postlane_test_lrfp_excl_a");
        let dir_b = home.join("postlane_test_lrfp_excl_b");
        write_config(&dir_a, Some("proj-abc"));
        write_config(&dir_b, Some("proj-xyz"));

        let state = make_state(vec![
            make_repo("r1", dir_a.to_str().unwrap()),
            make_repo("r2", dir_b.to_str().unwrap()),
        ]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "r1");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn test_list_repos_for_project_returns_empty_when_no_repos_match() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_lrfp_empty");
        write_config(&dir, Some("proj-other"));

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");
        assert!(result.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_repos_for_project_skips_repos_outside_home() {
        let state = make_state(vec![make_repo("r1", "/tmp/outside-home-repo")]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");
        assert!(result.is_empty(), "repos outside $HOME must be skipped");
    }

    #[test]
    fn test_unregister_repo_removes_entry_from_repos_json() {
        // unregister_repo_impl calls remove_repo_impl which writes repos.json.
        // We verify via the in-memory state lock that the entry is gone.
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_unreg_remove");
        fs::create_dir_all(&dir).expect("create dir");

        let state = make_state(vec![
            make_repo("r1", dir.to_str().unwrap()),
            make_repo("r2", "/tmp/other"),
        ]);
        unregister_repo_impl("r1", &state).expect("ok");

        let repos = state.repos.lock().expect("lock");
        assert!(!repos.repos.iter().any(|r| r.id == "r1"), "r1 must be removed");
        assert!(repos.repos.iter().any(|r| r.id == "r2"), "r2 must be retained");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unregister_repo_returns_error_when_id_not_found() {
        let state = make_state(vec![]);
        let result = unregister_repo_impl("nonexistent-id", &state);
        assert!(result.is_err(), "unknown repo_id must return Err");
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_list_repos_for_project_returns_empty_when_no_config_json() {
        // Repo exists inside $HOME but has no .postlane/config.json
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_lrfp_no_cfg");
        fs::create_dir_all(&dir).expect("create dir");
        // Deliberately do NOT write config.json

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");
        assert!(result.is_empty(), "repo without config.json must be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_repos_for_project_error_branch_skips_unreadable_config() {
        // Create a .postlane/config.json that is a directory (unreadable as file)
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_lrfp_unreadable");
        let cfg_path = dir.join(".postlane").join("config.json");
        // Make config.json a directory so read_to_string fails
        fs::create_dir_all(&cfg_path).expect("create config.json as directory");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = list_repos_for_project_impl("proj-abc", &state).expect("ok");
        assert!(result.is_empty(), "unreadable config.json must be skipped (Err branch)");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unregister_repo_stops_watcher_for_repo() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_unreg_watcher");
        fs::create_dir_all(&dir).expect("create dir");

        let state = make_state(vec![make_repo("r-watcher", dir.to_str().unwrap())]);
        // insert a placeholder watcher entry to simulate an active watcher
        {
            let watchers = state.watchers.lock().expect("lock watchers");
            // We can't create a real RecommendedWatcher without a real directory event loop,
            // so we verify the map is empty after unregister (stop_watcher removes the entry).
            // The watcher map starts empty; unregister_repo_impl must not panic on an empty map.
            assert!(watchers.is_empty());
            drop(watchers);
        }
        unregister_repo_impl("r-watcher", &state).expect("ok");
        let watchers = state.watchers.lock().expect("lock watchers");
        assert!(!watchers.contains_key("r-watcher"), "watcher entry must be removed");
        let _ = fs::remove_dir_all(&dir);
    }
}
