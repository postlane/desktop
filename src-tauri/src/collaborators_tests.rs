// SPDX-License-Identifier: BUSL-1.1
// Tests for collaborators.rs — checklist 24.4.14/24.4.14a.

use super::{list_project_collaborators_with_client, remove_collaborator_with_client, update_collaborator_role_with_client};
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;
use httpmock::Method::PATCH;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── list_project_collaborators ───────────────────────────────────────────────

#[tokio::test]
async fn test_list_returns_collaborators_on_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-1/collaborators");
        then.status(200).json_body(serde_json::json!({
            "collaborators": [
                { "user_id": "u1", "role": "admin", "added_at": "2026-01-01T00:00:00Z", "display_name": "Ada", "avatar_url": null }
            ]
        }));
    });

    let result = list_project_collaborators_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    let collaborators = result.expect("expected Ok");
    assert_eq!(collaborators.len(), 1);
    assert_eq!(collaborators[0].user_id, "u1");
    assert_eq!(collaborators[0].role, "admin");
    assert_eq!(collaborators[0].display_name.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn test_list_returns_err_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-1/collaborators");
        then.status(403).json_body(serde_json::json!({ "error": "forbidden" }));
    });

    let result = list_project_collaborators_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_list_returns_err_on_network_failure() {
    let result =
        list_project_collaborators_with_client("proj-1", &build_test_client(), "http://127.0.0.1:19996", "tok").await;
    assert!(result.is_err());
}

// ── update_collaborator_role ─────────────────────────────────────────────────

#[tokio::test]
async fn test_update_role_succeeds_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(PATCH)
            .path("/v1/projects/proj-1/collaborators/u1")
            .json_body(serde_json::json!({ "role": "admin" }));
        then.status(200)
            .json_body(serde_json::json!({ "project_id": "proj-1", "user_id": "u1", "role": "admin" }));
    });

    let result =
        update_collaborator_role_with_client("proj-1", "u1", "admin", &build_test_client(), &server.base_url(), "tok")
            .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_update_role_returns_err_on_404() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(PATCH).path("/v1/projects/proj-1/collaborators/u1");
        then.status(404).json_body(serde_json::json!({ "error": "not_a_collaborator" }));
    });

    let result =
        update_collaborator_role_with_client("proj-1", "u1", "admin", &build_test_client(), &server.base_url(), "tok")
            .await;
    assert!(result.is_err());
}

// ── remove_collaborator ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_remove_succeeds_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/v1/projects/proj-1/collaborators/u1");
        then.status(200).json_body(serde_json::json!({ "removed": true }));
    });

    let result = remove_collaborator_with_client("proj-1", "u1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_remove_returns_err_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/v1/projects/proj-1/collaborators/u1");
        then.status(403).json_body(serde_json::json!({ "error": "forbidden" }));
    });

    let result = remove_collaborator_with_client("proj-1", "u1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}
