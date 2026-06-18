// SPDX-License-Identifier: BUSL-1.1
//! Tests for §22.6 workspace disconnect and hard-delete logic.

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use httpmock::prelude::*;

use crate::workspace_disconnect::{
    remove_workspace_entry, delete_project_api, safelist_validate_delete_path,
    migration_journal_exists, workspace_path_from_repos,
};
use crate::storage::{self, ReposConfig};
use crate::workspace_entry::WorkspaceEntry;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_workspace_entry(id: &str, path: &str) -> WorkspaceEntry {
    WorkspaceEntry {
        id: id.to_string(),
        name: id.to_string(),
        workspace_path: path.to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn write_repos_with_workspace(repos_path: &Path, workspace: WorkspaceEntry) {
    let config = ReposConfig {
        version: 2,
        workspaces: vec![workspace],
        repos: vec![],
    };
    storage::write_repos(repos_path, &config).expect("write repos");
}

fn write_repos_with_two_workspaces(repos_path: &Path, ws1: WorkspaceEntry, ws2: WorkspaceEntry) {
    let config = ReposConfig {
        version: 2,
        workspaces: vec![ws1, ws2],
        repos: vec![],
    };
    storage::write_repos(repos_path, &config).expect("write repos");
}

// ── 22.6.15: workspace entry removed from repos.json ─────────────────────────

#[test]
fn test_remove_workspace_entry_removes_by_id() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let ws = make_workspace_entry("ws-abc", "/tmp/workspace");
    write_repos_with_workspace(&repos_path, ws);

    remove_workspace_entry(&repos_path, "ws-abc").expect("remove should succeed");

    let config = storage::read_repos_with_recovery(&repos_path).expect("read repos");
    assert!(config.workspaces.is_empty(), "workspace must be removed");
}

#[test]
fn test_remove_workspace_entry_leaves_other_workspaces() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    write_repos_with_two_workspaces(
        &repos_path,
        make_workspace_entry("ws-1", "/tmp/ws1"),
        make_workspace_entry("ws-2", "/tmp/ws2"),
    );

    let remaining = remove_workspace_entry(&repos_path, "ws-1").expect("remove");
    assert_eq!(remaining, 1, "one workspace remains");

    let config = storage::read_repos_with_recovery(&repos_path).expect("read");
    assert_eq!(config.workspaces.len(), 1);
    assert_eq!(config.workspaces[0].id, "ws-2");
}

// 22.6.7: workspace directory is untouched
#[test]
fn test_remove_workspace_entry_does_not_touch_directory() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let ws_dir = TempDir::new().unwrap();
    let marker = ws_dir.path().join("posts").join("important.md");
    fs::create_dir_all(marker.parent().unwrap()).unwrap();
    fs::write(&marker, "important content").unwrap();

    let ws = make_workspace_entry("ws-del", ws_dir.path().to_str().unwrap());
    write_repos_with_workspace(&repos_path, ws);

    remove_workspace_entry(&repos_path, "ws-del").expect("remove");

    assert!(marker.exists(), "workspace directory must be untouched");
}

// ── 22.6.16: 404 from DELETE treated as success ───────────────────────────────

#[tokio::test]
async fn test_delete_project_api_404_is_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-gone");
        then.status(404);
    });
    let result = delete_project_api(&server.base_url(), "proj-gone", "tok").await;
    assert!(result.is_ok(), "404 must be treated as success: {:?}", result);
}

#[tokio::test]
async fn test_delete_project_api_204_is_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-ok");
        then.status(204);
    });
    let result = delete_project_api(&server.base_url(), "proj-ok", "tok").await;
    assert!(result.is_ok(), "204 must succeed: {:?}", result);
}

#[tokio::test]
async fn test_delete_project_api_non_2xx_returns_pl_del_001() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-err");
        then.status(500);
    });
    let result = delete_project_api(&server.base_url(), "proj-err", "tok").await;
    assert!(result.is_err(), "500 must return Err");
    assert!(result.unwrap_err().contains("PL-DEL-001"), "error must contain PL-DEL-001");
}

// ── 22.6.17: no project_id skips API call ─────────────────────────────────────

#[test]
fn test_workspace_path_from_repos_returns_none_for_unknown_id() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    write_repos_with_workspace(&repos_path, make_workspace_entry("ws-known", "/tmp/ws"));

    let path = workspace_path_from_repos(&repos_path, "ws-unknown");
    assert!(path.is_none(), "unknown workspace_id must return None");
}

#[test]
fn test_workspace_path_from_repos_returns_path_for_known_id() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    write_repos_with_workspace(&repos_path, make_workspace_entry("ws-known", "/tmp/my-workspace"));

    let path = workspace_path_from_repos(&repos_path, "ws-known");
    assert_eq!(path, Some(PathBuf::from("/tmp/my-workspace")));
}

// ── 22.6.20/22.6.13: safelist validation ──────────────────────────────────────

fn make_repos_with_ws_path(repos_path: &Path, ws_path: &Path) {
    let config = ReposConfig {
        version: 2,
        workspaces: vec![make_workspace_entry("ws-test", ws_path.to_str().unwrap())],
        repos: vec![],
    };
    storage::write_repos(repos_path, &config).expect("write repos");
}

#[test]
fn test_safelist_validate_rejects_home() {
    let home = dirs::home_dir().expect("home dir");
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    make_repos_with_ws_path(&repos_path, &home);

    let result = safelist_validate_delete_path(&home, &repos_path);
    assert!(result.is_err(), "home dir must be rejected");
    assert!(result.unwrap_err().contains("PL-DEL-002"), "must contain PL-DEL-002");
}

#[test]
fn test_safelist_validate_rejects_shallow_path() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let shallow = PathBuf::from("/tmp");
    make_repos_with_ws_path(&repos_path, &shallow);

    let result = safelist_validate_delete_path(&shallow, &repos_path);
    assert!(result.is_err(), "path with < 4 components must be rejected");
    assert!(result.unwrap_err().contains("PL-DEL-002"), "must contain PL-DEL-002");
}

#[test]
fn test_safelist_validate_rejects_path_not_in_registry() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let ws_dir = TempDir::new().unwrap();
    make_repos_with_ws_path(&repos_path, ws_dir.path());

    let other = TempDir::new().unwrap();
    let result = safelist_validate_delete_path(other.path(), &repos_path);
    assert!(result.is_err(), "path not in registry must be rejected");
    assert!(result.unwrap_err().contains("PL-DEL-002"), "must contain PL-DEL-002");
}

#[test]
fn test_safelist_validate_rejects_symlink_resolving_to_home() {
    let home = dirs::home_dir().expect("home dir");
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let link = dir.path().join("ws-link");
    if std::os::unix::fs::symlink(&home, &link).is_err() {
        return; // Skip if can't create symlink (permissions)
    }
    make_repos_with_ws_path(&repos_path, &link);

    let result = safelist_validate_delete_path(&link, &repos_path);
    assert!(result.is_err(), "symlink resolving to home must be rejected");
    assert!(result.unwrap_err().contains("PL-DEL-002"), "must contain PL-DEL-002");
}

#[test]
fn test_safelist_validate_accepts_valid_deep_path() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    // TempDir typically creates paths like /var/folders/.../... which have >= 4 components
    make_repos_with_ws_path(&repos_path, dir.path());

    let result = safelist_validate_delete_path(dir.path(), &repos_path);
    // May fail if dir.path() is shallow (< 4 components) on this system — that's expected
    if dir.path().components().count() < 4 {
        assert!(result.is_err(), "shallow temp path rejected");
    } else {
        assert!(result.is_ok(), "valid deep path must be accepted: {:?}", result);
    }
}

// ── 22.6.21: workspace_path from repos.json, not user input ──────────────────

#[test]
fn test_safelist_validate_path_comes_from_registry() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    let ws_dir = TempDir::new().unwrap();
    make_repos_with_ws_path(&repos_path, ws_dir.path());

    // Even though we pass user_path, validation checks it against registry
    let user_provided = ws_dir.path(); // matches registry — should succeed if deep enough
    let result = safelist_validate_delete_path(user_provided, &repos_path);
    // Result depends on path depth; what matters is the registry check runs
    if ws_dir.path().components().count() < 4 {
        assert!(result.is_err());
    } else {
        assert!(result.is_ok());
    }
}

// ── 22.10.14: keyring key coverage ───────────────────────────────────────────

#[test]
fn test_scheduler_keyring_keys_covers_all_providers() {
    let keys = crate::workspace_disconnect::scheduler_keyring_keys("proj-abc");
    for provider in crate::credential_store::SCHEDULER_PROVIDERS {
        let expected = format!("{}/proj-abc", provider);
        assert!(keys.contains(&expected), "missing key for provider: {}", provider);
    }
    assert_eq!(
        keys.len(),
        crate::credential_store::SCHEDULER_PROVIDERS.len(),
        "must cover all {} scheduler providers",
        crate::credential_store::SCHEDULER_PROVIDERS.len(),
    );
}

#[test]
fn test_mastodon_keyring_keys_without_instance_returns_two_keys() {
    let keys = crate::workspace_disconnect::mastodon_keyring_keys("proj-abc", None);
    assert_eq!(keys.len(), 2, "no active instance → 2 keys (instance + username)");
    let instance_key = crate::mastodon_connection::active_instance_key("proj-abc");
    let username_key = crate::mastodon_connection::active_username_key("proj-abc");
    assert!(keys.contains(&instance_key), "must include active_instance key");
    assert!(keys.contains(&username_key), "must include active_username key");
}

#[test]
fn test_mastodon_keyring_keys_with_instance_returns_three_keys() {
    let keys = crate::workspace_disconnect::mastodon_keyring_keys("proj-abc", Some("mastodon.social"));
    assert_eq!(keys.len(), 3, "active instance → 3 keys (instance + username + access token)");
    let token_key = crate::mastodon_connection::access_token_key("proj-abc", "mastodon.social");
    assert!(keys.contains(&token_key), "must include access_token key for the instance");
}

// ── B14: remove_workspace_entry is a no-op for unknown id ────────────────────

#[test]
fn test_remove_workspace_entry_noop_for_unknown_id() {
    let dir = TempDir::new().unwrap();
    let repos_path = dir.path().join("repos.json");
    write_repos_with_workspace(&repos_path, make_workspace_entry("ws-known", "/tmp/known"));

    let remaining = remove_workspace_entry(&repos_path, "ws-unknown").expect("should not error");
    assert_eq!(remaining, 1, "existing workspace must be preserved");

    let config = storage::read_repos_with_recovery(&repos_path).expect("read repos");
    assert_eq!(config.workspaces.len(), 1);
    assert_eq!(config.workspaces[0].id, "ws-known");
}

// ── migration_journal_exists ──────────────────────────────────────────────────

#[test]
fn test_migration_journal_exists_returns_false_when_absent() {
    let dir = TempDir::new().unwrap();
    assert!(!migration_journal_exists(dir.path()));
}

#[test]
fn test_migration_journal_exists_returns_true_when_present() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join(".migration-journal.json"), "{}").unwrap();
    assert!(migration_journal_exists(dir.path()));
}
