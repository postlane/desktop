// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::draft_post_scanner::{drafts_from_posts_dir, drafts_from_repo_path, project_id_from_config};
use crate::project_config_ops::read_project_id_from_path_impl;
use crate::storage::{Repo, ReposConfig};
use crate::types::Post;
use crate::workspace_entry::WorkspaceEntry;
use crate::workspace_repos::read_workspace_repos;
use std::path::{Path, PathBuf};
use tauri::State;

pub type DraftPost = Post;

/// Scans drafts from a workspace registration (v2 `workspaces` array entry).
///
/// Reads `{workspace}/repos.json`, then for each active `RepoEntry` scans
/// `{workspace}/posts/{posts_dir}/` — the v1.4 canonical draft location (22.2.2).
fn drafts_from_workspace_entry(workspace: &WorkspaceEntry) -> Vec<Post> {
    let ws_path = Path::new(&workspace.workspace_path);
    let repos_json_path = ws_path.join("repos.json");
    let ws_repos = match read_workspace_repos(&repos_json_path) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[draft_queries] failed to read {}: {}", repos_json_path.display(), e);
            return vec![];
        }
    };

    ws_repos
        .repos
        .iter()
        .filter(|e| e.active)
        .flat_map(|entry| {
            let posts_subdir = ws_path.join("posts").join(&entry.posts_dir);
            if !posts_subdir.exists() {
                return vec![];
            }
            let eff_config = crate::workspace::effective_config_path(
                Path::new(&entry.path),
                ws_path,
            );
            let project_id = project_id_from_config(&eff_config);

            // Use a synthetic Repo so drafts_from_posts_dir sets repo_name + repo_id correctly.
            let synthetic_repo = Repo {
                id: entry.id.clone(),
                name: entry.name.clone(),
                path: entry.path.clone(),
                active: true,
                added_at: entry.added_at.clone(),
            };
            drafts_from_posts_dir(&synthetic_repo, &posts_subdir, project_id)
        })
        .collect()
}

/// Scans drafts from a legacy per-repo entry (v1 `repos` array).
fn drafts_from_workspace(workspace_repo: &Repo, repos: &ReposConfig) -> Vec<Post> {
    let workspace_path = PathBuf::from(&workspace_repo.path);
    let registered: std::collections::HashSet<&str> = repos
        .repos
        .iter()
        .filter(|r| r.id != workspace_repo.id)
        .map(|r| r.path.as_str())
        .collect();

    crate::workspace::discover_child_repos(&workspace_path)
        .into_iter()
        .filter(|child| !registered.contains(child.to_str().unwrap_or("")))
        .flat_map(|child| {
            let eff_config = crate::workspace::effective_config_path(&child, &workspace_path);
            let project_id = project_id_from_config(&eff_config);
            drafts_from_repo_path(workspace_repo, &child, project_id)
        })
        .collect()
}

fn drafts_from_repo(repo: &Repo, repos: &ReposConfig) -> Vec<Post> {
    if crate::workspace::is_workspace_root(std::path::Path::new(&repo.path)) {
        return drafts_from_workspace(repo, repos);
    }
    let project_id = read_project_id_from_path_impl(&repo.path, repos).ok().flatten();
    drafts_from_repo_path(repo, std::path::Path::new(&repo.path), project_id)
}

/// Maximum number of draft posts returned by a single `get_all_drafts` call.
/// Prevents UI-thread blocking on repos with large numbers of accumulated drafts.
pub const MAX_DRAFT_PAGE: usize = 50;

/// Returns up to [`MAX_DRAFT_PAGE`] draft posts across all active repos and workspaces.
///
/// Sorted deterministically: `repo_path` → `post_folder` → `platform`.
pub fn get_all_drafts_impl(state: &AppState) -> Result<Vec<Post>, String> {
    let repos = state.lock_repos()?;

    // v2 workspace entries (22.2.2)
    let mut drafts: Vec<Post> = repos
        .workspaces
        .iter()
        .filter(|w| w.active)
        .flat_map(drafts_from_workspace_entry)
        .collect();

    // Legacy per-repo entries (backward compat)
    let legacy: Vec<Post> = repos
        .repos
        .iter()
        .filter(|r| r.active)
        .flat_map(|repo| drafts_from_repo(repo, &repos))
        .collect();

    drafts.extend(legacy);
    drafts.sort_by(|a, b| {
        a.repo_path
            .cmp(&b.repo_path)
            .then(a.post_folder.cmp(&b.post_folder))
            .then(a.platform.cmp(&b.platform))
    });
    drafts.truncate(MAX_DRAFT_PAGE);
    Ok(drafts)
}

/// Tauri command — returns up to 50 draft posts across all active repos.
#[tauri::command]
pub fn get_all_drafts(state: State<'_, AppState>) -> Result<Vec<Post>, String> {
    get_all_drafts_impl(&state)
}

/// Tauri command — returns the total count of draft posts across all active repos.
#[tauri::command]
pub fn get_all_drafts_count(state: State<'_, AppState>) -> Result<usize, String> {
    let repos = state.lock_repos()?;

    let workspace_count: usize = repos
        .workspaces
        .iter()
        .filter(|w| w.active)
        .flat_map(drafts_from_workspace_entry)
        .count();

    let legacy_count: usize = repos
        .repos
        .iter()
        .filter(|r| r.active)
        .flat_map(|repo| drafts_from_repo(repo, &repos))
        .count();

    Ok(workspace_count + legacy_count)
}

#[cfg(test)]
#[path = "draft_queries_tests.rs"]
mod tests;
