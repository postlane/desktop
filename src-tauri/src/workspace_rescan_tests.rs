// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use tempfile::TempDir;

fn make_git_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).expect("create .git");
}

fn write_global_repos_with_workspace(
    repos_path: &std::path::Path,
    workspace_id: &str,
    workspace_path: &str,
) {
    let config = crate::storage::ReposConfig {
        version: 2,
        workspaces: vec![crate::workspace_entry::WorkspaceEntry {
            id: workspace_id.to_string(),
            name: "test-ws".to_string(),
            workspace_path: workspace_path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }],
        repos: vec![],
    };
    crate::storage::write_repos(repos_path, &config).expect("write global repos");
}

fn make_repo_entry(name: &str, path: &str) -> crate::workspace_repos::RepoEntry {
    crate::workspace_repos::RepoEntry {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        path: path.to_string(),
        posts_dir: name.to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn write_workspace_repos(
    workspace_path: &std::path::Path,
    entries: Vec<crate::workspace_repos::RepoEntry>,
) {
    let config = crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: entries };
    crate::workspace_repos::write_workspace_repos(&workspace_path.join("repos.json"), &config)
        .expect("write workspace repos");
}

// ── 22.3.18 — rescan finds newly cloned repo ─────────────────────────────────

/// 22.3.18 — A repo cloned after the workspace was first set up is discovered and
/// added to {workspace}/repos.json with active: true and a non-empty posts_dir.
#[test]
fn test_rescan_workspace_finds_newly_cloned_repo() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-rescan-1", ws.path().to_str().unwrap());

    // child-a: already registered and present on disk
    let child_a = ws.path().join("child-a");
    make_git_repo(&child_a);
    write_workspace_repos(
        ws.path(),
        vec![make_repo_entry("child-a", child_a.to_str().unwrap())],
    );

    // child-b: on disk but NOT yet in repos.json
    let child_b = ws.path().join("child-b");
    make_git_repo(&child_b);

    let result = rescan_workspace_impl(&repos_path, "proj-rescan-1")
        .expect("rescan must succeed");

    assert!(
        result.added.contains(&"child-b".to_string()),
        "child-b must appear in added, got: {:?}",
        result.added
    );
    assert!(
        result.unchanged.contains(&"child-a".to_string()),
        "child-a must appear in unchanged, got: {:?}",
        result.unchanged
    );
    assert!(result.deactivated.is_empty(), "no repos should be deactivated");

    // Verify repos.json on disk
    let written: crate::workspace_repos::WorkspaceReposConfig = serde_json::from_str(
        &fs::read_to_string(ws.path().join("repos.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(written.repos.len(), 2, "repos.json must have 2 entries");

    for entry in &written.repos {
        assert!(entry.active, "both entries must be active: {}", entry.name);
        assert!(!entry.posts_dir.is_empty(), "posts_dir must be non-empty: {}", entry.name);
    }
}

// ── 22.3.19 — rescan deactivates missing repo ─────────────────────────────────

/// 22.3.19 — A repo registered in repos.json but no longer present on disk is
/// marked active: false without being removed from the list.
#[test]
fn test_rescan_workspace_deactivates_missing_repo() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-rescan-2", ws.path().to_str().unwrap());

    let child_a = ws.path().join("child-a");
    let child_b = ws.path().join("child-b");

    // child-a exists on disk; child-b does not
    make_git_repo(&child_a);

    write_workspace_repos(
        ws.path(),
        vec![
            make_repo_entry("child-a", child_a.to_str().unwrap()),
            make_repo_entry("child-b", child_b.to_str().unwrap()),
        ],
    );

    let result = rescan_workspace_impl(&repos_path, "proj-rescan-2")
        .expect("rescan must succeed");

    assert!(
        result.deactivated.contains(&"child-b".to_string()),
        "child-b must appear in deactivated, got: {:?}",
        result.deactivated
    );
    assert!(
        result.unchanged.contains(&"child-a".to_string()),
        "child-a must appear in unchanged, got: {:?}",
        result.unchanged
    );
    assert!(result.added.is_empty(), "no repos should be added");

    // Verify repos.json on disk
    let written: crate::workspace_repos::WorkspaceReposConfig = serde_json::from_str(
        &fs::read_to_string(ws.path().join("repos.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(written.repos.len(), 2, "repos.json must still have 2 entries");

    let entry_a = written.repos.iter().find(|r| r.name == "child-a").unwrap();
    let entry_b = written.repos.iter().find(|r| r.name == "child-b").unwrap();
    assert!(entry_a.active, "child-a must remain active");
    assert!(!entry_b.active, "child-b must be deactivated");
}

// ── 22.10.12 — rescan assigns collision-safe posts_dir ───────────────────────

/// When a workspace already has repos with `posts_dir: "frontend"` and
/// `posts_dir: "frontend-2"`, a newly cloned repo whose basename is also
/// "frontend" must receive `posts_dir: "frontend-3"`.
#[test]
fn test_rescan_assigns_collision_safe_posts_dir_for_duplicate_basename() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-rescan-col", ws.path().to_str().unwrap());

    // Two existing repos at different absolute paths, both with basename "frontend"
    let dir_a = ws.path().join("org-a").join("frontend");
    let dir_b = ws.path().join("org-b").join("frontend");
    make_git_repo(&dir_a);
    make_git_repo(&dir_b);

    // Only dir_a is directly inside the workspace (1 level deep); place a
    // third "frontend" at workspace root level for discover_child_repos to find.
    let dir_c = ws.path().join("frontend");
    make_git_repo(&dir_c);

    // Pre-populate workspace repos.json with two entries whose posts_dir values
    // already occupy "frontend" and "frontend-2".
    let mut entry_a = make_repo_entry("frontend", dir_a.to_str().unwrap());
    entry_a.posts_dir = "frontend".to_string();
    let mut entry_b = make_repo_entry("frontend", dir_b.to_str().unwrap());
    entry_b.posts_dir = "frontend-2".to_string();
    write_workspace_repos(ws.path(), vec![entry_a, entry_b]);

    // dir_c is at workspace root — discover_child_repos will find it.
    let result = rescan_workspace_impl(&repos_path, "proj-rescan-col")
        .expect("rescan must succeed");

    assert!(
        result.added.contains(&"frontend".to_string()),
        "frontend (dir_c) must be added; got: {:?}",
        result.added
    );

    let written: crate::workspace_repos::WorkspaceReposConfig = serde_json::from_str(
        &std::fs::read_to_string(ws.path().join("repos.json")).unwrap(),
    )
    .unwrap();

    let new_entry = written.repos.iter()
        .find(|e| e.path == dir_c.to_str().unwrap())
        .expect("new entry for dir_c must exist in repos.json");

    assert_eq!(
        new_entry.posts_dir, "frontend-3",
        "new repo must receive posts_dir 'frontend-3' to avoid collision; got '{}'",
        new_entry.posts_dir
    );
}

// ── 22.9.11: workspace_rescan telemetry ──────────────────────────────────────

#[test]
fn test_workspace_rescan_records_telemetry_with_counts() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_rescan(&state, true, 2, 1);
    assert_eq!(state.telemetry.queue_len(), 1, "workspace_rescan must be queued");
    let events = state.telemetry.peek_queue();
    assert_eq!(events[0].name, "workspace_rescan");
    assert_eq!(events[0].properties["added"], 2);
    assert_eq!(events[0].properties["removed"], 1);
}

#[test]
fn test_workspace_rescan_no_event_without_consent() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_rescan(&state, false, 1, 0);
    assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
}
