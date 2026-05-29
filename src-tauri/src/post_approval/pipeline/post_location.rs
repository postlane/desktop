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
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    if repos.repos.iter().any(|r| r.path == canonical_str) {
        return Ok(PostLocation::Legacy { canonical: canonical_str });
    }

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

    Err(format!("Repo '{}' is not registered", canonical_str))
}
