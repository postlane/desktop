// SPDX-License-Identifier: BUSL-1.1

//! Workspace path existence check (22.3.21) and rename detection (22.3.22).
//!
//! Called once at app launch via `check_workspace_paths`: checks each workspace
//! entry in repos.json, starts file watchers for valid paths, and returns per-workspace
//! status so the frontend can show banners for missing or renamed workspaces.

use crate::workspace_entry::WorkspaceEntry;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A sibling directory that matches the stored project_id and could be the
/// renamed workspace folder.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RenamedCandidate {
    /// Absolute path to the candidate directory.
    pub path: String,
    /// Basename of the candidate directory.
    pub name: String,
    /// Directory mtime as Unix seconds. Used by the frontend to pre-select the
    /// most recently modified candidate when multiple are found.
    pub modified_secs: u64,
}

/// Per-workspace path check result.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "tag", rename_all = "snake_case")]
pub enum WorkspacePathStatus {
    /// Workspace path exists on disk. Watcher has been started.
    Ok,
    /// Workspace path does not exist but one or more nearby siblings match the
    /// stored project_id. The user must confirm before repos.json is updated.
    Renamed { candidates: Vec<RenamedCandidate> },
    /// Workspace path does not exist and no nearby candidates were found.
    Missing,
}

/// Result of checking a single workspace entry.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkspaceCheckResult {
    pub workspace_id: String,
    /// The path stored in repos.json (may no longer exist on disk).
    pub workspace_path: String,
    pub workspace_name: String,
    pub status: WorkspacePathStatus,
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Scans immediate children of `parent_dir` for directories containing
/// `config.json` with a matching `project_id`. Skips `original_path`.
/// Bounded to 50 entries to prevent unexpectedly large directory scans.
fn scan_siblings(parent_dir: &Path, project_id: &str, original_path: &Path) -> Vec<RenamedCandidate> {
    let Ok(entries) = std::fs::read_dir(parent_dir) else {
        return vec![];
    };
    let mut candidates = Vec::new();
    for entry in entries.take(50).flatten() {
        let path = entry.path();
        if !path.is_dir() || path == original_path {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path.join("config.json")) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            continue;
        };
        if json["project_id"].as_str() == Some(project_id) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let modified_secs = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                candidates.push(RenamedCandidate {
                    path: path.to_string_lossy().into_owned(),
                    name: name.to_string(),
                    modified_secs,
                });
            }
        }
    }
    candidates
}

/// Checks whether a single workspace entry's path still exists. If not, scans
/// sibling directories for a candidate with matching project_id.
pub fn check_workspace_path(entry: &WorkspaceEntry) -> WorkspaceCheckResult {
    let path = Path::new(&entry.workspace_path);
    if path.exists() {
        return WorkspaceCheckResult {
            workspace_id: entry.id.clone(),
            workspace_path: entry.workspace_path.clone(),
            workspace_name: entry.name.clone(),
            status: WorkspacePathStatus::Ok,
        };
    }
    let candidates = path
        .parent()
        .filter(|p| p.exists())
        .map(|parent| scan_siblings(parent, &entry.id, path))
        .unwrap_or_default();

    if candidates.len() > 1 {
        log::warn!(
            "[workspace_path_check] {} rename candidates found for workspace '{}' ({})",
            candidates.len(),
            entry.id,
            entry.workspace_path,
        );
    } else if candidates.len() == 1 {
        log::info!(
            "[workspace_path_check] rename candidate for workspace '{}': {}",
            entry.id,
            candidates[0].path,
        );
    }

    let status = if candidates.is_empty() {
        WorkspacePathStatus::Missing
    } else {
        WorkspacePathStatus::Renamed { candidates }
    };
    WorkspaceCheckResult {
        workspace_id: entry.id.clone(),
        workspace_path: entry.workspace_path.clone(),
        workspace_name: entry.name.clone(),
        status,
    }
}

/// Checks all workspace entries in repos.json. Called once at app launch.
pub fn check_all_workspace_paths_impl(repos_path: &Path) -> Vec<WorkspaceCheckResult> {
    let config = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| crate::storage::ReposConfig::default());
    config
        .workspaces
        .iter()
        .filter(|w| w.active)
        .map(check_workspace_path)
        .collect()
}

/// Updates `workspace_path` and `name` for the given workspace in repos.json.
/// The new name is derived from `basename(new_path)`.
/// Does NOT start file watchers — the caller is responsible for that.
pub fn update_workspace_path_impl(
    repos_path: &Path,
    workspace_id: &str,
    new_path: &str,
) -> Result<(), String> {
    let mut config = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| crate::storage::ReposConfig::default());
    let entry = config
        .workspaces
        .iter_mut()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| format!("workspace '{}' not found in repos.json", workspace_id))?;
    let name = Path::new(new_path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("invalid workspace path: {}", new_path))?
        .to_string();
    entry.workspace_path = new_path.to_string();
    entry.name = name;
    crate::storage::write_repos(repos_path, &config)
        .map_err(|e| format!("failed to write repos.json: {:?}", e))
}

/// Validates that the directory at `folder_path` has a `config.json` whose
/// `project_id` matches the stored workspace entry. Returns `PL-WS-002` on
/// mismatch so the frontend can surface a specific error message.
pub fn validate_workspace_folder_impl(
    repos_path: &Path,
    workspace_id: &str,
    folder_path: &str,
) -> Result<(), String> {
    let config = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| crate::storage::ReposConfig::default());
    let entry = config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| format!("workspace '{}' not found in repos.json", workspace_id))?;
    let config_path = Path::new(folder_path).join("config.json");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|_| "PL-WS-002: no config.json found in selected folder".to_string())?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|_| "PL-WS-002: config.json is not valid JSON".to_string())?;
    let folder_project_id = json["project_id"]
        .as_str()
        .ok_or_else(|| "PL-WS-002: config.json missing project_id field".to_string())?;
    if folder_project_id != entry.id {
        return Err("PL-WS-002: This folder belongs to a different Postlane project".to_string());
    }
    Ok(())
}

/// Starts file watchers for all active workspace paths that exist on disk.
/// Called from `check_workspace_paths` — do not call separately on startup.
pub fn start_valid_workspace_watchers(
    workspaces: &[WorkspaceEntry],
    state: &crate::app_state::AppState,
    handle: tauri::AppHandle,
) {
    for ws in workspaces {
        if !ws.active {
            continue;
        }
        if Path::new(&ws.workspace_path).exists() {
            crate::repo_mgmt::start_repo_watcher(&ws.id, &ws.workspace_path, state, handle.clone());
        }
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Checks all workspace paths once at app launch.
/// Starts file watchers for workspaces whose paths exist on disk.
/// Returns per-workspace status so the frontend can render banners.
#[tauri::command]
pub fn check_workspace_paths(
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Vec<WorkspaceCheckResult> {
    let results = check_all_workspace_paths_impl(&state.repos_path);
    let workspaces = state
        .repos
        .lock()
        .map(|r| r.workspaces.clone())
        .unwrap_or_default();
    start_valid_workspace_watchers(&workspaces, &state, app);
    results
}

/// Updates `workspace_path` in repos.json after the user confirms a rename
/// in the banner (22.3.22). Restarts the file watcher on the new path.
/// Emits `workspace_path_recovered` telemetry with `method: "auto"` (22.9.11).
#[tauri::command]
pub fn update_workspace_path(
    workspace_id: String,
    new_path: String,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    update_workspace_path_impl(&state.repos_path, &workspace_id, &new_path)?;
    crate::repo_mgmt::start_repo_watcher(&workspace_id, &new_path, &state, app);
    Ok(())
}

/// Opens a folder picker, validates the selected folder's `project_id`, and
/// updates `workspace_path` in repos.json. Called by the "Locate folder" banner
/// action (22.3.24). Emits `workspace_path_recovered` with `method: "manual"` (22.9.11).
#[tauri::command]
pub fn locate_workspace_folder(
    workspace_id: String,
    folder_path: String,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    validate_workspace_folder_impl(&state.repos_path, &workspace_id, &folder_path)?;
    update_workspace_path_impl(&state.repos_path, &workspace_id, &folder_path)?;
    crate::repo_mgmt::start_repo_watcher(&workspace_id, &folder_path, &state, app);
    Ok(())
}

/// Returns the workspace path for a given project_id, or None if not registered.
/// Used by Settings to display the voice guide path (22.3.22a).
#[tauri::command]
pub fn get_workspace_path(
    project_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Option<String> {
    let repos = state.repos.lock().ok()?;
    repos
        .workspaces
        .iter()
        .find(|w| w.id == project_id && w.active)
        .map(|w| w.workspace_path.clone())
}

#[cfg(test)]
#[path = "workspace_path_check_tests.rs"]
mod tests;
