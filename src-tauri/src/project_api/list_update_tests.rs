// SPDX-License-Identifier: BUSL-1.1

use super::{list_projects_with_client, update_project_org_login_with_client};
use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── list_projects ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_list_projects_returns_vec_on_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects");
        then.status(200).json_body(serde_json::json!({
            "projects": [{
                "id": "proj-1",
                "name": "Postlane",
                "workspace_type": "organization",
                "tier": "free",
                "billing_active": true,
                "is_owner": true,
                "status": "free_owned"
            }]
        }));
    });

    let result =
        list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
    let projects =
        result.expect("list_projects_with_client should succeed for 200 response");
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, "proj-1");
    assert_eq!(projects[0].name, "Postlane");
    assert_eq!(projects[0].workspace_type, "organization");
    assert_eq!(projects[0].tier, "free");
    assert!(projects[0].billing_active);
    assert!(projects[0].is_owner);
    assert_eq!(projects[0].status, "free_owned");
}

#[tokio::test]
async fn test_list_projects_deserializes_paid_owned_status() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects");
        then.status(200).json_body(serde_json::json!({
            "projects": [{
                "id": "proj-2",
                "name": "Acme",
                "workspace_type": "organization",
                "tier": "paid",
                "billing_active": true,
                "is_owner": true,
                "status": "paid_owned"
            }]
        }));
    });

    let result =
        list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
    let projects =
        result.expect("list_projects_with_client should succeed for 200 response");
    assert_eq!(projects[0].status, "paid_owned");
}

#[tokio::test]
async fn test_list_projects_returns_error_on_http_failure() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects");
        then.status(503);
    });

    let result =
        list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err(), "HTTP 503 must return Err");
}

#[tokio::test]
async fn test_list_projects_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects");
        then.status(401);
    });

    let result =
        list_projects_with_client(&build_test_client(), &server.base_url(), "expired-tok")
            .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        SESSION_EXPIRED_ERROR,
        "HTTP 401 must return session_expired error"
    );
}

// ── update_project_org_login ─────────────────────────────────────────────────

#[tokio::test]
async fn test_update_org_login_returns_ok_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH)
            .path("/v1/projects/proj-abc");
        then.status(200).json_body(serde_json::json!({ "ok": true }));
    });

    let result = update_project_org_login_with_client(
        "proj-abc",
        "my-org",
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_ok(), "200 response must map to Ok(()), got: {:?}", result);
}

#[tokio::test]
async fn test_update_org_login_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH)
            .path("/v1/projects/proj-abc");
        then.status(401);
    });

    let result = update_project_org_login_with_client(
        "proj-abc",
        "my-org",
        &build_test_client(),
        &server.base_url(),
        "expired-tok",
    )
    .await;
    assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR, "401 must return session expired");
}

#[tokio::test]
async fn test_update_org_login_returns_err_on_500() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH)
            .path("/v1/projects/proj-abc");
        then.status(500);
    });

    let result = update_project_org_login_with_client(
        "proj-abc",
        "my-org",
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_err(), "non-200 must return Err");
}

#[tokio::test]
async fn test_update_org_login_sends_provider_org_login_in_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH)
            .path("/v1/projects/proj-abc")
            .body_contains("\"provider_org_login\":\"acme\"");
        then.status(200).json_body(serde_json::json!({ "ok": true }));
    });

    let result = update_project_org_login_with_client(
        "proj-abc",
        "acme",
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_ok(), "patch body must contain provider_org_login field");
}

#[tokio::test]
async fn test_update_org_login_rejects_invalid_project_id() {
    let result = update_project_org_login_with_client(
        "../../etc/passwd",
        "my-org",
        &build_test_client(),
        "http://127.0.0.1:19993",
        "tok",
    )
    .await;
    assert!(result.is_err(), "invalid project_id must be rejected before network call");
}
