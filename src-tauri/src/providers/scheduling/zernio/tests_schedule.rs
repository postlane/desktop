// SPDX-License-Identifier: BUSL-1.1
use super::*;

async fn test_schedule_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/posts")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "post": { "_id": "zernio-post-456" }
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let scheduled_time = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let result = provider.schedule_post(
        "Test post content",
        "x",
        Some(scheduled_time),
        None,
        Some("profile-123"),
    ).await;

    assert!(result.is_ok(), "schedule_post failed: {:?}", result);
    assert_eq!(result.unwrap().scheduler_id, "zernio-post-456");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_with_image() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/posts");
        then.status(200)
            .json_body(serde_json::json!({
                "post": { "_id": "post-with-img-789" }
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Post with image",
        "x",
        None,
        Some("https://example.com/image.jpg"),
        Some("profile-123"),
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().scheduler_id, "post-with-img-789");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_http_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts");
        then.status(400)
            .json_body(serde_json::json!({
                "error": "Invalid platform"
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Test",
        "invalid-platform",
        None,
        None,
        Some("profile-123"),
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::HttpError { status, .. } => {
            assert_eq!(status, 400);
        }
        other => panic!("Expected HttpError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_wrapped_in_retry() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    // Create mock that will be called multiple times
    let mock = server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts");
        then.status(200)
            .json_body(serde_json::json!({
                "post": { "_id": "retry-test-123" }
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    // Test that schedule_post works (retry wrapper is tested separately in mod.rs)
    let result = provider.schedule_post(
        "Retry test",
        "x",
        None,
        None,
        Some("profile-123"),
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().scheduler_id, "retry-test-123");
    mock.assert();
}

#[tokio::test]
async fn test_cancel_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/posts/post-123/cancel")
            .header("Authorization", "Bearer test-key");
        then.status(200);
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.cancel_post("post-123", "x").await;
    assert!(result.is_ok());
    mock.assert();
}

#[tokio::test]
async fn test_get_queue_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/queue")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "posts": [
                    {
                        "post_id": "queued-1",
                        "platform": "x",
                        "scheduled_for": "2024-01-20T15:00:00Z",
                        "content": "Short post"
                    },
                    {
                        "post_id": "queued-2",
                        "platform": "bluesky",
                        "scheduled_for": "2024-01-21T10:00:00Z",
                        "content": "This is a very long post that exceeds eighty characters and should be truncated with ellipsis"
                    }
                ]
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_queue().await;
    assert!(result.is_ok());

    let queue = result.unwrap();
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[0].post_id, "queued-1");
    assert_eq!(queue[0].content_preview, "Short post");
    assert_eq!(queue[1].content_preview.len(), 83); // 80 chars + "..."
}

#[tokio::test]
async fn test_get_engagement_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/posts/post-456/engagement")
            .query_param("platform", "x")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "likes": 42,
                "reposts": 12,
                "replies": 5,
                "impressions": 1500
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_engagement("post-456", "x").await;
    assert!(result.is_ok());

    let engagement = result.unwrap();
    assert_eq!(engagement.likes, 42);
    assert_eq!(engagement.reposts, 12);
    assert_eq!(engagement.replies, 5);
    assert_eq!(engagement.impressions, Some(1500));
}

#[tokio::test]
async fn test_get_engagement_without_impressions() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/posts/post-789/engagement");
        then.status(200)
            .json_body(serde_json::json!({
                "likes": 10,
                "reposts": 2,
                "replies": 1
            }));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_engagement("post-789", "x").await;
    assert!(result.is_ok());

    let engagement = result.unwrap();
    assert_eq!(engagement.impressions, None);
}

#[tokio::test]
async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    // Mock 429 response with Retry-After header
    server.mock(|when, then| {
        when.method(POST).path("/api/v1/posts");
        then.status(429)
            .header("Retry-After", "60")  // 60 seconds
            .body("Rate limit exceeded");
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Test",
        "x",
        None,
        None,
        Some("profile-123"),
    ).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::RateLimit(duration) => {
            assert_eq!(duration.as_secs(), 60);
        }
        other => panic!("Expected RateLimit error, got {:?}", other),
    }
}
