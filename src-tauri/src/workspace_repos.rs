// SPDX-License-Identifier: BUSL-1.1

//! `{workspace}/repos.json` — workspace child-repo registry.
//!
//! Distinct from `~/.postlane/repos.json` (global desktop registry, owned by the desktop app).
//! This file is owned by the CLI and lists the child repos belonging to a workspace,
//! each with a `posts_dir` field that determines where its drafts are stored under
//! `{workspace}/posts/`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A child-repo entry in `{workspace}/repos.json`.
///
/// `posts_dir` is the canonical subdirectory under `{workspace}/posts/` where this
/// repo's drafts are stored. It defaults to `basename(path)` but is deduplicated at
/// registration time using `assign_posts_dir` — all path construction uses `posts_dir`,
/// never `basename(path)` at runtime.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RepoEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub posts_dir: String,
    pub active: bool,
    pub added_at: String,
}

/// Schema for `{workspace}/repos.json` (v1).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WorkspaceReposConfig {
    pub version: u32,
    pub repos: Vec<RepoEntry>,
}

/// Derives a unique `posts_dir` value for `repo_path` given the existing entries.
///
/// Returns `basename(repo_path)` if not already in use; otherwise appends `-2`, `-3`,
/// etc. until a unique value is found. Bounded at 1000 to prevent infinite loops on
/// pathological inputs.
pub fn assign_posts_dir(repo_path: &Path, existing_entries: &[RepoEntry]) -> String {
    let basename = repo_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo")
        .to_string();

    let used: std::collections::HashSet<&str> =
        existing_entries.iter().map(|e| e.posts_dir.as_str()).collect();

    if !used.contains(basename.as_str()) {
        return basename;
    }

    for n in 2..=1000 {
        let candidate = format!("{}-{}", basename, n);
        if !used.contains(candidate.as_str()) {
            return candidate;
        }
    }

    // Safety fallback — should never reach here in practice
    format!("{}-{}", basename, uuid::Uuid::new_v4().as_simple())
}

/// Reads `{workspace}/repos.json`. Returns an empty v1 config when the file is absent.
pub fn read_workspace_repos(path: &Path) -> Result<WorkspaceReposConfig, String> {
    if !path.exists() {
        return Ok(WorkspaceReposConfig { version: 1, repos: vec![] });
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {}", path.display(), e))
}

/// Writes `{workspace}/repos.json` atomically (tmp → rename).
pub fn write_workspace_repos(path: &Path, config: &WorkspaceReposConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("failed to serialise workspace repos: {}", e))?;
    crate::init::atomic_write(path, json.as_bytes())
        .map_err(|e| format!("failed to write {}: {}", path.display(), e))
}

/// Returns the posts directory for a workspace child repo.
/// Path: `{workspace_path}/posts/{posts_dir}`
pub fn workspace_posts_dir(workspace_path: &Path, posts_dir: &str) -> PathBuf {
    workspace_path.join("posts").join(posts_dir)
}

/// Creates `{workspace}/posts/` and `{workspace}/drafts/` eagerly (22.2.4).
/// Idempotent — safe to call even when the directories already exist.
pub fn create_workspace_dirs(workspace_path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(workspace_path.join("posts"))
        .map_err(|e| format!("failed to create {}/posts/: {}", workspace_path.display(), e))?;
    std::fs::create_dir_all(workspace_path.join("drafts"))
        .map_err(|e| format!("failed to create {}/drafts/: {}", workspace_path.display(), e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── 22.2.16 — assign_posts_dir ────────────────────────────────────────────

    #[test]
    fn test_assign_posts_dir_returns_basename_when_no_collision() {
        let path = Path::new("/code/myorg/frontend");
        let existing: Vec<RepoEntry> = vec![];
        assert_eq!(assign_posts_dir(path, &existing), "frontend");
    }

    #[test]
    fn test_assign_posts_dir_returns_basename_2_when_one_collision() {
        let path = Path::new("/code/myorg/frontend");
        let existing = vec![RepoEntry {
            id: "r1".to_string(),
            name: "frontend".to_string(),
            path: "/code/other/frontend".to_string(),
            posts_dir: "frontend".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }];
        assert_eq!(assign_posts_dir(path, &existing), "frontend-2");
    }

    #[test]
    fn test_assign_posts_dir_returns_basename_3_when_two_collisions() {
        let path = Path::new("/code/myorg/frontend");
        let existing = vec![
            RepoEntry {
                id: "r1".to_string(),
                name: "frontend".to_string(),
                path: "/a/frontend".to_string(),
                posts_dir: "frontend".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            },
            RepoEntry {
                id: "r2".to_string(),
                name: "frontend-2".to_string(),
                path: "/b/frontend".to_string(),
                posts_dir: "frontend-2".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            },
        ];
        assert_eq!(assign_posts_dir(path, &existing), "frontend-3");
    }

    #[test]
    fn test_assign_posts_dir_handles_path_with_no_filename() {
        let path = Path::new("/");
        let existing: Vec<RepoEntry> = vec![];
        // Falls back to "repo" when no basename
        assert_eq!(assign_posts_dir(path, &existing), "repo");
    }

    // ── RepoEntry + WorkspaceReposConfig roundtrip ────────────────────────────

    #[test]
    fn test_repo_entry_roundtrips_through_json() {
        let entry = RepoEntry {
            id: "r-1".to_string(),
            name: "frontend".to_string(),
            path: "/code/myorg/frontend".to_string(),
            posts_dir: "frontend".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: RepoEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.posts_dir, "frontend");
        assert_eq!(back.path, "/code/myorg/frontend");
    }

    #[test]
    fn test_workspace_repos_config_roundtrips() {
        let config = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "r1".to_string(),
                name: "frontend".to_string(),
                path: "/code/frontend".to_string(),
                posts_dir: "frontend".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let back: WorkspaceReposConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.version, 1);
        assert_eq!(back.repos.len(), 1);
        assert_eq!(back.repos[0].posts_dir, "frontend");
    }

    // ── 22.2.17 — two repos with identical basename get distinct posts_dir ───

    #[test]
    fn test_two_repos_with_identical_basename_get_distinct_posts_dir() {
        let path_a = Path::new("/org-a/repo");
        let path_b = Path::new("/org-b/repo");
        let existing: Vec<RepoEntry> = vec![];
        let dir_a = assign_posts_dir(path_a, &existing);

        let entry_a = RepoEntry {
            id: "r1".to_string(),
            name: "repo".to_string(),
            path: path_a.to_str().unwrap().to_string(),
            posts_dir: dir_a.clone(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let dir_b = assign_posts_dir(path_b, &[entry_a]);
        assert_ne!(dir_a, dir_b, "identical basenames must produce distinct posts_dir values");
        assert_eq!(dir_a, "repo");
        assert_eq!(dir_b, "repo-2");
    }

    // ── read/write workspace repos.json ───────────────────────────────────────

    #[test]
    fn test_read_workspace_repos_returns_empty_when_file_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("repos.json");
        let config = read_workspace_repos(&path).expect("should succeed");
        assert_eq!(config.version, 1);
        assert!(config.repos.is_empty());
    }

    #[test]
    fn test_write_and_read_workspace_repos_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("repos.json");
        let config = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "r1".to_string(),
                name: "frontend".to_string(),
                path: "/code/frontend".to_string(),
                posts_dir: "frontend".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&path, &config).expect("write");
        let loaded = read_workspace_repos(&path).expect("read");
        assert_eq!(loaded.repos.len(), 1);
        assert_eq!(loaded.repos[0].posts_dir, "frontend");
    }

    // ── 22.2.4 eager directory creation ───────────────────────────────────────

    /// 22.2.4 — create_workspace_dirs creates {workspace}/posts/ and {workspace}/drafts/.
    #[test]
    fn test_create_workspace_dirs_creates_posts_and_drafts() {
        let dir = tempfile::TempDir::new().unwrap();
        super::create_workspace_dirs(dir.path()).expect("create dirs");
        assert!(dir.path().join("posts").is_dir(), "posts/ must be created");
        assert!(dir.path().join("drafts").is_dir(), "drafts/ must be created");
    }

    /// 22.2.4 — create_workspace_dirs is idempotent.
    #[test]
    fn test_create_workspace_dirs_is_idempotent() {
        let dir = tempfile::TempDir::new().unwrap();
        super::create_workspace_dirs(dir.path()).expect("first call");
        super::create_workspace_dirs(dir.path()).expect("second call must not error");
    }
}
