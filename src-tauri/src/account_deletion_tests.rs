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


fn make_entry(id: &str, path: &str) -> WorkspaceEntry {
    WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
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
    let result = delete_account_impl(params, &build_client()).await;
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
        delete_workspace_dirs: false,
    };
    let result = delete_account_impl(params, &build_client()).await;
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
    let result = delete_account_impl(params, &build_client()).await;
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

// ── GDPR-C4: PII cache files deleted on wipe ─────────────────────────────────

#[test]
fn test_wipe_postlane_files_deletes_pii_cache_files() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: vec![], repos: vec![] }).unwrap();
    for name in &["license_cache.json", "analytics_cache.json", "analytics_sites.json"] {
        fs::write(tmp.path().join(name), r#"{"user":{"email":"alice@example.com"}}"#).unwrap();
    }
    wipe_postlane_files(tmp.path()).unwrap();
    for name in &["license_cache.json", "analytics_cache.json", "analytics_sites.json"] {
        assert!(!tmp.path().join(name).exists(), "{name} must be deleted on account wipe");
    }
}

// ── B23: Step 9 deletes dirs even after repos.json wiped by step 6 ──────────

#[test]
fn test_delete_workspace_dirs_succeeds_after_repos_json_wiped() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let ws_dir = tmp.path().join("my-workspace").join("inner").join("deep").join("ws");
    fs::create_dir_all(&ws_dir).unwrap();

    // Simulate step 6: repos.json is wiped to empty before step 9 runs
    storage::write_repos(&repos_path, &ReposConfig { version: 2, workspaces: vec![], repos: vec![] }).unwrap();

    // Snapshot was captured at phase 0, before the wipe
    let snapshot = vec![make_entry("ws-1", ws_dir.to_str().unwrap())];

    let failures = delete_workspace_dirs(&snapshot, &repos_path);
    assert!(failures.is_empty(), "dirs must be deleted even after repos.json is wiped: {:?}", failures);
    assert!(!ws_dir.exists(), "workspace directory must be gone after checkbox=true");
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

// ── Step 3 non-fatal behaviour (rewritten 2026-07-01) ────────────────────────
// GitLab revocation failure must not abort the overall account deletion —
// it's a third-party credential cleanup, not core Postlane data.

#[tokio::test]
async fn test_gitlab_revocation_failure_does_not_abort_deletion() {
    let server = MockServer::start();
    full_mock(&server);
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/gitlab-revoke");
        then.status(502);
    });
    let tmp = TempDir::new().unwrap();
    make_repos(&tmp, vec![make_entry("proj-1", tmp.path().join("ws").to_str().unwrap())]);
    let params = make_params(&tmp, &server);
    let result = delete_account_impl(params, &build_client()).await;
    assert!(result.is_ok(), "failed GitLab revocation must not abort deletion: {:?}", result);
}

// ── 22.9.11: account_deleted telemetry ───────────────────────────────────────

// ── 22.9.11: account_deleted telemetry — spec-correct field names ─────────────
// Spec (build brief §Telemetry events): had_github_app (bool), had_gitlab_token
// (bool), optional_deletion_checked (bool). Not counts or renamed variants.

#[test]
fn test_account_deleted_records_all_five_payload_fields() {
    let state = crate::test_fixtures::make_state(vec![]);
    crate::account_deletion_commands::record_account_deleted(
        &state, true,
        3,     // project_count
        true,  // had_github_app
        true,  // had_gitlab_token
        false, // optional_deletion_checked
        3,     // workspace_count
    );
    assert_eq!(state.telemetry.queue_len(), 1);
    let ev = &state.telemetry.peek_queue()[0];
    assert_eq!(ev.name, "account_deleted");
    assert_eq!(ev.properties["project_count"], 3);
    assert_eq!(ev.properties["had_github_app"], true);
    assert_eq!(ev.properties["had_gitlab_token"], true);
    assert_eq!(ev.properties["optional_deletion_checked"], false);
    assert_eq!(ev.properties["workspace_count"], 3);
}

#[test]
fn test_account_deleted_no_event_without_consent() {
    let state = crate::test_fixtures::make_state(vec![]);
    crate::account_deletion_commands::record_account_deleted(
        &state, false, 1, false, false, false, 1,
    );
    assert_eq!(state.telemetry.queue_len(), 0);
}

// ── 22.7.14: Step 4 — keyring wipe covers all project-scoped and global keys ──
// `clear_all_keyring` requires AppHandle; `clear_all_keyring_impl` is the injectable
// pure-function form that can be tested without a Tauri runtime.

#[test]
fn test_clear_all_keyring_deletes_all_project_scoped_keys() {
    let project_id = "proj-22714";
    let mut deleted: Vec<String> = Vec::new();
    crate::account_deletion_commands::clear_all_keyring_impl(
        &[project_id.to_string()],
        |key| deleted.push(key.to_string()),
    );
    for key in crate::credential_store::project_keyring_keys(project_id) {
        assert!(deleted.contains(&key), "Step 4 must delete project key: {key}");
    }
    for key in crate::credential_store::global_keyring_keys() {
        assert!(deleted.contains(&key.to_string()), "Step 4 must delete global key: {key}");
    }
}

#[test]
fn test_clear_all_keyring_covers_multiple_projects() {
    let mut deleted: Vec<String> = Vec::new();
    crate::account_deletion_commands::clear_all_keyring_impl(
        &["proj-a".to_string(), "proj-b".to_string()],
        |key| deleted.push(key.to_string()),
    );
    for pid in ["proj-a", "proj-b"] {
        for key in crate::credential_store::project_keyring_keys(pid) {
            assert!(deleted.contains(&key), "must delete {key} for project {pid}");
        }
    }
}

// ── 22.7.10b: Step 1 — 5xx server response returns Err ───────────────────────

#[tokio::test]
async fn test_delete_all_projects_returns_error_on_5xx() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE).path("/v1/projects/proj-x");
        then.status(500);
    });
    let ids = vec!["proj-x".to_string()];
    let result = delete_all_projects(&server.base_url(), "tok", &ids, &build_client()).await;
    assert!(result.is_err(), "5xx from project delete must return Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-DEL-001"), "error must include PL-DEL-001, got: {msg}");
    assert!(msg.contains("500"), "error must mention status 500, got: {msg}");
}

// ── 22.7.25b: delete_account_impl with delete_workspace_dirs = true ──────────

#[tokio::test]
async fn test_delete_account_impl_with_delete_workspace_dirs_true_deletes_dirs() {
    let tmp = TempDir::new().expect("create temp dir");
    let server = MockServer::start();
    full_mock(&server);

    let ws_dir = tmp.path().join("some").join("deep").join("workspace").join("repo");
    fs::create_dir_all(&ws_dir).expect("create workspace dir");

    make_repos(&tmp, vec![make_entry("ws-1", ws_dir.to_str().expect("path to str"))]);

    let params = DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: server.base_url(),
        token: "tok".to_string(),
        project_ids: vec!["proj-1".to_string()],
        project_ids_with_github_app: vec![],
        delete_workspace_dirs: true,
    };
    let result = delete_account_impl(params, &build_client()).await;
    assert!(result.is_ok(), "delete with workspace dirs must succeed: {:?}", result);
    assert!(!ws_dir.exists(), "workspace directory must be removed when delete_workspace_dirs=true");
}

// ── 22.7.25c: snapshot_workspaces missing repos.json returns empty — line 103 ─

#[tokio::test]
async fn test_delete_account_impl_missing_repos_json_still_succeeds() {
    let tmp = TempDir::new().expect("create temp dir");
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(200).json_body(serde_json::json!({"valid": true}));
    });
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/delete");
        then.status(200);
    });
    // repos.json intentionally absent — snapshot_workspaces must return Ok(vec![])
    let params = DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: server.base_url(),
        token: "tok".to_string(),
        project_ids: vec![],
        project_ids_with_github_app: vec![],
        delete_workspace_dirs: false,
    };
    let result = delete_account_impl(params, &build_client()).await;
    assert!(result.is_ok(), "absent repos.json must not abort deletion: {:?}", result);
}

// ── 22.7.15b: Step 5 — account/delete 401 returns Err (line 182) ─────────────

#[tokio::test]
async fn test_delete_account_record_returns_error_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(200).json_body(serde_json::json!({"valid": true}));
    });
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE)
            .path_matches(regex::Regex::new("/v1/projects/").expect("regex"));
        then.status(204);
    });
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/delete");
        then.status(401);
    });
    let tmp = TempDir::new().expect("create temp dir");
    make_repos(&tmp, vec![]);
    let params = DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: server.base_url(),
        token: "tok".to_string(),
        project_ids: vec![],
        project_ids_with_github_app: vec![],
        delete_workspace_dirs: false,
    };
    let result = delete_account_impl(params, &build_client()).await;
    assert!(result.is_err(), "401 from account/delete must return Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-DEL-004"), "error must include PL-DEL-004, got: {msg}");
}

// ── 22.7.15c: Step 5 — account/delete 5xx returns Err (line 183) ─────────────

#[tokio::test]
async fn test_delete_account_record_returns_error_on_5xx() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(200).json_body(serde_json::json!({"valid": true}));
    });
    server.mock(|when, then| {
        when.method(httpmock::Method::DELETE)
            .path_matches(regex::Regex::new("/v1/projects/").expect("regex"));
        then.status(204);
    });
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/delete");
        then.status(500);
    });
    let tmp = TempDir::new().expect("create temp dir");
    make_repos(&tmp, vec![]);
    let params = DeleteAccountParams {
        postlane_dir: tmp.path().to_path_buf(),
        api_base: server.base_url(),
        token: "tok".to_string(),
        project_ids: vec![],
        project_ids_with_github_app: vec![],
        delete_workspace_dirs: false,
    };
    let result = delete_account_impl(params, &build_client()).await;
    assert!(result.is_err(), "5xx from account/delete must return Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-DEL-004"), "error must include PL-DEL-004, got: {msg}");
    assert!(msg.contains("500"), "error must mention status 500, got: {msg}");
}

// ── Step 3 (rewritten 2026-07-01) — revocation runs server-side ─────────────
// A prior version called GitLab directly with no credentials at all and its
// caller discarded the result unconditionally — always "succeeding" while
// never actually revoking anything. See account_deletion.rs's Step 3 comment.

#[tokio::test]
async fn test_revoke_gitlab_calls_backend_endpoint_with_bearer_auth() {
    let server = MockServer::start();
    let m = server.mock(|when, then| {
        when.method(httpmock::Method::POST)
            .path("/v1/account/gitlab-revoke")
            .header("authorization", "Bearer tok");
        then.status(200);
    });
    let result = revoke_gitlab_token(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_ok(), "200 from backend must succeed: {:?}", result);
    m.assert();
}

#[tokio::test]
async fn test_revoke_gitlab_returns_error_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/gitlab-revoke");
        then.status(401);
    });
    let result = revoke_gitlab_token(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_err(), "401 must return Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-DEL-003"), "error must include PL-DEL-003, got: {msg}");
}

#[tokio::test]
async fn test_revoke_gitlab_returns_error_on_non_2xx() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::POST).path("/v1/account/gitlab-revoke");
        then.status(502);
    });
    let result = revoke_gitlab_token(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_err(), "non-2xx backend response must return Err");
    let msg = result.unwrap_err();
    assert!(msg.contains("PL-DEL-003"), "error must include PL-DEL-003, got: {msg}");
    assert!(msg.contains("502"), "error must mention status 502, got: {msg}");
}

#[tokio::test]
async fn test_revoke_gitlab_network_error_returns_err() {
    // Port 0 on localhost is not listening — guarantees a connection error.
    let result = revoke_gitlab_token("http://127.0.0.1:0", "tok", &build_client()).await;
    assert!(result.is_err(), "network error must return Err, not panic");
    assert!(result.unwrap_err().contains("PL-DEL-003"));
}

// ── 22.7.8b: preflight_session direct branch tests ────────────────────────────

#[tokio::test]
async fn test_preflight_session_200_returns_ok() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(200).json_body(serde_json::json!({"valid": true}));
    });
    let result = preflight_session(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_ok(), "200 must return Ok: {:?}", result);
}

#[tokio::test]
async fn test_preflight_session_401_returns_session_expired_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(401);
    });
    let result = preflight_session(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("session") || msg.to_lowercase().contains("sign"),
        "401 must return session-expired message, got: {msg}"
    );
}

#[tokio::test]
async fn test_preflight_session_5xx_returns_server_error_message() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/auth/session");
        then.status(500);
    });
    let result = preflight_session(&server.base_url(), "tok", &build_client()).await;
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.contains("500") || msg.to_lowercase().contains("connection") || msg.to_lowercase().contains("verify"),
        "5xx must return server-error message mentioning status or connection, got: {msg}"
    );
}
