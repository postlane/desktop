// SPDX-License-Identifier: BUSL-1.1

//! `discover_child_repos` Tauri command (checklist 24.3.2) — wraps the
//! existing bare `workspace::discover_child_repos` (one level deep, no
//! `Result`/naming semantics) with `ChildRepo`/`Result` output and
//! deduplicated `posts_dir` assignment, reusing
//! `workspace_repos::assign_posts_dir` (the same v1.4 dedup algorithm
//! `add_workspace` already uses) rather than reimplementing it.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ChildRepo {
    pub name: String,
    pub path: String,
    pub posts_dir: String,
}

/// Assigns a deduplicated `posts_dir` to each path in `child_paths`, in order,
/// via `workspace_repos::assign_posts_dir` -- the same v1.4 algorithm
/// `add_workspace` uses (basename, then `-2`, `-3`, ... on collision).
pub fn assign_child_repo_posts_dirs(child_paths: &[PathBuf]) -> Vec<ChildRepo> {
    let mut entries: Vec<crate::workspace_repos::RepoEntry> = Vec::new();
    let mut repos = Vec::with_capacity(child_paths.len());
    for path in child_paths {
        let posts_dir = crate::workspace_repos::assign_posts_dir(path, &entries);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("repo")
            .to_string();
        let path_str = path.to_string_lossy().to_string();

        entries.push(crate::workspace_repos::RepoEntry {
            id: String::new(),
            name: name.clone(),
            path: path_str.clone(),
            posts_dir: posts_dir.clone(),
            active: true,
            added_at: String::new(),
        });
        repos.push(ChildRepo { name, path: path_str, posts_dir });
    }
    repos
}

pub fn discover_child_repos_impl(workspace_path: &Path) -> Result<Vec<ChildRepo>, String> {
    let child_paths = crate::workspace::discover_child_repos(workspace_path);
    if child_paths.is_empty() {
        return Err(
            "No Git repositories found in this folder. Select a folder that contains one or more Git repos."
                .to_string(),
        );
    }
    Ok(assign_child_repo_posts_dirs(&child_paths))
}

#[tauri::command]
pub fn discover_child_repos(path: String) -> Result<Vec<ChildRepo>, String> {
    discover_child_repos_impl(Path::new(&path))
}

#[cfg(test)]
#[path = "child_repo_discovery_tests.rs"]
mod tests;
