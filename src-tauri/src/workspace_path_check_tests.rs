// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn write_config_json(dir: &std::path::Path, project_id: &str) {
    let json = serde_json::json!({ "project_id": project_id, "schema_version": 4 });
    fs::write(dir.join("config.json"), serde_json::to_string(&json).unwrap()).unwrap();
}

fn make_workspace_entry(id: &str, name: &str, path: &std::path::Path) -> WorkspaceEntry {
    WorkspaceEntry {
        id: id.to_string(),
        name: name.to_string(),
        workspace_path: path.to_string_lossy().to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn write_global_repos(repos_path: &std::path::Path, workspaces: &[WorkspaceEntry]) {
    let config = crate::storage::ReposConfig {
        version: 2,
        workspaces: workspaces.to_vec(),
        repos: vec![],
    };
    let json = serde_json::to_string_pretty(&config).unwrap();
    fs::write(repos_path, json).unwrap();
}

// ── 22.3.26: workspace path exists → Ok status ────────────────────────────────

#[test]
fn test_workspace_path_exists_returns_ok() {
    let dir = TempDir::new().unwrap();
    let workspace = dir.path().join("myorg");
    fs::create_dir_all(&workspace).unwrap();
    write_config_json(&workspace, "proj-1");

    let entry = make_workspace_entry("proj-1", "myorg", &workspace);
    let result = check_workspace_path(&entry);

    assert_eq!(result.workspace_id, "proj-1");
    assert!(
        matches!(result.status, WorkspacePathStatus::Ok),
        "expected Ok, got {:?}",
        result.status
    );
}

// ── 22.3.27: renamed folder detected as single candidate ─────────────────────

#[test]
fn test_renamed_workspace_folder_found_as_candidate() {
    let dir = TempDir::new().unwrap();
    let old_path = dir.path().join("myorg");
    let new_path = dir.path().join("myorg-renamed");

    // old path does NOT exist; new_path does and has matching project_id
    fs::create_dir_all(&new_path).unwrap();
    write_config_json(&new_path, "proj-1");

    let entry = make_workspace_entry("proj-1", "myorg", &old_path);
    let result = check_workspace_path(&entry);

    match result.status {
        WorkspacePathStatus::Renamed { candidates } => {
            assert_eq!(candidates.len(), 1, "should find exactly one rename candidate");
            assert_eq!(candidates[0].path, new_path.to_string_lossy().as_ref());
            assert_eq!(candidates[0].name, "myorg-renamed");
        }
        other => panic!("expected Renamed, got {:?}", other),
    }
}

// 22.3.27: repos.json must NOT be updated until update_workspace_path_impl is called
#[test]
fn test_update_workspace_path_impl_updates_repos_json() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let old_path = dir.path().join("myorg");
    let new_path = dir.path().join("myorg-renamed");

    write_global_repos(&repos_path, &[make_workspace_entry("proj-1", "myorg", &old_path)]);

    // Before update: still has old path
    let before = crate::storage::read_repos_with_recovery(&repos_path).unwrap();
    assert_eq!(before.workspaces[0].workspace_path, old_path.to_string_lossy().as_ref());

    update_workspace_path_impl(&repos_path, "proj-1", &new_path.to_string_lossy()).unwrap();

    // After update: new path and updated basename name
    let after = crate::storage::read_repos_with_recovery(&repos_path).unwrap();
    assert_eq!(after.workspaces[0].workspace_path, new_path.to_string_lossy().as_ref());
    assert_eq!(after.workspaces[0].name, "myorg-renamed");
}

// 22.3.27: check does not mutate repos.json (the scan is read-only)
#[test]
fn test_check_workspace_path_does_not_mutate_repos_json() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let old_path = dir.path().join("myorg");
    let new_path = dir.path().join("myorg-renamed");

    fs::create_dir_all(&new_path).unwrap();
    write_config_json(&new_path, "proj-1");
    write_global_repos(&repos_path, &[make_workspace_entry("proj-1", "myorg", &old_path)]);

    let entry = make_workspace_entry("proj-1", "myorg", &old_path);
    let _ = check_workspace_path(&entry);

    // repos.json must still have the old path
    let after = crate::storage::read_repos_with_recovery(&repos_path).unwrap();
    assert_eq!(after.workspaces[0].workspace_path, old_path.to_string_lossy().as_ref());
}

// ── 22.3.28: two siblings match → both candidates returned ───────────────────

#[test]
fn test_multiple_rename_candidates_returned() {
    let dir = TempDir::new().unwrap();
    let old_path = dir.path().join("myorg");
    let candidate_a = dir.path().join("myorg-a");
    let candidate_b = dir.path().join("myorg-b");

    fs::create_dir_all(&candidate_a).unwrap();
    fs::create_dir_all(&candidate_b).unwrap();
    write_config_json(&candidate_a, "proj-1");
    write_config_json(&candidate_b, "proj-1");

    let entry = make_workspace_entry("proj-1", "myorg", &old_path);
    let result = check_workspace_path(&entry);

    match result.status {
        WorkspacePathStatus::Renamed { candidates } => {
            assert_eq!(candidates.len(), 2, "should find both rename candidates");
        }
        other => panic!("expected Renamed with 2 candidates, got {:?}", other),
    }
}

// ── 22.3.29: folder moved to non-adjacent location → Missing ─────────────────

#[test]
fn test_missing_workspace_returns_missing_status() {
    let dir = TempDir::new().unwrap();
    // Path whose parent also does not exist → no siblings to scan
    let gone = dir.path().join("deep").join("nested").join("gone");

    let entry = make_workspace_entry("proj-1", "gone", &gone);
    let result = check_workspace_path(&entry);

    assert!(
        matches!(result.status, WorkspacePathStatus::Missing),
        "expected Missing, got {:?}",
        result.status
    );
}

// 22.3.29: sibling without matching project_id is not selected as candidate
#[test]
fn test_sibling_with_wrong_project_id_is_not_a_candidate() {
    let dir = TempDir::new().unwrap();
    let old_path = dir.path().join("myorg");
    let sibling = dir.path().join("other-project");

    fs::create_dir_all(&sibling).unwrap();
    // Different project_id
    write_config_json(&sibling, "different-project");

    let entry = make_workspace_entry("proj-1", "myorg", &old_path);
    let result = check_workspace_path(&entry);

    assert!(
        matches!(result.status, WorkspacePathStatus::Missing),
        "sibling with wrong project_id must not be a candidate; got {:?}",
        result.status
    );
}

// 22.3.29: validate_workspace_folder_impl + update succeeds with matching project_id
#[test]
fn test_validate_and_update_workspace_folder_succeeds() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let old_path = dir.path().join("old");
    let new_folder = dir.path().join("workspace");

    fs::create_dir_all(&new_folder).unwrap();
    write_config_json(&new_folder, "proj-1");
    write_global_repos(&repos_path, &[make_workspace_entry("proj-1", "myorg", &old_path)]);

    // Validation should succeed
    let validate_result =
        validate_workspace_folder_impl(&repos_path, "proj-1", &new_folder.to_string_lossy());
    assert!(validate_result.is_ok(), "should succeed when project_id matches; got {:?}", validate_result);

    // Update should write the new path
    update_workspace_path_impl(&repos_path, "proj-1", &new_folder.to_string_lossy()).unwrap();
    let after = crate::storage::read_repos_with_recovery(&repos_path).unwrap();
    assert_eq!(after.workspaces[0].workspace_path, new_folder.to_string_lossy().as_ref());
}

// ── 22.3.30: mismatched project_id rejected with PL-WS-002 ───────────────────

#[test]
fn test_validate_workspace_folder_rejects_mismatched_project_id() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let old_path = dir.path().join("old");
    let folder = dir.path().join("wrong-project");

    fs::create_dir_all(&folder).unwrap();
    write_config_json(&folder, "completely-different-project");
    write_global_repos(&repos_path, &[make_workspace_entry("proj-1", "myorg", &old_path)]);

    let result = validate_workspace_folder_impl(&repos_path, "proj-1", &folder.to_string_lossy());

    assert!(result.is_err(), "must reject mismatched project_id");
    let err = result.unwrap_err();
    assert!(
        err.contains("PL-WS-002"),
        "error must contain PL-WS-002; got: {}",
        err
    );
    // repos.json must be unchanged
    let after = crate::storage::read_repos_with_recovery(&repos_path).unwrap();
    assert_eq!(after.workspaces[0].workspace_path, old_path.to_string_lossy().as_ref());
}

// 22.3.30: reject when config.json is absent
#[test]
fn test_validate_workspace_folder_rejects_missing_config_json() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let old_path = dir.path().join("old");
    let folder = dir.path().join("no-config");

    fs::create_dir_all(&folder).unwrap();
    // No config.json written
    write_global_repos(&repos_path, &[make_workspace_entry("proj-1", "myorg", &old_path)]);

    let result = validate_workspace_folder_impl(&repos_path, "proj-1", &folder.to_string_lossy());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("PL-WS-002"));
}

// ── 22.9.11: workspace_path_recovered telemetry ───────────────────────────────

#[test]
fn test_path_recovered_auto_records_event() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_path_recovered(&state, true, "auto");
    assert_eq!(state.telemetry.queue_len(), 1);
    let ev = &state.telemetry.peek_queue()[0];
    assert_eq!(ev.name, "workspace_path_recovered");
    assert_eq!(ev.properties["method"], "auto");
}

#[test]
fn test_path_recovered_manual_records_event() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_path_recovered(&state, true, "manual");
    assert_eq!(state.telemetry.queue_len(), 1);
    assert_eq!(state.telemetry.peek_queue()[0].properties["method"], "manual");
}

#[test]
fn test_path_recovered_no_event_without_consent() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_path_recovered(&state, false, "auto");
    assert_eq!(state.telemetry.queue_len(), 0);
}
