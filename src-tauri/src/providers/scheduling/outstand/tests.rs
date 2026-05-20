// SPDX-License-Identifier: BUSL-1.1
use super::*;
use httpmock::prelude::*;

fn make_provider(server: &MockServer) -> OutstandProvider {
    let mut p = OutstandProvider::new("test-key".to_string());
    p.base_url = server.base_url();
    p
}

#[tokio::test]
async fn test_schedule_post_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(200).json_body(serde_json::json!({
            "data": { "id": "out_abc123", "postUrl": "https://linkedin.com/posts/out_abc123" }
        }));
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "linkedin", None, None, Some("acc-1")).await;
    assert!(result.is_ok(), "{:?}", result);
    let res = result.unwrap();
    assert_eq!(res.scheduler_id, "out_abc123");
    assert_eq!(res.platform_url, Some("https://linkedin.com/posts/out_abc123".to_string()));
}

#[tokio::test]
async fn test_schedule_post_unauthorised() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(403);
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(429).header("Retry-After", "60");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    match result {
        Err(ProviderError::RateLimit(d)) => assert_eq!(d.as_secs(), 60),
        other => panic!("expected RateLimit, got {:?}", other),
    }
}

#[tokio::test]
async fn test_cancel_post_not_supported() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/posts/post-1");
        then.status(405);
    });
    let provider = make_provider(&server);
    let result = provider.cancel_post("post-1", "linkedin").await;
    assert!(matches!(result, Err(ProviderError::NotSupported(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_partial() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts/post-1/analytics");
        then.status(200).json_body(serde_json::json!({
            "data": { "likes": 10, "shares": null, "comments": null, "reach": null }
        }));
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("post-1", "linkedin").await;
    assert!(result.is_ok(), "{:?}", result);
    let eng = result.unwrap();
    assert_eq!(eng.likes, 10);
    assert_eq!(eng.reposts, 0);
    assert_eq!(eng.impressions, None);
}

#[tokio::test]
async fn test_list_profiles_returns_profiles() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/social-accounts");
        then.status(200).json_body(serde_json::json!({
            "data": [{ "id": "p1", "name": "My X", "platform": "twitter" }]
        }));
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(result.is_ok(), "{:?}", result);
    let profiles = result.unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "p1");
    assert_eq!(profiles[0].name, "My X");
    assert_eq!(profiles[0].platforms, vec!["twitter"]);
}

#[tokio::test]
async fn test_list_profiles_returns_err_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/social-accounts");
        then.status(401);
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_queue_returns_empty_on_empty_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts").query_param("status", "scheduled");
        then.status(200).json_body(serde_json::json!({ "data": [] }));
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    assert!(result.is_ok(), "{:?}", result);
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_get_queue_returns_err_on_network_failure() {
    // Port 1 is reserved and never listening, so this will fail at the TCP level.
    let mut provider = OutstandProvider::new("test-key".to_string());
    provider.base_url = "http://127.0.0.1:1".to_string();
    let result = provider.get_queue().await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

#[test]
fn test_name_returns_outstand() {
    let provider = OutstandProvider::new("key".to_string());
    assert_eq!(provider.name(), "outstand");
}

#[tokio::test]
async fn test_schedule_post_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(500).body("internal error");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_list_profiles_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/social-accounts");
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
async fn test_cancel_post_non_2xx_non_405_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/posts/post-x");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.cancel_post("post-x", "linkedin").await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_cancel_post_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/posts/post-del");
        then.status(200);
    });
    let provider = make_provider(&server);
    let result = provider.cancel_post("post-del", "linkedin").await;
    assert!(result.is_ok(), "{:?}", result);
}

#[tokio::test]
async fn test_get_queue_returns_scheduled_posts() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts").query_param("status", "scheduled");
        then.status(200).json_body(serde_json::json!({
            "data": [{
                "id": "q1",
                "platform": "linkedin",
                "scheduledAt": "2026-06-01T10:00:00Z",
                "containers": [{ "content": "Hello from queue" }]
            }]
        }));
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    assert!(result.is_ok(), "{:?}", result);
    let posts = result.unwrap();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].post_id, "q1");
    assert_eq!(posts[0].platform, "linkedin");
    assert!(posts[0].content_preview.contains("Hello"));
}

#[tokio::test]
async fn test_get_queue_truncates_long_content_preview() {
    let server = MockServer::start();
    let long_content = "x".repeat(100);
    server.mock(|when, then| {
        when.method(GET).path("/posts").query_param("status", "scheduled");
        then.status(200).json_body(serde_json::json!({
            "data": [{
                "id": "q2",
                "platform": "linkedin",
                "scheduledAt": "2026-06-01T10:00:00Z",
                "containers": [{ "content": long_content }]
            }]
        }));
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    assert!(result.is_ok(), "{:?}", result);
    let posts = result.unwrap();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].content_preview.ends_with("..."), "long content must be truncated with ...");
    assert!(posts[0].content_preview.len() <= 83, "preview must not exceed 80 chars + ...");
}

#[tokio::test]
async fn test_get_queue_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts").query_param("status", "scheduled");
        then.status(500).body("error");
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_with_scheduled_at() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(200).json_body(serde_json::json!({
            "data": { "id": "out_sched", "postUrl": null }
        }));
    });
    let provider = make_provider(&server);
    let dt = chrono::DateTime::parse_from_rfc3339("2026-06-01T10:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let result = provider.schedule_post("Hello", "linkedin", Some(dt), None, None).await;
    assert!(result.is_ok(), "{:?}", result);
    let res = result.unwrap();
    assert_eq!(res.scheduler_id, "out_sched");
    assert_eq!(res.platform_url, None);
}

#[tokio::test]
async fn test_schedule_post_missing_id_returns_unknown_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/posts");
        then.status(200).json_body(serde_json::json!({ "data": {} }));
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

#[tokio::test]
async fn test_post_url_returns_none() {
    let provider = OutstandProvider::new("key".to_string());
    assert_eq!(provider.post_url("linkedin", "some-id"), None);
}

#[tokio::test]
async fn test_test_connection_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/usage");
        then.status(200).json_body(serde_json::json!({}));
    });
    let provider = make_provider(&server);
    let result = provider.test_connection().await;
    assert!(result.is_ok(), "{:?}", result);
}

#[tokio::test]
async fn test_test_connection_401_returns_auth_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/usage");
        then.status(401);
    });
    let provider = make_provider(&server);
    let result = provider.test_connection().await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts/p1/analytics");
        then.status(500).body("error");
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("p1", "linkedin").await;
    match result {
        Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
        other => panic!("expected HttpError(500), got {:?}", other),
    }
}

#[tokio::test]
async fn test_get_engagement_unauthorised() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/posts/p2/analytics");
        then.status(403);
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("p2", "linkedin").await;
    assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
}
