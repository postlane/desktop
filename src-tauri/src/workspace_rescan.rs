// SPDX-License-Identifier: BUSL-1.1

//! Rescan workspace for newly added or removed repos (22.3.16).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::workspace_repos::RepoEntry;

/// Result of a workspace rescan operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RescanResult {
    /// Names of repos that were newly discovered and added.
    pub added: Vec<String>,
    /// Names of repos that are no longer present on disk and were deactivated.
    pub deactivated: Vec<String>,
    /// Names of repos that were already present and remain active.
    pub unchanged: Vec<String>,
}

/// Returns paths from `discovered` that have no matching entry in `existing`
/// (by path string). Skips paths that already have an entry, whether active or
/// inactive, to avoid re-adding previously deactivated repos.
fn find_new_paths(discovered: &[PathBuf], existing: &[RepoEntry]) -> Vec<PathBuf> {
    discovered
        .iter()
        .filter(|p| {
            let path_str = p.to_string_lossy();
            !existing.iter().any(|r| r.path == path_str.as_ref())
        })
        .cloned()
        .collect()
}

/// Creates a new `RepoEntry` for `path`, assigning a unique `posts_dir` value
/// by consulting `all_entries` (which should include any entries already
/// appended in the current batch).
///
/// Uses `uuid::Uuid::new_v4()` for `id` and `chrono::Utc::now().to_rfc3339()`
/// for `added_at`.
fn build_new_entry(path: &Path, all_entries: &[RepoEntry]) -> Result<RepoEntry, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("could not derive repo name from path '{}'", path.display()))?
        .to_string();

    let path_str = path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 path '{}' cannot be stored", path.display()))?
        .to_string();

    let posts_dir = crate::workspace_repos::assign_posts_dir(path, all_entries);

    Ok(RepoEntry {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        path: path_str,
        posts_dir,
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    })
}

/// Main implementation: scans the workspace directory and updates
/// `{workspace}/repos.json` to reflect the current state on disk.
///
/// - Newly discovered git repos (not in `repos.json` at all) are added as
///   active entries.
/// - Repos whose path no longer exists on disk are marked `active: false`.
/// - Previously deactivated repos are never re-added even if the path
///   reappears on disk.
pub fn rescan_workspace_impl(repos_path: &Path, workspace_id: &str) -> Result<RescanResult, String> {
    let workspace_path = crate::workspace_confirm::find_workspace_path(repos_path, workspace_id)?;
    let ws_path = PathBuf::from(&workspace_path);

    let discovered = crate::workspace::discover_child_repos(&ws_path);
    let discovered_strs: std::collections::HashSet<String> = discovered
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    let ws_repos_path = ws_path.join("repos.json");
    let mut existing = crate::workspace_repos::read_workspace_repos(&ws_repos_path)?;

    let new_paths = find_new_paths(&discovered, &existing.repos);

    let mut added_names: Vec<String> = Vec::new();
    for path in &new_paths {
        let entry = build_new_entry(path, &existing.repos)?;
        added_names.push(entry.name.clone());
        existing.repos.push(entry);
    }

    let mut deactivated_names: Vec<String> = Vec::new();
    let mut unchanged_names: Vec<String> = Vec::new();
    for entry in &mut existing.repos {
        if added_names.contains(&entry.name) {
            continue;
        }
        if entry.active && !discovered_strs.contains(&entry.path) {
            entry.active = false;
            deactivated_names.push(entry.name.clone());
        } else if entry.active && discovered_strs.contains(&entry.path) {
            unchanged_names.push(entry.name.clone());
        }
    }

    crate::workspace_repos::write_workspace_repos(&ws_repos_path, &existing)?;

    Ok(RescanResult {
        added: added_names,
        deactivated: deactivated_names,
        unchanged: unchanged_names,
    })
}

/// Tauri command: rescans the workspace for new or removed repos and updates
/// `{workspace}/repos.json`. Starts file watchers for any newly added repos.
#[tauri::command]
pub fn rescan_workspace(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<RescanResult, String> {
    let result = rescan_workspace_impl(&state.repos_path, &workspace_id)?;

    if !result.added.is_empty() {
        let workspace_path =
            crate::workspace_confirm::find_workspace_path(&state.repos_path, &workspace_id)?;
        let ws_repos_path = PathBuf::from(&workspace_path).join("repos.json");
        let ws_repos = crate::workspace_repos::read_workspace_repos(&ws_repos_path)?;
        for entry in &ws_repos.repos {
            if result.added.contains(&entry.name) {
                crate::repo_mgmt::start_repo_watcher(&entry.id, &entry.path, &state, app.clone());
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
#[path = "workspace_rescan_tests.rs"]
mod tests;
