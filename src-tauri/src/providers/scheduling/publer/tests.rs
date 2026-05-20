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

// ── build_schedule_body ──────────────────────────────────────────────────────

#[test]
fn test_build_schedule_body_includes_platform_and_content() {
    let body = build_schedule_body("hello", "x", None, None);
    assert_eq!(
        body["bulk"]["posts"][0]["networks"]["x"]["text"],
        serde_json::json!("hello")
    );
}

#[test]
fn test_build_schedule_body_includes_profile_id_when_given() {
    let body = build_schedule_body("hi", "x", None, Some("prof-1"));
    assert_eq!(
        body["bulk"]["posts"][0]["accounts"][0]["id"],
        serde_json::json!("prof-1")
    );
}

#[test]
fn test_build_schedule_body_includes_scheduled_at_when_given() {
    use chrono::TimeZone;
    let dt = chrono::Utc.with_ymd_and_hms(2025, 6, 1, 12, 0, 0).unwrap();
    let body = build_schedule_body("hi", "x", Some(dt), Some("prof-1"));
    let scheduled_at = &body["bulk"]["posts"][0]["accounts"][0]["scheduled_at"];
    assert!(scheduled_at.is_string(), "scheduled_at should be a string, got {scheduled_at}");
    assert!(
        scheduled_at.as_str().unwrap().starts_with("2025-06-01"),
        "unexpected value: {scheduled_at}"
    );
}

#[test]
fn test_build_schedule_body_no_scheduled_at_when_no_profile() {
    let body = build_schedule_body("hi", "x", None, None);
    assert_eq!(
        body["bulk"]["posts"][0]["accounts"][0],
        serde_json::json!({})
    );
}

// ── parse_queued_post ────────────────────────────────────────────────────────

#[test]
fn test_parse_queued_post_returns_some_for_valid_entry() {
    let p = serde_json::json!({
        "id": "post-42",
        "account_id": "twitter",
        "scheduled_at": "2025-07-01T09:00:00Z",
        "text": "Short post"
    });
    let result = parse_queued_post(&p);
    assert!(result.is_some());
    let post = result.unwrap();
    assert_eq!(post.post_id, "post-42");
    assert_eq!(post.platform, "twitter");
    assert_eq!(post.content_preview, "Short post");
}

#[test]
fn test_parse_queued_post_returns_none_when_id_missing() {
    let p = serde_json::json!({
        "account_id": "twitter",
        "scheduled_at": "2025-07-01T09:00:00Z",
        "text": "No id"
    });
    assert!(parse_queued_post(&p).is_none());
}

#[test]
fn test_parse_queued_post_returns_none_when_scheduled_at_missing() {
    let p = serde_json::json!({
        "id": "post-1",
        "account_id": "twitter",
        "text": "No date"
    });
    assert!(parse_queued_post(&p).is_none());
}

#[test]
fn test_parse_queued_post_returns_none_when_scheduled_at_invalid() {
    let p = serde_json::json!({
        "id": "post-1",
        "account_id": "twitter",
        "scheduled_at": "not-a-date",
        "text": "Bad date"
    });
    assert!(parse_queued_post(&p).is_none());
}

#[test]
fn test_parse_queued_post_truncates_long_content() {
    let long_text = "a".repeat(100);
    let p = serde_json::json!({
        "id": "post-1",
        "account_id": "twitter",
        "scheduled_at": "2025-07-01T09:00:00Z",
        "text": long_text
    });
    let post = parse_queued_post(&p).unwrap();
    assert!(
        post.content_preview.ends_with("..."),
        "expected truncation, got: {}",
        post.content_preview
    );
}

#[test]
fn test_parse_queued_post_does_not_truncate_short_content() {
    let short_text = "a".repeat(40);
    let p = serde_json::json!({
        "id": "post-1",
        "account_id": "twitter",
        "scheduled_at": "2025-07-01T09:00:00Z",
        "text": short_text
    });
    let post = parse_queued_post(&p).unwrap();
    assert!(
        !post.content_preview.ends_with("..."),
        "expected no truncation, got: {}",
        post.content_preview
    );
    assert_eq!(post.content_preview.len(), 40);
}

// ── fetch_workspace_id ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_fetch_workspace_id_returns_err_on_401() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/workspaces");
        then.status(401);
    });
    let client = build_client();
    let base_url = format!("{}/api/v1", server.base_url());
    let result = fetch_workspace_id(&client, "bad-key", &base_url).await;
    assert!(
        matches!(result, Err(ProviderError::AuthError(_))),
        "expected AuthError, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_fetch_workspace_id_returns_err_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/workspaces");
        then.status(403);
    });
    let client = build_client();
    let base_url = format!("{}/api/v1", server.base_url());
    let result = fetch_workspace_id(&client, "bad-key", &base_url).await;
    assert!(
        matches!(result, Err(ProviderError::AuthError(_))),
        "expected AuthError, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_fetch_workspace_id_returns_err_when_no_workspaces() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/workspaces");
        then.status(200).json_body(serde_json::json!([]));
    });
    let client = build_client();
    let base_url = format!("{}/api/v1", server.base_url());
    let result = fetch_workspace_id(&client, "key", &base_url).await;
    assert!(
        matches!(result, Err(ProviderError::Unknown(_))),
        "expected Unknown error for empty workspaces, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_fetch_workspace_id_returns_first_workspace_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/api/v1/workspaces");
        then.status(200).json_body(serde_json::json!([
            { "id": "ws-1" },
            { "id": "ws-2" }
        ]));
    });
    let client = build_client();
    let base_url = format!("{}/api/v1", server.base_url());
    let result = fetch_workspace_id(&client, "key", &base_url).await;
    assert_eq!(result.unwrap(), "ws-1");
}
