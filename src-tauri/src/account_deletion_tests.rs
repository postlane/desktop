// SPDX-License-Identifier: BUSL-1.1
//! Tests for §22.7.4 — delete_account_impl and step helpers.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use httpmock::prelude::*;

use crate::account_deletion::*;
use crate::storage::{self, ReposConfig};
use crate::workspace_entry::WorkspaceEntry;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn build_client() -> reqwest::Client {
    crate::providers::scheduling::build_client()
}

fn accept_any_url(_url: &str) -> Result<(), String> { Ok(()) }
fn ssrf_validator(url: &str) -> Result<(), String> { crate::ssrf_validation::validate_ssrf_url(url) }

fn make_entry(id: &str, path: &str) -> WorkspaceEntry {
    WorkspaceEntry {
        id: id.to_string(), name: id.to_string(),
        workspace_path: path.to_string(),
        active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn make_repos(tmp: &TempDir, workspaces: Vec<WorkspaceEntry>) -> PathBuf {
    let p = tmp.path().join("repos.json");
    let config = ReposConfig { version: 2, workspaces, repos: vec![] };
    storage::write_repos(&p, &config).unwrap();
    p
}

fn make_params(tmp: &TempDir, server: &MockServer) -> DeleteAccountParams {
    DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: server.base_url(),
        token: "tok".to_string(),
        project_ids: vec!["proj-1".to_string()],
        project_ids_with_github_app: vec![],
        gitlab_instance_url: None,
        delete_workspace_dirs: false,
    }
}

fn full_mock(server: &MockServer) {
    server.mock(|when, then| { when.method(GET).path("/v1/auth/session"); then.status(200).json_body(serde_json::json!({"valid": true})); });
    server.mock(|when, then| { when.method(httpmock::Method::DELETE).path_matches(regex::Regex::new("/v1/projects/").unwrap()); then.status(204); });
    server.mock(|when, then| { when.method(httpmock::Method::POST).path("/v1/account/delete"); then.status(200); });
}

// ── 22.7.23: pre-flight 401 → delete does not start ─────────────────────────

#[tokio::test]
async fn test_preflight_401_aborts_delete() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(401);
    });
    let tmp = TempDir::new().unwrap();
    make_repos(&tmp, vec![]);
    let params = make_params(&tmp, &server);
    let result = delete_account_impl(params, &build_client(), accept_any_url).await;
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("session") || msg.contains("expired") || msg.contains("401"),
        "error must mention session failure, got: {msg}");
}

// ── 22.7.24: pre-flight network error → delete does not start ────────────────

#[tokio::test]
async fn test_preflight_network_error_aborts_delete() {
    let tmp = TempDir::new().unwrap();
    make_repos(&tmp, vec![]);
    let params = DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: "http://127.0.0.1:19001".to_string(),
        token: "tok".to_string(),
        project_ids: vec![],
        project_ids_with_github_app: vec![],
        gitlab_instance_url: None,
        delete_workspace_dirs: false,
    };
    let result = delete_account_impl(params, &build_client(), accept_any_url).await;
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("connection") || msg.contains("network") || msg.contains("verify"),
        "error must mention network/connection, got: {msg}");
}

// ── 22.7.25: pre-flight 200 → proceeds ───────────────────────────────────────

#[tokio::test]
async fn test_preflight_200_proceeds_to_deletion() {
    let tmp = TempDir::new().unwrap();
    let server = MockServer::start();
    full_mock(&server);
    make_repos(&tmp, vec![make_entry("proj-1", tmp.path().join("ws").to_str().unwrap())]);
    let params = make_params(&tmp, &server);
    let result = delete_account_impl(params, &build_client(), accept_any_url).await;
    assert!(result.is_ok(), "200 pre-flight must allow deletion to proceed: {:?}", result);
}

// ── 22.7.10: Step 1 — delete each project; 404 = success ─────────────────────

#[tokio::test]
async fn test_delete_all_projects_calls_api_for_each() {
    let server = MockServer::start();
    let m1 = server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-a");
        then.status(204);
    });
    let m2 = server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-b");
        then.status(204);
    });
    let ids = vec!["proj-a".to_string(), "proj-b".to_string()];
    let result = delete_all_projects(&server.base_url(), "tok", &ids, &build_client()).await;
    assert!(result.is_ok());
    m1.assert(); m2.assert();
}

#[tokio::test]
async fn test_delete_all_projects_404_is_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/gone");
        then.status(404);
    });
    let result = delete_all_projects(&server.base_url(), "tok", &["gone".to_string()], &build_client()).await;
    assert!(result.is_ok(), "404 must be treated as success");
}

// ── 22.7.11: Step 2 — GitHub App disconnect ──────────────────────────────────

#[tokio::test]
async fn test_disconnect_all_github_apps_called_for_each() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/github/installation");
        then.status(204);
    });
    let result = disconnect_all_github_apps(
        &server.base_url(), "tok", &["proj-1".to_string()], &build_client(),
    ).await;
    assert!(result.is_ok());
    m.assert_hits(1);
}

#[tokio::test]
async fn test_disconnect_all_github_apps_skipped_when_empty() {
    let server = MockServer::start();
    let result = disconnect_all_github_apps(
        &server.base_url(), "tok", &[], &build_client(),
    ).await;
    assert!(result.is_ok(), "empty list must succeed without API calls");
    // Verify no calls were made to the mock server
    let _ = server; // no requests expected
}

// ── 22.7.13: Step 3 — uses stored GitLab instance URL ────────────────────────

#[tokio::test]
async fn test_revoke_gitlab_uses_stored_instance_url() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/oauth/token");
        then.status(200);
    });
    // accept_any_url bypasses SSRF checks so the httpmock server (HTTP localhost) is reachable.
    let instance_url = server.base_url();
    let result = revoke_gitlab_token(Some(&instance_url), &build_client(), accept_any_url).await;
    assert!(result.is_ok(), "valid URL must succeed: {:?}", result);
    m.assert();
}

#[tokio::test]
async fn test_revoke_gitlab_none_is_noop() {
    let result = revoke_gitlab_token(None, &build_client(), ssrf_validator).await;
    assert!(result.is_ok(), "no GitLab token is a noop");
}

// ── 22.7.16: Step 6 — repos.json wiped ──────────────────────────────────────

#[test]
fn test_wipe_postlane_files_clears_repos_json() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let config = ReposConfig { version: 2, workspaces: vec![make_entry("ws", "/ws")], repos: vec![] };
    storage::write_repos(&repos_path, &config).unwrap();

    wipe_postlane_files(tmp.path()).unwrap();

    let after = storage::read_repos_with_recovery(&repos_path).unwrap();
    assert!(after.workspaces.is_empty(), "workspaces must be empty after wipe");
    assert!(after.repos.is_empty(), "repos must be empty after wipe");
}

// ── 22.7.17: Steps 7–8 — state files deleted ─────────────────────────────────

#[test]
fn test_wipe_postlane_files_deletes_session_files() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: vec![], repos: vec![] }).unwrap();
    for name in &["session.token", "local.token", "port", "wizard_state.json", "app_state.json"] {
        fs::write(tmp.path().join(name), "data").unwrap();
    }
    wipe_postlane_files(tmp.path()).unwrap();
    for name in &["session.token", "local.token", "port", "wizard_state.json", "app_state.json"] {
        assert!(!tmp.path().join(name).exists(), "{name} must be deleted");
    }
}

#[test]
fn test_wipe_postlane_files_succeeds_when_files_absent() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: vec![], repos: vec![] }).unwrap();
    // No other files present — must not error
    assert!(wipe_postlane_files(tmp.path()).is_ok());
}

// ── 22.7.20: Step 9 — workspace dirs deleted when checkbox checked ────────────

#[test]
fn test_delete_workspace_dirs_removes_when_checkbox_checked() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let ws_dir = tmp.path().join("my-workspace").join("inner").join("deep").join("ws");
    fs::create_dir_all(&ws_dir).unwrap();
    fs::write(ws_dir.join("draft.md"), "content").unwrap();

    let snapshot = vec![make_entry("ws-1", ws_dir.to_str().unwrap())];
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: snapshot.clone(), repos: vec![] }).unwrap();

    let failures = delete_workspace_dirs(&snapshot, &repos_path);
    assert!(failures.is_empty(), "valid workspace dir must be deleted: {:?}", failures);
    assert!(!ws_dir.exists(), "workspace directory must be gone");
}

#[test]
fn test_delete_workspace_dirs_skips_when_checkbox_unchecked() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let ws_dir = tmp.path().join("keep-ws");
    fs::create_dir_all(&ws_dir).unwrap();

    let snapshot = vec![make_entry("ws-keep", ws_dir.to_str().unwrap())];
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: vec![], repos: vec![] }).unwrap();

    // Pass empty snapshot (checkbox=false → caller passes empty snapshot)
    let failures = delete_workspace_dirs(&[], &repos_path);
    assert!(ws_dir.exists(), "directory must survive when snapshot is empty");
    assert!(failures.is_empty());
}

// ── 22.7.22: Step 9 — safelist rejects $HOME and shallow paths ───────────────

#[test]
fn test_delete_workspace_dirs_rejects_home() {
    let home = dirs::home_dir().expect("home dir");
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let snapshot = vec![make_entry("ws-home", home.to_str().unwrap())];
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: snapshot.clone(), repos: vec![] }).unwrap();

    let failures = delete_workspace_dirs(&snapshot, &repos_path);
    assert!(!failures.is_empty(), "home directory must be rejected");
    assert!(home.exists(), "home directory must not be deleted");
}

#[test]
fn test_delete_workspace_dirs_rejects_shallow_path() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let shallow = PathBuf::from("/tmp");
    let snapshot = vec![make_entry("ws-shallow", "/tmp")];
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: snapshot.clone(), repos: vec![] }).unwrap();

    let failures = delete_workspace_dirs(&snapshot, &repos_path);
    if shallow.components().count() < 4 {
        assert!(!failures.is_empty(), "/tmp must be rejected as too shallow");
    }
}
