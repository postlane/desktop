// SPDX-License-Identifier: BUSL-1.1
// Tests for workspace_license_sync.rs (checklist 24.4.8).

use super::*;
use crate::storage::ReposConfig;
use crate::workspace_entry::WorkspaceEntry;

fn make_entry(id: &str, license_status: Option<&str>) -> WorkspaceEntry {
    WorkspaceEntry {
        id: id.to_string(),
        name: id.to_string(),
        workspace_path: format!("/code/{}", id),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
        license_status: license_status.map(|s| s.to_string()),
        is_owner: None,
        status_updated_at: None,
    }
}

fn make_workspace_info(project_id: &str, status: &str, is_owner: bool) -> WorkspaceLicenseInfo {
    WorkspaceLicenseInfo {
        project_id: project_id.to_string(),
        name: project_id.to_string(),
        status: status.to_string(),
        is_owner,
        status_updated_at: "2026-07-01T00:00:00Z".to_string(),
    }
}

fn write_config(repos_path: &std::path::Path, workspaces: Vec<WorkspaceEntry>) {
    let config = ReposConfig { version: 2, workspaces, repos: vec![] };
    let json = serde_json::to_string_pretty(&config).expect("serialize");
    std::fs::write(repos_path, json).expect("write repos.json");
}

fn read_config(repos_path: &std::path::Path) -> ReposConfig {
    read_repos_with_recovery(repos_path).expect("read repos.json")
}

#[test]
fn test_apply_updates_matching_workspace_status() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-1", None)]);

    let workspaces = vec![make_workspace_info("proj-1", "paid_owned", true)];
    apply_license_statuses(&repos_path, &workspaces).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].license_status.as_deref(), Some("paid_owned"));
}

#[test]
fn test_apply_updates_matching_workspace_is_owner() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-1", None)]);

    let workspaces = vec![make_workspace_info("proj-1", "paid_owned", true)];
    apply_license_statuses(&repos_path, &workspaces).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].is_owner, Some(true));
}

#[test]
fn test_apply_updates_is_owner_false_for_collaborator_workspace() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-1", None)]);

    let workspaces = vec![make_workspace_info("proj-1", "collaborator", false)];
    apply_license_statuses(&repos_path, &workspaces).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].is_owner, Some(false));
}

#[test]
fn test_apply_updates_matching_workspace_status_updated_at() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-1", None)]);

    let mut info = make_workspace_info("proj-1", "payment_failed", true);
    info.status_updated_at = "2026-06-20T12:00:00Z".to_string();
    apply_license_statuses(&repos_path, &[info]).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].status_updated_at.as_deref(), Some("2026-06-20T12:00:00Z"));
}

#[test]
fn test_apply_leaves_non_matching_workspace_untouched() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-untouched", Some("payment_failed"))]);

    let workspaces = vec![make_workspace_info("proj-other", "paid_owned", true)];
    apply_license_statuses(&repos_path, &workspaces).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].license_status.as_deref(), Some("payment_failed"));
    assert_eq!(config.workspaces[0].is_owner, None);
}

#[test]
fn test_apply_updates_multiple_matching_workspaces() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(
        &repos_path,
        vec![make_entry("proj-1", None), make_entry("proj-2", Some("paid_owned"))],
    );

    let workspaces = vec![
        make_workspace_info("proj-1", "paid_owned", true),
        make_workspace_info("proj-2", "payment_failed", true),
    ];
    apply_license_statuses(&repos_path, &workspaces).expect("apply");

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].license_status.as_deref(), Some("paid_owned"));
    assert_eq!(config.workspaces[1].license_status.as_deref(), Some("payment_failed"));
}

#[test]
fn test_apply_handles_missing_repos_json() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");

    let workspaces = vec![make_workspace_info("proj-1", "paid_owned", true)];
    let result = apply_license_statuses(&repos_path, &workspaces);
    assert!(result.is_ok());
}

#[test]
fn test_sync_license_statuses_writes_through_path_override() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let repos_path = dir.path().join("repos.json");
    write_config(&repos_path, vec![make_entry("proj-1", None)]);
    *TEST_REPOS_PATH_OVERRIDE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = Some(repos_path.clone());

    let workspaces = vec![make_workspace_info("proj-1", "unlicensed", false)];
    sync_license_statuses(&workspaces).expect("sync");

    *TEST_REPOS_PATH_OVERRIDE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = None;

    let config = read_config(&repos_path);
    assert_eq!(config.workspaces[0].license_status.as_deref(), Some("unlicensed"));
    assert_eq!(config.workspaces[0].is_owner, Some(false));
}
