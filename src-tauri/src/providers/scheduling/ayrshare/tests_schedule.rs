// SPDX-License-Identifier: BUSL-1.1
use super::*;

#[tokio::test]
async fn test_schedule_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/post")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "id": "ayrshare-post-456"
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let scheduled_time = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let result = provider.schedule_post(
        "Test post content",
        "twitter",
        Some(scheduled_time),
        None,
        Some("profile-123"),
    ).await;

    assert!(result.is_ok(), "schedule_post failed: {:?}", result);
    assert_eq!(result.unwrap().scheduler_id, "ayrshare-post-456");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_with_image() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/api/post")
            .json_body(serde_json::json!({
                "post": "Post with image",
                "platforms": ["twitter"],
                "profileKey": "profile-123",
                "mediaUrls": ["https://example.com/image.jpg"]
            }));
        then.status(200)
            .json_body(serde_json::json!({
                "id": "post-with-img-789"
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Post with image",
        "twitter",
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
        when.method(POST).path("/api/post");
        then.status(400)
            .json_body(serde_json::json!({
                "error": "Invalid platform"
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
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
        when.method(POST).path("/api/post");
        then.status(200)
            .json_body(serde_json::json!({
                "id": "retry-test-123"
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    // Test that schedule_post works (retry wrapper is tested separately in mod.rs)
    let result = provider.schedule_post(
        "Retry test",
        "twitter",
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
        when.method(DELETE)
            .path("/api/post/post-123")
            .header("Authorization", "Bearer test-key");
        then.status(200);
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.cancel_post("post-123", "twitter").await;
    assert!(result.is_ok());
    mock.assert();
}

#[tokio::test]
async fn test_get_queue_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/api/history")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "posts": [
                    {
                        "id": "queued-1",
                        "platform": "twitter",
                        "scheduleDate": "2024-01-20T15:00:00Z",
                        "post": "Short post"
                    },
                    {
                        "id": "queued-2",
                        "platform": "bluesky",
                        "scheduleDate": "2024-01-21T10:00:00Z",
                        "post": "This is a very long post that exceeds eighty characters and should be truncated with ellipsis"
                    }
                ]
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
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
            .path("/api/analytics/post/post-456")
            .query_param("platforms", "twitter")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "likes": 42,
                "retweets": 12,
                "comments": 5,
                "impressions": 1500
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_engagement("post-456", "twitter").await;
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
            .path("/api/analytics/post/post-789");
        then.status(200)
            .json_body(serde_json::json!({
                "likes": 10,
                "retweets": 2,
                "comments": 1
            }));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_engagement("post-789", "twitter").await;
    assert!(result.is_ok());

    let engagement = result.unwrap();
    assert_eq!(engagement.impressions, None);
}

#[test]
fn test_post_url_returns_correct_format_for_twitter() {
    // Test: post_url for Twitter returns correct URL format
    let provider = AyrshareProvider::new("test-key".to_string());

    let url = provider.post_url("twitter", "1234567890");
    assert_eq!(url, Some("https://x.com/i/web/status/1234567890".to_string()));

    // Also test "x" as an alias
    let url = provider.post_url("x", "9876543210");
    assert_eq!(url, Some("https://x.com/i/web/status/9876543210".to_string()));
}

#[test]
fn test_post_url_returns_none_for_bluesky() {
    // Test: Bluesky URLs require handle, return None for now
    let provider = AyrshareProvider::new("test-key".to_string());

    let url = provider.post_url("bluesky", "test-post-id");
    assert_eq!(url, None);
}

#[test]
fn test_post_url_returns_none_for_mastodon() {
    // Test: Mastodon URLs are instance-specific, return None
    let provider = AyrshareProvider::new("test-key".to_string());

    let url = provider.post_url("mastodon", "test-post-id");
    assert_eq!(url, None);
}

#[test]
fn test_post_url_returns_none_for_unsupported_platform() {
    // Test: Unsupported platforms return None instead of panicking
    let provider = AyrshareProvider::new("test-key".to_string());

    let url = provider.post_url("unknown-platform", "test-id");
    assert_eq!(url, None);
}
