// SPDX-License-Identifier: BUSL-1.1

use std::path::{Path, PathBuf};

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

/// Returns the effective config path for a child repo: the child's own
/// `.postlane/config.json` if it exists, otherwise the workspace parent's.
pub fn effective_config_path(child_path: &Path, workspace_path: &Path) -> PathBuf {
    let child_config = child_path.join(".postlane").join("config.json");
    if child_config.exists() {
        child_config
    } else {
        workspace_path.join(".postlane").join("config.json")
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
}
