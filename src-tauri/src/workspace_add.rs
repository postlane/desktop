// SPDX-License-Identifier: BUSL-1.1

//! Desktop "Add workspace" flow (22.3).
//!
//! A workspace is a parent directory that contains one or more child git repos.
//! Workspace-level config files (`config.json`, `config.local.json`) live at
//! the workspace root — no `.postlane/` subdirectory is created inside the workspace.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Summary of a discovered child repo returned by `add_workspace_impl`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepoSummary {
    pub name: String,
    pub path: String,
    pub posts_dir: String,
}

/// Returned by `add_workspace_impl` and the `add_workspace` Tauri command.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceSetupResult {
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub discovered_repos: Vec<RepoSummary>,
}

/// Canonicalize `folder_path` and validate it is not inside the Postlane reserved
/// directory. Rejects symlinks that resolve into `postlane_dir` via canonicalization.
fn resolve_and_validate(folder_path: &Path, postlane_dir: &Path) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(folder_path)
        .map_err(|e| format!("failed to resolve path {}: {}", folder_path.display(), e))?;
    let canonical_postlane = std::fs::canonicalize(postlane_dir)
        .unwrap_or_else(|_| postlane_dir.to_path_buf());
    if canonical.starts_with(&canonical_postlane) {
        return Err(format!(
            "PathReserved: '{}' is inside the Postlane reserved directory — \
             cannot use as a workspace",
            canonical.display()
        ));
    }
    Ok(canonical)
}

/// Write `config.json` (skip if exists) and `config.local.json` with 0600 permissions
/// (skip if exists), then append `config.local.json` to `.gitignore`.
fn write_workspace_config_files(workspace: &Path, project_id: &str) -> Result<(), String> {
    let config_path = workspace.join("config.json");
    if !config_path.exists() {
        let config = crate::repo_init_config::build_initial_config_json(project_id);
        let bytes = serde_json::to_vec_pretty(&config)
            .map_err(|e| format!("failed to serialise config.json: {}", e))?;
        crate::init::atomic_write(&config_path, &bytes)
            .map_err(|e| format!("failed to write config.json: {}", e))?;
    }
    let local_config_path = workspace.join("config.local.json");
    if !local_config_path.exists() {
        let local_config = crate::repo_init_config::build_initial_config_local_json();
        let local_json = serde_json::to_string_pretty(&local_config)
            .map_err(|e| format!("failed to serialise config.local.json: {}", e))?;
        crate::config_local_write::write_workspace_local_config(workspace, &local_json)?;
    }
    crate::config_local_write::append_config_local_to_gitignore(workspace)
}

/// Build `RepoEntry` records from discovered child paths, assigning unique `posts_dir`
/// values, then write them to `{workspace}/repos.json`.
fn write_child_repos(
    workspace: &Path,
    child_paths: &[PathBuf],
) -> Result<Vec<crate::workspace_repos::RepoEntry>, String> {
    let mut entries: Vec<crate::workspace_repos::RepoEntry> = Vec::new();
    for child in child_paths {
        let posts_dir = crate::workspace_repos::assign_posts_dir(child, &entries);
        let name = child
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string();
        let path_str = child
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 path: {}", child.display()))?
            .to_string();
        entries.push(crate::workspace_repos::RepoEntry {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            path: path_str,
            posts_dir,
            active: true,
            added_at: chrono::Utc::now().to_rfc3339(),
        });
    }
    let config = crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: entries.clone() };
    crate::workspace_repos::write_workspace_repos(&workspace.join("repos.json"), &config)?;
    Ok(entries)
}

/// Add a workspace entry to the global `~/.postlane/repos.json` (idempotent by `project_id`).
fn register_workspace_globally(
    repos_path: &Path,
    project_id: &str,
    name: &str,
    workspace_path: &str,
) -> Result<(), String> {
    let mut global = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| crate::storage::ReposConfig::default());
    if !global.workspaces.iter().any(|w| w.id == project_id) {
        global.workspaces.push(crate::workspace_entry::WorkspaceEntry {
            id: project_id.to_string(),
            name: name.to_string(),
            workspace_path: workspace_path.to_string(),
            active: true,
            added_at: chrono::Utc::now().to_rfc3339(),
        });
        crate::storage::write_repos(repos_path, &global)
            .map_err(|e| format!("failed to update global repos.json: {:?}", e))?;
    }
    Ok(())
}

/// Pure implementation of the "Add workspace" flow. Testable without Tauri.
pub fn add_workspace_impl(
    folder_path: &Path,
    repos_path: &Path,
    postlane_dir: &Path,
    project_id: &str,
) -> Result<WorkspaceSetupResult, String> {
    let canonical = resolve_and_validate(folder_path, postlane_dir)?;

    if canonical.join(".git").is_dir() {
        return Err(format!(
            "PL-WS-003: '{}' is a Git repository. Select the parent folder that \
             contains your repositories instead, or use 'Add individual repository'.",
            canonical.display()
        ));
    }

    let child_paths = crate::workspace::discover_child_repos(&canonical);
    if child_paths.is_empty() {
        return Err(format!(
            "PL-WS-001: No Git repositories found in {}. Add repositories to \
             this folder and try again.",
            canonical.display()
        ));
    }

    write_workspace_config_files(&canonical, project_id)?;
    let repo_entries = write_child_repos(&canonical, &child_paths)?;
    crate::workspace_repos::create_workspace_dirs(&canonical)?;

    let workspace_name = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    let workspace_path_str = canonical
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 workspace path: {}", canonical.display()))?
        .to_string();
    register_workspace_globally(repos_path, project_id, &workspace_name, &workspace_path_str)?;

    let discovered_repos = repo_entries
        .iter()
        .map(|r| RepoSummary { name: r.name.clone(), path: r.path.clone(), posts_dir: r.posts_dir.clone() })
        .collect();

    Ok(WorkspaceSetupResult {
        workspace_id: project_id.to_string(),
        workspace_path: canonical,
        discovered_repos,
    })
}

/// Updates the in-memory `AppState.repos.workspaces` after `add_workspace_impl`
/// writes the workspace to `repos.json`. Without this call, `get_all_drafts_impl`
/// cannot find the workspace until the next app restart.
pub fn sync_workspace_to_app_state(
    result: &WorkspaceSetupResult,
    state: &crate::app_state::AppState,
) {
    let entry = crate::workspace_entry::WorkspaceEntry {
        id: result.workspace_id.clone(),
        name: result.workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string(),
        workspace_path: result.workspace_path
            .to_str()
            .unwrap_or("")
            .to_string(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Ok(mut repos) = state.lock_repos() {
        if !repos.workspaces.iter().any(|w| w.id == entry.id) {
            repos.workspaces.push(entry);
        }
    }
}

/// Tauri command: registers `folder_path` as a new workspace for `project_id`.
///
/// Does NOT start the file watcher — the caller must invoke a separate
/// confirmation command (22.3.4) which starts the watcher after the user
/// confirms (or deselects) the discovered repos.
pub(crate) fn record_workspace_created(
    state: &crate::app_state::AppState, consent: bool, repo_count: usize,
) {
    state.telemetry.record(consent, "workspace_created", serde_json::json!({ "repo_count": repo_count }));
}

#[tauri::command]
pub fn add_workspace(
    folder_path: String,
    project_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<WorkspaceSetupResult, String> {
    use crate::init::postlane_dir;
    let pl_dir = postlane_dir()?;
    let path = PathBuf::from(&folder_path);
    let result = add_workspace_impl(&path, &state.repos_path, &pl_dir, &project_id)?;
    sync_workspace_to_app_state(&result, &state);
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_workspace_created(&state, consent, result.discovered_repos.len());
    Ok(result)
}

#[cfg(test)]
#[path = "workspace_add_tests.rs"]
mod tests;
