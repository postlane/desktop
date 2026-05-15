// SPDX-License-Identifier: BUSL-1.1

use super::{
    get_project_voice_guide_cached, get_voice_guide_fields_with_client,
    save_project_voice_guide_and_fields_with_client, save_project_voice_guide_with_client,
};
use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

// ── voice guide caching ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_voice_guide_cache_hit_avoids_second_request() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache1");
        then.status(200).json_body(serde_json::json!({ "voice_guide": "Concise tone." }));
    });
    let r1 = get_project_voice_guide_cached(
        "proj-vgcache1",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await;
    assert_eq!(r1.unwrap(), Some("Concise tone.".to_string()));
    let r2 = get_project_voice_guide_cached(
        "proj-vgcache1",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await;
    assert_eq!(r2.unwrap(), Some("Concise tone.".to_string()));
    mock.assert_hits(1);
}

#[tokio::test]
async fn test_voice_guide_cache_expires_with_zero_ttl() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache2");
        then.status(200).json_body(serde_json::json!({ "voice_guide": "Fresh voice." }));
    });
    get_project_voice_guide_cached(
        "proj-vgcache2",
        &build_client(),
        &server.base_url(),
        "tok",
        0,
    )
    .await
    .unwrap();
    get_project_voice_guide_cached(
        "proj-vgcache2",
        &build_client(),
        &server.base_url(),
        "tok",
        0,
    )
    .await
    .unwrap();
    mock.assert_hits(2);
}

#[tokio::test]
async fn test_voice_guide_cache_invalidated_on_save() {
    let server = MockServer::start();
    let get_mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache3");
        then.status(200).json_body(serde_json::json!({ "voice_guide": "Old voice." }));
    });
    let patch_mock = server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-vgcache3");
        then.status(200);
    });
    get_project_voice_guide_cached(
        "proj-vgcache3",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await
    .unwrap();
    save_project_voice_guide_with_client(
        "proj-vgcache3",
        "New voice.",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await
    .unwrap();
    get_project_voice_guide_cached(
        "proj-vgcache3",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await
    .unwrap();
    get_mock.assert_hits(2);
    patch_mock.assert_hits(1);
}

#[tokio::test]
async fn test_voice_guide_cache_miss_for_none_response() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache4");
        then.status(200).json_body(serde_json::json!({ "voice_guide": null }));
    });
    get_project_voice_guide_cached(
        "proj-vgcache4",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await
    .unwrap();
    get_project_voice_guide_cached(
        "proj-vgcache4",
        &build_client(),
        &server.base_url(),
        "tok",
        3600,
    )
    .await
    .unwrap();
    mock.assert_hits(2);
}

// ── get_voice_guide_fields ────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_voice_guide_fields_returns_none_when_null() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(200)
            .json_body(serde_json::json!({ "voice_guide": "", "voice_guide_fields": null }));
    });
    let result = get_voice_guide_fields_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(result, Ok(None));
}

#[tokio::test]
async fn test_get_voice_guide_fields_returns_some_when_set() {
    let fields = serde_json::json!({ "description": "Builder", "tone": "Direct" });
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(200).json_body(
            serde_json::json!({ "voice_guide": "", "voice_guide_fields": fields.clone() }),
        );
    });
    let result = get_voice_guide_fields_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(result, Ok(Some(fields)));
}

#[tokio::test]
async fn test_get_voice_guide_fields_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
        then.status(401);
    });
    let result = get_voice_guide_fields_with_client(
        "proj-abc",
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
}

#[tokio::test]
async fn test_get_voice_guide_fields_returns_err_on_network_failure() {
    let result = get_voice_guide_fields_with_client(
        "proj-abc",
        &build_client(),
        "http://127.0.0.1:1",
        "tok",
    )
    .await;
    assert!(result.is_err());
}

// ── save_project_voice_guide_and_fields ───────────────────────────────────────

#[tokio::test]
async fn test_save_voice_guide_and_fields_sends_fields_in_body() {
    let server = MockServer::start();
    let fields = serde_json::json!({ "description": "Builder" });
    let mock = server.mock(|when, then| {
        when.method(httpmock::Method::PATCH)
            .path("/v1/projects/proj-abc")
            .json_body(serde_json::json!({
                "voice_guide": "Direct.",
                "voice_guide_fields": { "description": "Builder" }
            }));
        then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
    });
    save_project_voice_guide_and_fields_with_client(
        "proj-abc",
        "Direct.",
        Some(&fields),
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await
    .expect("should succeed");
    mock.assert();
}

#[tokio::test]
async fn test_save_voice_guide_and_fields_without_fields_succeeds() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
        then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
    });
    save_project_voice_guide_and_fields_with_client(
        "proj-abc",
        "Direct.",
        None,
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await
    .expect("None fields must be accepted");
}

#[tokio::test]
async fn test_save_voice_guide_and_fields_returns_session_expired_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
        then.status(401);
    });
    let result = save_project_voice_guide_and_fields_with_client(
        "proj-abc",
        "Direct.",
        None,
        &build_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
}
