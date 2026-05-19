// SPDX-License-Identifier: BUSL-1.1

use super::{
    get_project_voice_guide_full_with_client, get_project_voice_guide_with_client,
    save_project_voice_guide_with_client,
};
use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

// ── get_project_voice_guide_full ─────────────────────────────────────────────

#[tokio::test]
async fn test_full_returns_both_fields_in_one_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-full1");
        then.status(200).json_body(serde_json::json!({
            "voice_guide": "Direct tone.",
            "voice_guide_fields": { "tone": "direct" }
        }));
    });
    let result = get_project_voice_guide_full_with_client(
        "proj-full1", &build_client(), &server.base_url(), "tok",
    ).await;
    let data = result.expect("should succeed");
    assert_eq!(data.voice_guide, Some("Direct tone.".to_string()));
    assert_eq!(data.voice_guide_fields, Some(serde_json::json!({ "tone": "direct" })));
    mock.assert_hits(1);
}

#[tokio::test]
async fn test_full_returns_none_fields_when_both_null() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-full2");
        then.status(200).json_body(serde_json::json!({
            "voice_guide": null, "voice_guide_fields": null
        }));
    });
    let data = get_project_voice_guide_full_with_client(
        "proj-full2", &build_client(), &server.base_url(), "tok",
    ).await.unwrap();
    assert_eq!(data.voice_guide, None);
    assert_eq!(data.voice_guide_fields, None);
}

#[tokio::test]
async fn test_full_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-full3");
        then.status(401);
    });
    let err = get_project_voice_guide_full_with_client(
        "proj-full3", &build_client(), &server.base_url(), "tok",
    ).await.unwrap_err();
    assert_eq!(err, SESSION_EXPIRED_ERROR);
}

#[tokio::test]
async fn test_full_returns_err_on_non_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-full4");
        then.status(503);
    });
    assert!(get_project_voice_guide_full_with_client(
        "proj-full4", &build_client(), &server.base_url(), "tok",
    ).await.is_err());
}

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
