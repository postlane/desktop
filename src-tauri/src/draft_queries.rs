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
            // 22.1.3: project_id is workspace-level — always read from the workspace
            // config, never from a child repo's own config.json (which may lack the field).
            let project_id = project_id_from_config(&ws_path.join("config.json"));

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

/// 22.1.3 — project_id for workspace child repos must come from the workspace
/// config, not the child repo's own config.json (which may have no project_id).
#[cfg(test)]
mod project_id_tests {
    use super::*;
    use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

    fn write_workspace_draft_simple(ws: &std::path::Path, posts_dir: &str, folder: &str, platform: &str) {
        let p = ws.join("posts").join(posts_dir).join(folder);
        std::fs::create_dir_all(&p).expect("create post dir");
        std::fs::write(p.join(format!("{}.md", platform)), "content").expect("write md");
        std::fs::write(p.join("meta.json"), "{}").expect("write meta");
    }

    /// When a workspace child repo has its own .postlane/config.json with no
    /// project_id, the draft must still carry the workspace's project_id.
    #[test]
    fn drafts_from_workspace_entry_uses_workspace_project_id_when_child_config_has_none() {
        let ws = tempfile::TempDir::new().expect("ws dir");
        let child = ws.path().join("repo-a");
        std::fs::create_dir_all(child.join(".postlane")).expect("create .postlane");
        // Child has its own config with NO project_id (simulates `postlane init` output).
        std::fs::write(
            child.join(".postlane/config.json"),
            r#"{"version":1,"llm":{"provider":"anthropic"}}"#,
        ).expect("write child config");
        // Workspace config has the authoritative project_id.
        std::fs::write(
            ws.path().join("config.json"),
            r#"{"project_id":"ws-proj-abc","schema_version":4}"#,
        ).expect("write ws config");

        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "child-id".to_string(), name: "repo-a".to_string(),
                path: child.to_str().unwrap().to_string(),
                posts_dir: "repo-a".to_string(),
                active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&ws.path().join("repos.json"), &ws_repos).expect("write ws repos");
        write_workspace_draft_simple(ws.path(), "repo-a", "my-post", "bluesky");

        let ws_entry = crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
            id: "ws-proj-abc".to_string(), name: "ws".to_string(),
            workspace_path: ws.path().to_str().unwrap().to_string(),
            active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let drafts = drafts_from_workspace_entry(&ws_entry);

        assert_eq!(drafts.len(), 1, "draft must appear");
        assert_eq!(
            drafts[0].project_id.as_deref(), Some("ws-proj-abc"),
            "project_id must come from workspace config, not child config",
        );
    }

    /// 22.10.8 — child repo whose config.json contains a different project_id must
    /// still use the workspace project_id in the queue (22.1.3 invariant).
    #[test]
    fn drafts_from_workspace_entry_ignores_conflicting_child_project_id() {
        let ws = tempfile::TempDir::new().expect("ws dir");
        let child = ws.path().join("repo-b");
        std::fs::create_dir_all(child.join(".postlane")).expect("create .postlane");
        // Child has its own project_id that differs from the workspace.
        std::fs::write(
            child.join(".postlane/config.json"),
            r#"{"project_id":"child-different-proj","schema_version":1}"#,
        ).expect("write child config");
        std::fs::write(
            ws.path().join("config.json"),
            r#"{"project_id":"ws-proj-xyz","schema_version":4}"#,
        ).expect("write ws config");

        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "child-b-id".to_string(), name: "repo-b".to_string(),
                path: child.to_str().unwrap().to_string(),
                posts_dir: "repo-b".to_string(),
                active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&ws.path().join("repos.json"), &ws_repos).expect("write ws repos");
        write_workspace_draft_simple(ws.path(), "repo-b", "child-post", "bluesky");

        let ws_entry = crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
            id: "ws-proj-xyz".to_string(), name: "ws".to_string(),
            workspace_path: ws.path().to_str().unwrap().to_string(),
            active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let drafts = drafts_from_workspace_entry(&ws_entry);

        assert_eq!(drafts.len(), 1, "draft must appear");
        assert_eq!(
            drafts[0].project_id.as_deref(), Some("ws-proj-xyz"),
            "workspace project_id must win over child's conflicting project_id",
        );
    }
}
