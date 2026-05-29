// SPDX-License-Identifier: BUSL-1.1

//! Confirmation step for the "Add workspace" flow (22.3.4).
//!
//! After `add_workspace` writes ALL discovered repos to `{workspace}/repos.json`,
//! this command rewrites it with only the repos the user selected, then starts
//! the file watcher on `{workspace}/posts/`.

use std::path::Path;

/// Rewrites `{workspace}/repos.json` to contain only the entries whose `path`
/// is in `selected_paths`. Entries not in the selection are dropped. Entry
/// fields (id, posts_dir, added_at) are preserved from the existing file.
///
/// Finds the workspace path by looking up `workspace_id` in the global
/// `repos_path` (`~/.postlane/repos.json`).
pub fn confirm_workspace_repos_impl(
    repos_path: &Path,
    workspace_id: &str,
    selected_paths: &[String],
) -> Result<(), String> {
    let workspace_path = find_workspace_path(repos_path, workspace_id)?;
    let ws_path = std::path::PathBuf::from(&workspace_path);
    let ws_repos_path = ws_path.join("repos.json");

    let existing = crate::workspace_repos::read_workspace_repos(&ws_repos_path)?;
    let selected_set: std::collections::HashSet<&str> =
        selected_paths.iter().map(String::as_str).collect();
    let kept: Vec<_> = existing
        .repos
        .into_iter()
        .filter(|r| selected_set.contains(r.path.as_str()))
        .collect();

    let updated = crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: kept };
    crate::workspace_repos::write_workspace_repos(&ws_repos_path, &updated)
}

/// Tauri command: rewrites `{workspace}/repos.json` with only the selected repos,
/// then starts the file watcher on `{workspace}/posts/`.
#[tauri::command]
pub fn confirm_workspace_repos(
    workspace_id: String,
    selected_paths: Vec<String>,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    confirm_workspace_repos_impl(&state.repos_path, &workspace_id, &selected_paths)?;
    let workspace_path = find_workspace_path(&state.repos_path, &workspace_id)?;
    crate::repo_mgmt::start_repo_watcher(&workspace_id, &workspace_path, &state, app);
    Ok(())
}

/// Looks up the workspace root path for `workspace_id` in the global repos registry.
///
/// Returns an error if the workspace is not found.
pub(crate) fn find_workspace_path(repos_path: &Path, workspace_id: &str) -> Result<String, String> {
    let global = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| crate::storage::ReposConfig::default());
    global
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .map(|w| w.workspace_path.clone())
        .ok_or_else(|| format!("workspace '{}' not found in repos.json", workspace_id))
}

#[cfg(test)]
#[path = "workspace_confirm_tests.rs"]
mod tests;
