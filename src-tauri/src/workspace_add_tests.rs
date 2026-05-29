// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use tempfile::TempDir;

fn make_git_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).expect("create .git");
}

// ── 22.3.8 ── creates config.json, config.local.json, .gitignore entry ───────

#[test]
fn test_add_workspace_creates_config_files() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    make_git_repo(&ws.path().join("child-repo"));

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-abc");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    assert!(ws.path().join("config.json").exists(), "config.json must exist");
    assert!(ws.path().join("config.local.json").exists(), "config.local.json must exist");

    let gitignore = ws.path().join(".gitignore");
    assert!(gitignore.exists(), ".gitignore must exist");
    let gi_content = fs::read_to_string(&gitignore).unwrap();
    assert!(
        gi_content.lines().any(|l| l.trim() == "config.local.json"),
        ".gitignore must contain config.local.json, got:\n{}", gi_content
    );
}

// 22.3.8 — does not overwrite existing config.json
#[test]
fn test_add_workspace_does_not_overwrite_existing_config_json() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    let config_path = ws.path().join("config.json");
    fs::write(&config_path, r#"{"project_id":"existing-proj"}"#).unwrap();

    make_git_repo(&ws.path().join("repo-a"));

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "new-proj");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    let content = fs::read_to_string(&config_path).unwrap();
    assert!(
        content.contains("existing-proj"),
        "config.json must not be overwritten, got: {}", content
    );
}

// ── 22.3.9 ── writes repos.json files and global registry ────────────────────

#[test]
fn test_add_workspace_writes_workspace_repos_json_and_global_registry() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    make_git_repo(&ws.path().join("frontend"));
    make_git_repo(&ws.path().join("backend"));

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-xyz");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    let ws_repos_path = ws.path().join("repos.json");
    assert!(ws_repos_path.exists(), "{{workspace}}/repos.json must exist");
    let ws_repos: crate::workspace_repos::WorkspaceReposConfig =
        serde_json::from_str(&fs::read_to_string(&ws_repos_path).unwrap()).unwrap();
    assert_eq!(ws_repos.repos.len(), 2, "must have 2 repos");
    assert!(
        ws_repos.repos.iter().all(|r| !r.posts_dir.is_empty()),
        "all repos must have a non-empty posts_dir"
    );

    let global: crate::storage::ReposConfig =
        serde_json::from_str(&fs::read_to_string(&repos_path).unwrap()).unwrap();
    assert_eq!(global.workspaces.len(), 1, "must have 1 workspace entry in global repos.json");
    assert_eq!(global.workspaces[0].id, "proj-xyz");
    assert!(!global.workspaces[0].workspace_path.is_empty());
}

// ── 22.3.11 ── single-repo workspace ─────────────────────────────────────────

#[test]
fn test_add_workspace_single_repo_returns_one_discovered_repo() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    make_git_repo(&ws.path().join("only-repo"));

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-single");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let ws_result = result.unwrap();
    assert_eq!(ws_result.discovered_repos.len(), 1);
    assert_eq!(ws_result.workspace_id, "proj-single");
}

// ── 22.3.12 ── zero-repo folder → PL-WS-001 ──────────────────────────────────

#[test]
fn test_add_workspace_zero_repos_returns_pl_ws_001() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    // No child git repos created

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-zero");
    assert!(result.is_err(), "expected Err for zero-repo folder");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-WS-001"), "expected PL-WS-001 in error, got: {}", msg);

    assert!(!ws.path().join("repos.json").exists(), "no repos.json created on error");
    assert!(!repos_path.exists(), "global repos.json must not be written on error");
}

// ── 22.3.13 ── path inside ~/.postlane/ rejected ──────────────────────────────

#[test]
fn test_add_workspace_rejects_path_inside_postlane_dir() {
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    let inside = fake_postlane.path().join("sub");
    fs::create_dir_all(&inside).unwrap();

    let result = add_workspace_impl(&inside, &repos_path, fake_postlane.path(), "proj-inside");
    assert!(result.is_err(), "path inside postlane dir must be rejected");

    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("reserved") || msg.to_lowercase().contains("postlane"),
        "error must explain rejection, got: {}", msg
    );
    assert!(!repos_path.exists(), "global repos.json must not be written");
}

// ── 22.3.14 ── symlink resolving to ~/.postlane/ rejected ────────────────────

#[cfg(unix)]
#[test]
fn test_add_workspace_rejects_symlink_resolving_to_postlane_dir() {
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    // A child repo inside fake_postlane so that a missing symlink check would let
    // discover_child_repos succeed and the function would write repos_path.
    // This makes the test non-vacuous: a broken impl would write repos_path and fail.
    make_git_repo(&fake_postlane.path().join("child-repo"));

    let link_parent = TempDir::new().unwrap();
    let link = link_parent.path().join("symlink-to-postlane");
    std::os::unix::fs::symlink(fake_postlane.path(), &link).unwrap();

    let result = add_workspace_impl(&link, &repos_path, fake_postlane.path(), "proj-symlink");
    assert!(result.is_err(), "symlink resolving to postlane dir must be rejected");
    assert!(!repos_path.exists(), "global repos.json must not be written");
    assert!(
        !fake_postlane.path().join("config.json").exists(),
        "no config.json must be created inside postlane_dir"
    );
}

// ── 22.3.5b / 22.3.5d ── folder is itself a git repo → PL-WS-003 ────────────

#[test]
fn test_add_workspace_rejects_git_repo_folder_with_pl_ws_003() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    make_git_repo(ws.path());

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-git");
    assert!(result.is_err(), "git repo as workspace must be rejected");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-WS-003"), "expected PL-WS-003 in error, got: {}", msg);

    assert!(!ws.path().join("config.json").exists(), "config.json must not be created on error");
    assert!(!repos_path.exists(), "global repos.json must not be written on error");
}

// ── posts/ and drafts/ directories created eagerly ───────────────────────────

#[test]
fn test_add_workspace_creates_posts_and_drafts_dirs() {
    let ws = TempDir::new().unwrap();
    let fake_postlane = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    make_git_repo(&ws.path().join("repo-a"));

    let result = add_workspace_impl(ws.path(), &repos_path, fake_postlane.path(), "proj-dirs");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    assert!(ws.path().join("posts").is_dir(), "{{workspace}}/posts/ must be created");
    assert!(ws.path().join("drafts").is_dir(), "{{workspace}}/drafts/ must be created");
}
