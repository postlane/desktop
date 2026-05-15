// SPDX-License-Identifier: BUSL-1.1

use super::create_project_with_client;
use crate::project_registry::CreateProjectError;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── create_project ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_creates_project_returns_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/projects");
        then.status(200).json_body(serde_json::json!({
            "project_id": "new-uuid-abc", "name": "My Project", "tier": "free",
            "workspace_type": "personal"
        }));
    });

    let result = create_project_with_client(
        "My Project",
        "personal",
        None,
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    let (id, name, _wt) =
        result.expect("create_project_with_client should succeed for 200 response");
    assert_eq!(id, "new-uuid-abc");
    assert_eq!(name, "My Project");
}

#[tokio::test]
async fn test_create_project_rejects_empty_name_before_network_call() {
    let result = create_project_with_client(
        "",
        "personal",
        None,
        None,
        &build_test_client(),
        "http://127.0.0.1:19996",
        "tok",
    )
    .await;
    assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
}

#[tokio::test]
async fn test_create_project_rejects_whitespace_only_name() {
    let result = create_project_with_client(
        "   ",
        "personal",
        None,
        None,
        &build_test_client(),
        "http://127.0.0.1:19996",
        "tok",
    )
    .await;
    assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
}

#[tokio::test]
async fn test_create_project_returns_error_on_402() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/projects");
        then.status(402)
            .json_body(serde_json::json!({ "error": "no_free_slot" }));
    });

    let result = create_project_with_client(
        "Second Project",
        "personal",
        None,
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(matches!(result, Err(CreateProjectError::NoFreeSlot)));
}

#[tokio::test]
async fn test_create_project_passes_workspace_type() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/projects")
            .body_contains("\"workspace_type\":\"organization\"");
        then.status(200).json_body(serde_json::json!({
            "project_id": "org-uuid-abc", "name": "Acme", "tier": "free",
            "workspace_type": "organization"
        }));
    });

    let result = create_project_with_client(
        "Acme",
        "organization",
        None,
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    let (id, name, wt) =
        result.expect("create_project with organization workspace_type should succeed");
    assert_eq!(id, "org-uuid-abc");
    assert_eq!(name, "Acme");
    assert_eq!(wt, "organization");
}

#[tokio::test]
async fn test_create_project_rejects_invalid_workspace_type() {
    let result = create_project_with_client(
        "Acme",
        "enterprise",
        None,
        None,
        &build_test_client(),
        "http://127.0.0.1:19994",
        "tok",
    )
    .await;
    assert!(matches!(
        result,
        Err(CreateProjectError::InvalidWorkspaceType(_))
    ));
}

#[tokio::test]
async fn test_create_project_sends_provider_org_login_in_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/projects")
            .body_contains("\"provider_org_login\":\"postlane\"");
        then.status(200).json_body(serde_json::json!({
            "project_id": "org-proj-uuid", "name": "postlane", "tier": "free",
            "workspace_type": "organization"
        }));
    });

    let result = create_project_with_client(
        "postlane",
        "organization",
        Some("postlane"),
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(
        result.is_ok(),
        "should send provider_org_login and succeed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_create_project_sends_provider_group_path_in_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/projects")
            .body_contains("\"provider_group_path\":\"acme-corp\"");
        then.status(200).json_body(serde_json::json!({
            "project_id": "gl-proj-uuid", "name": "acme-corp", "tier": "free",
            "workspace_type": "organization"
        }));
    });

    let result = create_project_with_client(
        "acme-corp",
        "organization",
        None,
        Some("acme-corp"),
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(
        result.is_ok(),
        "should send provider_group_path and succeed: {:?}",
        result
    );
}

#[tokio::test]
async fn test_create_project_returns_org_already_registered_on_409() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/projects");
        then.status(409)
            .json_body(serde_json::json!({ "error": "org_already_registered" }));
    });

    let result = create_project_with_client(
        "postlane",
        "organization",
        Some("postlane"),
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(
        matches!(result, Err(CreateProjectError::OrgAlreadyRegistered)),
        "HTTP 409 must map to OrgAlreadyRegistered, got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_create_project_with_org_login_does_not_require_name() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/projects");
        then.status(200).json_body(serde_json::json!({
            "project_id": "org-uuid", "name": "postlane", "tier": "free",
            "workspace_type": "organization"
        }));
    });

    let result = create_project_with_client(
        "",
        "organization",
        Some("postlane"),
        None,
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(
        result.is_ok(),
        "empty name with org_login should not fail validation: {:?}",
        result
    );
}
