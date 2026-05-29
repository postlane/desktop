// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use tempfile::TempDir;

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

fn write_workspace_repos_json(
    workspace_path: &std::path::Path,
    entries: Vec<crate::workspace_repos::RepoEntry>,
) {
    let config = crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: entries };
    crate::workspace_repos::write_workspace_repos(&workspace_path.join("repos.json"), &config)
        .expect("write workspace repos");
}

// ── 22.3.10 — selected repos written; deselected repos absent ────────────────

#[test]
fn test_confirm_workspace_repos_writes_only_selected_repos() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-1", ws.path().to_str().unwrap());
    write_workspace_repos_json(ws.path(), vec![
        make_repo_entry("frontend", "/code/org/frontend"),
        make_repo_entry("backend", "/code/org/backend"),
        make_repo_entry("docs", "/code/org/docs"),
    ]);

    let selected = vec!["/code/org/frontend".to_string(), "/code/org/docs".to_string()];
    let result = confirm_workspace_repos_impl(&repos_path, "proj-1", &selected);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    let written: crate::workspace_repos::WorkspaceReposConfig =
        serde_json::from_str(&fs::read_to_string(ws.path().join("repos.json")).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 2, "only 2 repos must remain");
    assert!(written.repos.iter().any(|r| r.path == "/code/org/frontend"), "frontend must be present");
    assert!(written.repos.iter().any(|r| r.path == "/code/org/docs"), "docs must be present");
    assert!(!written.repos.iter().any(|r| r.path == "/code/org/backend"), "backend must be absent");
}

#[test]
fn test_confirm_workspace_repos_writes_all_when_all_selected() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-2", ws.path().to_str().unwrap());
    write_workspace_repos_json(ws.path(), vec![
        make_repo_entry("frontend", "/code/org/frontend"),
        make_repo_entry("backend", "/code/org/backend"),
    ]);

    let selected = vec!["/code/org/frontend".to_string(), "/code/org/backend".to_string()];
    let result = confirm_workspace_repos_impl(&repos_path, "proj-2", &selected);
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    let written: crate::workspace_repos::WorkspaceReposConfig =
        serde_json::from_str(&fs::read_to_string(ws.path().join("repos.json")).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 2);
}

#[test]
fn test_confirm_workspace_repos_returns_error_when_workspace_not_found() {
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    // Write global repos with NO matching workspace
    let config = crate::storage::ReposConfig {
        version: 2,
        workspaces: vec![],
        repos: vec![],
    };
    crate::storage::write_repos(&repos_path, &config).expect("write");

    let result = confirm_workspace_repos_impl(&repos_path, "nonexistent-ws", &[]);
    assert!(result.is_err(), "expected Err for missing workspace");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("not found") || msg.contains("workspace"),
        "error must mention workspace, got: {}", msg
    );
}

#[test]
fn test_confirm_workspace_repos_preserves_repo_entry_fields() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");

    write_global_repos_with_workspace(&repos_path, "proj-3", ws.path().to_str().unwrap());
    let original = make_repo_entry("my-repo", "/code/org/my-repo");
    let original_id = original.id.clone();
    let original_posts_dir = original.posts_dir.clone();
    write_workspace_repos_json(ws.path(), vec![original]);

    let result = confirm_workspace_repos_impl(
        &repos_path,
        "proj-3",
        &["/code/org/my-repo".to_string()],
    );
    assert!(result.is_ok());

    let written: crate::workspace_repos::WorkspaceReposConfig =
        serde_json::from_str(&fs::read_to_string(ws.path().join("repos.json")).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 1);
    assert_eq!(written.repos[0].id, original_id, "id must be preserved");
    assert_eq!(written.repos[0].posts_dir, original_posts_dir, "posts_dir must be preserved");
}
