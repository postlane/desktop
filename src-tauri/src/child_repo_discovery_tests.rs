// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn make_git_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).expect("create .git");
}

// ── 24.3.2 ── zero child repos → user-facing error, no advance ──────────────

#[test]
fn test_setup_rejects_workspace_with_no_git_repos() {
    let ws = TempDir::new().unwrap();
    // A non-git subdirectory should not count.
    fs::create_dir_all(ws.path().join("not-a-repo")).unwrap();

    let result = discover_child_repos_impl(ws.path());
    assert!(result.is_err(), "expected Err for zero-repo folder");
    assert_eq!(
        result.unwrap_err(),
        "No Git repositories found in this folder. Select a folder that contains one or more Git repos."
    );
}

// ── 24.3.2 ── discovers first-level child repos with posts_dir assigned ─────

#[test]
fn test_setup_discovers_child_repos() {
    let ws = TempDir::new().unwrap();
    make_git_repo(&ws.path().join("frontend"));
    make_git_repo(&ws.path().join("backend"));

    let result = discover_child_repos_impl(ws.path());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let repos = result.unwrap();
    assert_eq!(repos.len(), 2);

    let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"frontend"));
    assert!(names.contains(&"backend"));
    assert!(repos.iter().all(|r| !r.posts_dir.is_empty()));
    assert!(repos.iter().all(|r| !r.path.is_empty()));
    // No basename collision here, so posts_dir must equal the basename exactly.
    assert!(repos.iter().any(|r| r.name == "frontend" && r.posts_dir == "frontend"));
    assert!(repos.iter().any(|r| r.name == "backend" && r.posts_dir == "backend"));
}

// ── 24.3.2 ── posts_dir deduplicated on basename collision ──────────────────
//
// A single directory listing can never contain two entries with the same
// name (the filesystem itself guarantees uniqueness), so a same-call
// collision within `discover_child_repos_impl` is unreachable on a real
// filesystem. What IS reachable, and what this test verifies, is that
// `assign_child_repo_posts_dirs` -- the pure sequential-dedup step this
// module's real discovery path delegates to -- correctly deduplicates two
// entries that share a basename but live at different parent paths (the
// same synthetic-path approach `workspace_repos.rs`'s own dedup test uses).

#[test]
fn test_posts_dir_deduplicated_on_collision() {
    let paths = vec![
        PathBuf::from("/org-a/shared-name"),
        PathBuf::from("/org-b/shared-name"),
    ];

    let repos = assign_child_repo_posts_dirs(&paths);
    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].posts_dir, "shared-name");
    assert_eq!(repos[1].posts_dir, "shared-name-2");
    assert_ne!(repos[0].posts_dir, repos[1].posts_dir, "collision must be deduplicated");
}
