// SPDX-License-Identifier: BUSL-1.1

//! `PostLocation` — resolves where a post's files are stored for workspace vs legacy repos.

use crate::app_state::AppState;

/// Where a post's files are located — determines path resolution in `approve_post_impl`.
#[derive(Debug)]
pub enum PostLocation {
    /// Legacy per-repo layout: post at `{canonical}/.postlane/posts/{folder}/`.
    Legacy { canonical: String },
    /// Workspace layout: post at `{workspace}/posts/{posts_dir}/{folder}/` (22.2.7).
    Workspace {
        canonical: String,
        workspace_path: String,
        posts_dir: String,
        repo_name: String,
    },
}

impl PostLocation {
    /// Returns the post folder path for this location.
    pub fn posts_base(&self, post_folder: &str) -> std::path::PathBuf {
        match self {
            Self::Legacy { canonical } => {
                std::path::Path::new(canonical).join(".postlane/posts").join(post_folder)
            }
            Self::Workspace { workspace_path, posts_dir, .. } => {
                std::path::Path::new(workspace_path).join("posts").join(posts_dir).join(post_folder)
            }
        }
    }

    pub fn canonical(&self) -> &str {
        match self {
            Self::Legacy { canonical } | Self::Workspace { canonical, .. } => canonical,
        }
    }

    /// The root path to use for reading scheduler config and account IDs.
    /// Legacy posts: the canonical repo path (config lives at `{root}/.postlane/config.json`).
    /// Workspace posts: the workspace path (config lives at `{root}/config.json`).
    pub fn config_root(&self) -> &str {
        match self {
            Self::Legacy { canonical } => canonical,
            Self::Workspace { workspace_path, .. } => workspace_path,
        }
    }
}

/// Validates `repo_path` and resolves the post location (22.2.7).
///
/// Accepts legacy per-repo paths (`repos.repos[]`) and workspace child repo paths
/// (found via `{workspace}/repos.json` entries). Returns `PostLocation` so callers
/// can derive the correct post file path for each layout.
pub fn validate_repo_path(repo_path: &str, state: &AppState) -> Result<PostLocation, String> {
    let canonical = std::fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize repo path '{}': {}", repo_path, e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or("Repo path contains non-UTF-8 characters")?
        .to_string();
    let repos = state.lock_repos()?;

    // Check workspace children first — workspace takes precedence over legacy
    // registrations when a repo appears in both (workspace is the current model).
    for ws_entry in repos.workspaces.iter().filter(|w| w.active) {
        let ws_repos_path = std::path::Path::new(&ws_entry.workspace_path).join("repos.json");
        let ws_repos = match crate::workspace_repos::read_workspace_repos(&ws_repos_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let Some(entry) = ws_repos.repos.iter().find(|e| e.path == canonical_str) {
            return Ok(PostLocation::Workspace {
                canonical: canonical_str,
                workspace_path: ws_entry.workspace_path.clone(),
                posts_dir: entry.posts_dir.clone(),
                repo_name: entry.name.clone(),
            });
        }
    }

    if repos.repos.iter().any(|r| r.path == canonical_str) {
        return Ok(PostLocation::Legacy { canonical: canonical_str });
    }

    Err(format!("Repo '{}' is not registered", canonical_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Repo, ReposConfig};
    use crate::workspace_entry::WorkspaceEntry;
    use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

    fn make_state_with_both(
        ws_path: &std::path::Path,
        child_path: &str,
        posts_dir: &str,
    ) -> AppState {
        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "child-id".to_string(),
                name: "child".to_string(),
                path: child_path.to_string(),
                posts_dir: posts_dir.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&ws_path.join("repos.json"), &ws_repos).expect("write ws repos");

        let repos_config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: "proj-1".to_string(),
                name: "ws".to_string(),
                workspace_path: ws_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![crate::storage::Repo {
                id: "legacy-id".to_string(),
                name: "child".to_string(),
                path: child_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        crate::app_state::AppState::new_with_path(repos_config, tmp.path().to_path_buf())
    }

    /// When a repo appears in both the legacy `repos` array and a workspace's `repos.json`,
    /// the workspace location must be returned. Legacy must not shadow workspace.
    #[test]
    fn validate_repo_path_prefers_workspace_over_legacy_when_in_both() {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        let child_dir = tmp.path().join("repo-a");
        std::fs::create_dir_all(&child_dir).expect("create child dir");
        let child_path = std::fs::canonicalize(&child_dir)
            .expect("canonicalize")
            .to_str()
            .unwrap()
            .to_string();
        let state = make_state_with_both(tmp.path(), &child_path, "repo-a");
        let result = validate_repo_path(&child_path, &state).expect("must resolve");

        match result {
            PostLocation::Workspace { posts_dir, .. } => {
                assert_eq!(posts_dir, "repo-a",
                    "posts_dir must come from workspace repos.json");
            }
            PostLocation::Legacy { canonical } => {
                panic!(
                    "workspace must take precedence over legacy, but got Legacy({})",
                    canonical
                );
            }
        }
    }

    #[test]
    fn test_workspace_config_root_is_workspace_path() {
        let loc = PostLocation::Workspace {
            canonical: "/ws/repo-a".to_string(),
            workspace_path: "/ws".to_string(),
            posts_dir: "repo-a".to_string(),
            repo_name: "repo-a".to_string(),
        };
        assert_eq!(loc.config_root(), "/ws");
    }

    #[test]
    fn test_legacy_config_root_is_canonical() {
        let loc = PostLocation::Legacy { canonical: "/repos/my-repo".to_string() };
        assert_eq!(loc.config_root(), "/repos/my-repo");
    }

    // --- §posts_base ---

    #[test]
    fn test_posts_base_legacy_constructs_postlane_posts_path() {
        let loc = PostLocation::Legacy { canonical: "/repos/my-repo".to_string() };
        let result = loc.posts_base("my-post-2026");
        assert_eq!(
            result,
            std::path::PathBuf::from("/repos/my-repo/.postlane/posts/my-post-2026"),
            "legacy posts_base must join canonical + .postlane/posts + folder"
        );
    }

    #[test]
    fn test_posts_base_workspace_constructs_workspace_posts_path() {
        let loc = PostLocation::Workspace {
            canonical: "/ws/repo-a".to_string(),
            workspace_path: "/ws".to_string(),
            posts_dir: "repo-a".to_string(),
            repo_name: "repo-a".to_string(),
        };
        let result = loc.posts_base("my-post-2026");
        assert_eq!(
            result,
            std::path::PathBuf::from("/ws/posts/repo-a/my-post-2026"),
            "workspace posts_base must join workspace_path + posts + posts_dir + folder"
        );
    }

    // --- §validate_repo_path workspace repos.json error path ---

    /// When a workspace entry's repos.json cannot be read, validation must continue
    /// and fall through to the legacy repos check (line 74: `Err(_) => continue`).
    #[test]
    fn test_validate_repo_path_falls_through_to_legacy_when_workspace_repos_json_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Workspace dir WITHOUT a repos.json — triggers the Err(_) => continue branch
        let ws_dir = dir.path().join("workspace");
        std::fs::create_dir_all(&ws_dir).expect("create ws dir");
        // Child repo dir — registered as a legacy repo only
        let repo_dir = dir.path().join("repo-a");
        std::fs::create_dir_all(&repo_dir).expect("create repo dir");
        let canonical_path = std::fs::canonicalize(&repo_dir)
            .expect("canonicalize")
            .to_str()
            .unwrap()
            .to_string();

        let repos_config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: "ws-1".to_string(),
                name: "ws".to_string(),
                workspace_path: ws_dir.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "repo-a".to_string(),
                path: canonical_path.clone(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        let tmp = tempfile::NamedTempFile::new().expect("tmp file");
        let state = crate::app_state::AppState::new_with_path(repos_config, tmp.path().to_path_buf());

        let result = validate_repo_path(&canonical_path, &state)
            .expect("must resolve via legacy fallback when workspace repos.json is missing");
        match result {
            PostLocation::Legacy { canonical } => {
                assert_eq!(canonical, canonical_path,
                    "legacy canonical must match the registered path");
            }
            PostLocation::Workspace { .. } => {
                panic!("must resolve as Legacy when workspace repos.json is missing, not Workspace");
            }
        }
    }
}
