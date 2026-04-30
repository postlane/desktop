// SPDX-License-Identifier: BUSL-1.1
use super::*;
use httpmock::prelude::*;

fn make_provider(server: &MockServer) -> PublerProvider {
    let mut p = PublerProvider::new("test-api-key".to_string());
    p.base_url = format!("{}/api/v1", server.base_url());
    p
}

/// Pre-seed the workspace_id cell so tests don't need a /workspaces mock.
async fn with_workspace(provider: &PublerProvider) {
    provider.workspace_id.get_or_init(|| async { "ws-123".to_string() }).await;
}

#[tokio::test]
async fn test_schedule_post_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts/schedule");
        then.status(200).json_body(serde_json::json!({ "job_id": "j1" }));
    });
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/job_status/j1");
        then.status(200).json_body(serde_json::json!({
            "status": "completed",
            "post": { "id": "publer-post-abc" }
        }));
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.schedule_post("Hello", "linkedin", None, None, Some("acc-1")).await;
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(result.unwrap().scheduler_id, "publer-post-abc");
}

#[tokio::test]
async fn test_schedule_post_job_timeout() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts/schedule");
        then.status(200).json_body(serde_json::json!({ "job_id": "j-slow" }));
    });
    // All 5 polls return pending
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/job_status/j-slow");
        then.status(200).json_body(serde_json::json!({ "status": "pending" }));
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

#[tokio::test]
async fn test_schedule_post_unauthorised() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts/schedule");
        then.status(403);
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    match result {
        Err(ProviderError::AuthError(msg)) => {
            assert!(msg.contains("publer.com/plans"), "message: {}", msg);
        }
        other => panic!("expected AuthError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_429_returns_rate_limit_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts/schedule");
        then.status(429).header("X-RateLimit-Reset", "9999999999");
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::RateLimit(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_partial() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/posts/post-1");
        then.status(200).json_body(serde_json::json!({
            "analytics": { "likes": 5, "shares": null, "comments": null, "reach": null }
        }));
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.get_engagement("post-1", "linkedin").await;
    assert!(result.is_ok(), "{:?}", result);
    let eng = result.unwrap();
    assert_eq!(eng.likes, 5);
    assert_eq!(eng.reposts, 0);
    assert_eq!(eng.impressions, None);
}

#[tokio::test]
async fn test_schedule_post_429_rate_limit_capped_at_one_hour() {
    let server = MockServer::start();
    // X-RateLimit-Reset timestamp far in the future — must be capped at 3600s
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts/schedule");
        then.status(429).header("X-RateLimit-Reset", "9999999999");
    });
    let provider = make_provider(&server);
    with_workspace(&provider).await;
    let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
    match result {
        Err(ProviderError::RateLimit(d)) => {
            assert!(d.as_secs() <= 3600, "duration must be capped at 3600s, got {}s", d.as_secs());
        }
        other => panic!("expected RateLimit, got {:?}", other),
    }
}

#[test]
fn test_post_url_returns_none() {
    let provider = PublerProvider::new("key".to_string());
    assert_eq!(provider.post_url("linkedin", "post-1"), None);
}
