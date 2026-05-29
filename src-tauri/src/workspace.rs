// SPDX-License-Identifier: BUSL-1.1

use std::path::{Path, PathBuf};

/// In-memory representation of a workspace loaded from disk.
///
/// Loaded from `{workspace_path}/repos.json` (the workspace child-repo registry,
/// distinct from `~/.postlane/repos.json` which is the global desktop registry).
/// All config reads for workspace repos use fields on this struct rather than
/// constructing `.postlane/` paths inline.
pub struct WorkspaceRoot {
    /// Absolute path to the workspace root directory.
    pub workspace_path: PathBuf,
    /// The workspace's `id` from `~/.postlane/repos.json` — equals the project_id.
    pub workspace_id: String,
    /// Child repos loaded from `{workspace_path}/repos.json`, each with a `posts_dir` field.
    pub repos: Vec<crate::workspace_repos::RepoEntry>,
}

/// Returns true when `path` is a workspace root: a directory that has no `.git/`
/// directly inside it but contains one or more child repos.
pub fn is_workspace_root(path: &Path) -> bool {
    !path.join(".git").exists()
}

/// Enumerates immediate child directories of `workspace_path` that are git repos.
///
/// Rules:
/// - One level deep only.
/// - Symlinks are never followed; symlinked entries are skipped.
/// - Non-directory entries are skipped.
/// - Directories without a `.git/` subdirectory are skipped.
pub fn discover_child_repos(workspace_path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(workspace_path) else {
        return vec![];
    };

    let mut children: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            // Use symlink_metadata so symlinks are NOT followed — the metadata
            // describes the symlink itself, not its target.
            let meta = std::fs::symlink_metadata(&path).ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            if !meta.is_dir() {
                return None;
            }
            if path.join(".git").is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    children.sort();
    children
}

/// Returns the effective config path for a child repo.
///
/// Priority order (22.1.2):
///   1. `{child_path}/.postlane/config.json` — per-repo override (child wins on conflict)
///   2. `{workspace_path}/config.json`        — workspace primary (at workspace root, not .postlane/)
///
/// Legacy per-repo installs (entries in the `repos` array of `~/.postlane/repos.json`,
/// which have no associated workspace_path) call this with the same path for both
/// arguments, so the per-repo `.postlane/config.json` is always returned (22.1.15).
pub fn effective_config_path(child_path: &Path, workspace_path: &Path) -> PathBuf {
    let child_config = child_path.join(".postlane").join("config.json");
    if child_config.exists() {
        child_config
    } else {
        workspace_path.join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("postlane_ws_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        dir
    }

    fn make_git_repo(dir: &Path) {
        fs::create_dir_all(dir.join(".git")).expect("create .git");
    }

    #[test]
    fn test_is_workspace_root_returns_false_for_git_repo() {
        let dir = setup_dir("is_ws_git");
        make_git_repo(&dir);
        assert!(!is_workspace_root(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_workspace_root_returns_true_when_no_git_dir() {
        let dir = setup_dir("is_ws_no_git");
        assert!(is_workspace_root(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_child_repos_finds_git_dirs_one_level_deep() {
        let ws = setup_dir("discover_basic");
        let child_a = ws.join("repo-a");
        let child_b = ws.join("repo-b");
        make_git_repo(&child_a);
        make_git_repo(&child_b);
        // Non-git dir — should be excluded
        fs::create_dir_all(ws.join("not-a-repo")).expect("create non-git");

        let found = discover_child_repos(&ws);
        assert_eq!(found.len(), 2, "only git repos should be found");
        assert!(found.iter().any(|p| p.ends_with("repo-a")));
        assert!(found.iter().any(|p| p.ends_with("repo-b")));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_discover_child_repos_does_not_recurse_into_grandchildren() {
        let ws = setup_dir("discover_depth");
        let child = ws.join("child");
        let grandchild = child.join("grandchild");
        make_git_repo(&child);
        make_git_repo(&grandchild);

        let found = discover_child_repos(&ws);
        // Only `child` should appear — `grandchild` is two levels deep
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("child"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_discover_child_repos_skips_symlinks() {
        let ws = setup_dir("discover_symlink");
        let real_repo = setup_dir("discover_symlink_real_repo");
        make_git_repo(&real_repo);
        // Symlink inside workspace pointing to the real repo
        let link = ws.join("linked-repo");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_repo, &link).expect("create symlink");

        let found = discover_child_repos(&ws);
        assert!(
            found.is_empty(),
            "symlinked child repos must not be followed, found: {:?}",
            found
        );
        let _ = fs::remove_dir_all(&ws);
        let _ = fs::remove_dir_all(&real_repo);
    }

    #[test]
    fn test_discover_child_repos_returns_sorted_paths() {
        let ws = setup_dir("discover_sorted");
        for name in ["repo-c", "repo-a", "repo-b"] {
            make_git_repo(&ws.join(name));
        }
        let found = discover_child_repos(&ws);
        let names: Vec<&str> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["repo-a", "repo-b", "repo-c"]);
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_effective_config_path_uses_child_when_present() {
        let ws = setup_dir("eff_cfg_child");
        let child = ws.join("repo-a");
        fs::create_dir_all(child.join(".postlane")).expect("create .postlane");
        fs::write(child.join(".postlane/config.json"), "{}").expect("write child config");

        let path = effective_config_path(&child, &ws);
        assert!(path.ends_with("repo-a/.postlane/config.json"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_discover_child_repos_returns_empty_for_nonexistent_path() {
        let path = std::path::Path::new("/nonexistent/workspace/path/that/does/not/exist");
        let found = discover_child_repos(path);
        assert!(found.is_empty(), "nonexistent path must return empty vec");
    }

    #[test]
    fn test_discover_child_repos_skips_files_not_dirs() {
        let ws = setup_dir("discover_files");
        // Create a regular file in the workspace — must be skipped
        fs::write(ws.join("not-a-dir.txt"), "content").expect("write file");
        // Also create a real git repo so the result is non-trivially exercised
        make_git_repo(&ws.join("real-repo"));
        let found = discover_child_repos(&ws);
        assert_eq!(found.len(), 1, "only directories should be returned, not files");
        assert!(found[0].ends_with("real-repo"));
        let _ = fs::remove_dir_all(&ws);
    }

    #[test]
    fn test_effective_config_path_falls_back_to_workspace_config() {
        let ws = setup_dir("eff_cfg_ws");
        let child = ws.join("repo-a");
        fs::create_dir_all(&child).expect("create child");
        // No child config.json

        let path = effective_config_path(&child, &ws);
        assert!(
            path.to_str().unwrap().contains(&ws.to_str().unwrap()),
            "should use workspace config"
        );
        assert!(path.ends_with("config.json"));
        let _ = fs::remove_dir_all(&ws);
    }

    // ── 22.1.2 workspace-root config path ─────────────────────────────────────

    /// 22.1.2 — workspace config is at {workspace_root}/config.json (not .postlane/).
    #[test]
    fn test_effective_config_path_fallback_is_workspace_root_config_json() {
        let ws = setup_dir("eff_cfg_v2_root");
        let child = ws.join("repo-a");
        fs::create_dir_all(&child).expect("create child");

        let path = effective_config_path(&child, &ws);
        let expected = ws.join("config.json");
        assert_eq!(path, expected, "workspace fallback must be {{workspace}}/config.json, not .postlane/config.json");
        let _ = fs::remove_dir_all(&ws);
    }

    // ── 22.1.7 WorkspaceRoot struct ───────────────────────────────────────────

    /// 22.1.7 — WorkspaceRoot struct can be constructed and exposes fields correctly.
    #[test]
    fn test_workspace_root_struct_fields() {
        let root = WorkspaceRoot {
            workspace_path: std::path::PathBuf::from("/code/myorg"),
            workspace_id: "ws-abc".to_string(),
            repos: vec![],
        };
        assert_eq!(root.workspace_id, "ws-abc");
        assert_eq!(root.workspace_path, std::path::PathBuf::from("/code/myorg"));
        assert!(root.repos.is_empty());
    }

    // ── 22.1.10 schema_version tolerance ──────────────────────────────────────

    /// 22.1.10 — config.json without schema_version field is read without error.
    /// 22.1.18 — effective config resolves correctly for config.json lacking schema_version.
    #[test]
    fn test_config_json_without_schema_version_reads_correctly() {
        let ws = setup_dir("no_schema_ver");
        // Write a config.json with no schema_version field (simulates legacy file)
        let config_path = ws.join("config.json");
        fs::write(&config_path, r#"{"project_id":"proj-legacy-no-schema"}"#).expect("write");

        let content = fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        // schema_version must be absent without causing an error
        assert!(parsed["schema_version"].is_null(), "schema_version must be absent");
        // project_id must still be readable
        assert_eq!(parsed["project_id"].as_str(), Some("proj-legacy-no-schema"));

        // effective_config_path must resolve to this file when no child config exists
        let child = ws.join("repo-a");
        fs::create_dir_all(&child).expect("create child");
        let eff_path = effective_config_path(&child, &ws);
        assert_eq!(eff_path, config_path);

        let _ = fs::remove_dir_all(&ws);
    }

    // ── 22.1.13–22.1.15 config resolution ────────────────────────────────────

    /// 22.1.13 — workspace config is primary; per-repo override wins on conflict;
    /// non-conflicting fields from workspace still present in resolved path.
    #[test]
    fn test_workspace_config_is_primary_child_override_wins() {
        let ws = setup_dir("cfg_resolve_ws");
        let child = ws.join("repo-a");
        fs::create_dir_all(child.join(".postlane")).expect("create .postlane");

        // Workspace config (primary)
        fs::write(ws.join("config.json"), r#"{"project_id":"ws-proj","schema_version":4}"#)
            .expect("write workspace config");
        // Child override config
        fs::write(child.join(".postlane").join("config.json"), r#"{"project_id":"child-proj"}"#)
            .expect("write child config");

        // Child config wins on conflict (project_id)
        let eff_path = effective_config_path(&child, &ws);
        assert_eq!(eff_path, child.join(".postlane").join("config.json"), "child config must win");

        let _ = fs::remove_dir_all(&ws);
    }

    /// 22.1.14 — project_id read from workspace config when no per-repo override present.
    #[test]
    fn test_project_id_read_from_workspace_config_when_no_child_override() {
        let ws = setup_dir("cfg_ws_proj_id");
        let child = ws.join("repo-a");
        fs::create_dir_all(&child).expect("create child");
        fs::write(ws.join("config.json"), r#"{"project_id":"ws-proj-id-123"}"#)
            .expect("write workspace config");

        let eff_path = effective_config_path(&child, &ws);
        assert_eq!(eff_path, ws.join("config.json"), "workspace config must be used when no child config");

        let content = fs::read_to_string(&eff_path).expect("read config");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("ws-proj-id-123"));

        let _ = fs::remove_dir_all(&ws);
    }

    /// 22.1.15 — legacy per-repo config path resolves correctly for repos array entries.
    /// When workspace_path == child_path (legacy install), per-repo .postlane/config.json is used.
    #[test]
    fn test_legacy_per_repo_config_path_resolves_for_repos_array_entries() {
        let repo = setup_dir("cfg_legacy_repo");
        let postlane = repo.join(".postlane");
        fs::create_dir_all(&postlane).expect("create .postlane");
        fs::write(postlane.join("config.json"), r#"{"project_id":"legacy-proj"}"#)
            .expect("write per-repo config");

        // Legacy install: no separate workspace path — pass repo as both args
        let eff_path = effective_config_path(&repo, &repo);
        assert_eq!(eff_path, postlane.join("config.json"), "legacy per-repo config must be resolved");

        let _ = fs::remove_dir_all(&repo);
    }
}
