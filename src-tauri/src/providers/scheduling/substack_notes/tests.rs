// SPDX-License-Identifier: BUSL-1.1
use super::*;
use httpmock::prelude::*;

fn make_provider(server: &MockServer) -> SubstackNotesProvider {
    let mut p = SubstackNotesProvider::new("sess-cookie-abc".to_string());
    p.base_url = server.base_url();
    p
}

fn profile_mock(server: &MockServer) {
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(200).json_body(serde_json::json!({
            "id": "user-1", "handle": "myhandle", "name": "My Publication"
        }));
    });
}

#[tokio::test]
async fn test_schedule_post_success() {
    let server = MockServer::start();
    profile_mock(&server);
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/comment/feed");
        then.status(200).json_body(serde_json::json!({ "id": "note_abc123" }));
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello Substack", "substack", None, None, None).await;
    assert!(result.is_ok(), "{:?}", result);
    let res = result.unwrap();
    assert_eq!(res.scheduler_id, "note_abc123");
    assert_eq!(res.platform_url, Some("https://substack.com/@myhandle/note/note_abc123".to_string()));
}

#[tokio::test]
async fn test_schedule_post_unauthorised() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(401);
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "substack", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
    let server = MockServer::start();
    profile_mock(&server);
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/comment/feed");
        then.status(429).header("Retry-After", "30");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "substack", None, None, None).await;
    match result {
        Err(ProviderError::RateLimit(d)) => assert_eq!(d.as_secs(), 30),
        other => panic!("expected RateLimit, got {:?}", other),
    }
}

#[tokio::test]
async fn test_cancel_post_not_supported() {
    let provider = SubstackNotesProvider::new("cookie".to_string());
    let result = provider.cancel_post("note-1", "substack").await;
    match result {
        Err(ProviderError::NotSupported(msg)) => {
            assert!(msg.contains("substack.com"), "message: {}", msg);
        }
        other => panic!("expected NotSupported, got {:?}", other),
    }
}

#[tokio::test]
async fn test_get_engagement_partial() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/comment/note-1");
        then.status(200).json_body(serde_json::json!({
            "reactions_count": 7, "children_count": null
        }));
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("note-1", "substack").await;
    assert!(result.is_ok(), "{:?}", result);
    let eng = result.unwrap();
    assert_eq!(eng.likes, 7);
    assert_eq!(eng.replies, 0);
    assert_eq!(eng.impressions, None);
}

#[tokio::test]
async fn test_get_queue_returns_empty() {
    let provider = SubstackNotesProvider::new("cookie".to_string());
    let result = provider.get_queue().await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_post_url_uses_cached_username() {
    let provider = SubstackNotesProvider::new("cookie".to_string());
    *provider.username.lock().unwrap() = Some("myhandle".to_string());
    assert_eq!(
        provider.post_url("substack", "note-123"),
        Some("https://substack.com/@myhandle/note/note-123".to_string())
    );
}

#[test]
fn test_post_url_returns_none_when_username_unknown() {
    let provider = SubstackNotesProvider::new("cookie".to_string());
    assert_eq!(provider.post_url("substack", "note-123"), None);
}

#[test]
fn test_cookie_header_strips_control_characters() {
    let provider = SubstackNotesProvider::new("valid\r\nX-Injected: evil".to_string());
    let header = provider.cookie_header();
    assert!(!header.contains('\r'), "CR must be stripped");
    assert!(!header.contains('\n'), "LF must be stripped");
    assert_eq!(header, "connect.sid=validX-Injected: evil");
}

#[tokio::test]
async fn test_list_profiles_unauthorised_returns_auth_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(401);
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_403_returns_auth_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(403);
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_non_2xx_non_auth_returns_http_error() {
    let server = MockServer::start();
    profile_mock(&server);
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/comment/feed");
        then.status(500).body("internal error");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "substack", None, None, None).await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_missing_id_in_response_returns_unknown_error() {
    let server = MockServer::start();
    profile_mock(&server);
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/comment/feed");
        then.status(200).json_body(serde_json::json!({ "type": "publication" }));
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "substack", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_unauthorised_returns_auth_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/comment/note-x");
        then.status(401);
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("note-x", "substack").await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/comment/note-y");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("note-y", "substack").await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_list_profiles_caches_username_on_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(200).json_body(serde_json::json!({
            "id": "user-1", "handle": "cachedhandle", "name": "Cached Pub"
        }));
    });
    let provider = make_provider(&server);
    provider.list_profiles().await.expect("list_profiles ok");
    assert_eq!(
        provider.cached_username(),
        Some("cachedhandle".to_string()),
        "username must be cached after list_profiles"
    );
}

#[tokio::test]
async fn test_ensure_username_cached_uses_cached_value() {
    let server = MockServer::start();
    let profile_mock_handle = server.mock(|when, then| {
        when.method(GET).path("/api/v1/profile");
        then.status(200).json_body(serde_json::json!({
            "id": "u1", "handle": "myhandle", "name": "My Pub"
        }));
    });
    let provider = make_provider(&server);
    provider.ensure_username_cached().await.expect("first call ok");
    provider.ensure_username_cached().await.expect("second call ok — must not re-fetch");
    profile_mock_handle.assert_hits(1);
}
