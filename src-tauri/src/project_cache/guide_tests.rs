// SPDX-License-Identifier: BUSL-1.1

use super::{
    get_project_voice_guide_with_client, save_project_voice_guide_with_client,
};
use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

// ── SessionExpired on 401 ────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_project_voice_guide_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(401);
    });
    let result = get_project_voice_guide_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "expired-tok",
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        SESSION_EXPIRED_ERROR,
        "HTTP 401 must return session_expired error"
    );
}

#[tokio::test]
async fn test_save_project_voice_guide_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
        then.status(401);
    });
    let result = save_project_voice_guide_with_client(
        "proj-abc",
        "Guide text.",
        &build_client(),
        &server.base_url(),
        "expired-tok",
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        SESSION_EXPIRED_ERROR,
        "HTTP 401 must return session_expired error"
    );
}

// ── get_project_voice_guide ──────────────────────────────────────────────────

#[tokio::test]
async fn test_get_voice_guide_returns_none_when_null() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(200).json_body(serde_json::json!({ "voice_guide": null }));
    });
    let result = get_project_voice_guide_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(result, Ok(None));
}

#[tokio::test]
async fn test_get_voice_guide_returns_some_when_set() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(200)
            .json_body(serde_json::json!({ "voice_guide": "Direct and technical." }));
    });
    let result = get_project_voice_guide_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(result, Ok(Some("Direct and technical.".to_string())));
}

#[tokio::test]
async fn test_get_voice_guide_returns_err_on_non_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(404);
    });
    let result = get_project_voice_guide_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_get_voice_guide_returns_err_on_network_failure() {
    let result = get_project_voice_guide_with_client(
        "proj-abc",
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err());
}

// ── save_project_voice_guide ─────────────────────────────────────────────────

#[tokio::test]
async fn test_saves_voice_guide() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
        then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
    });
    save_project_voice_guide_with_client(
        "proj-abc",
        "Direct and technical.",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await
    .expect("should succeed");
    mock.assert();
}

#[tokio::test]
async fn test_accepts_empty_voice_guide() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
        then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
    });
    save_project_voice_guide_with_client(
        "proj-abc",
        "",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await
    .expect("should accept empty voice guide");
}

#[tokio::test]
async fn test_save_project_voice_guide_returns_error_on_http_failure() {
    let result = save_project_voice_guide_with_client(
        "proj-abc",
        "Direct.",
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err(), "network failure must return Err");
}

#[tokio::test]
async fn test_save_project_voice_guide_rejects_voice_guide_exceeding_5000_chars() {
    let long_guide = "x".repeat(5001);
    let result = save_project_voice_guide_with_client(
        "proj-abc",
        &long_guide,
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("5000"), "error must mention the limit");
}

// ── validate_project_id integration tests ────────────────────────────────────

#[tokio::test]
async fn test_get_voice_guide_rejects_invalid_project_id() {
    let result = get_project_voice_guide_with_client(
        "proj/../../evil",
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid"));
}

#[tokio::test]
async fn test_save_voice_guide_rejects_invalid_project_id() {
    let result = save_project_voice_guide_with_client(
        "proj name",
        "guide text",
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid"));
}
