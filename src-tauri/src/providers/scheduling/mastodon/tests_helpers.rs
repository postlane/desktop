// SPDX-License-Identifier: BUSL-1.1
use super::*;

#[tokio::test]
async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(429)
            .header("Retry-After", "60")
            .body("Rate limit exceeded");
    });

    let provider = make_provider(&server);
    let result = provider.schedule_post("Test", "mastodon", None, None, None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::RateLimit(duration) => {
            assert_eq!(duration.as_secs(), 60);
        }
        other => panic!("Expected RateLimit, got {:?}", other),
    }
    mock.assert();
}

// 9.5.12 — fetch_instance_char_limit reads max_characters; defaults to 500 on failure
#[tokio::test]
async fn test_instance_character_limit_fetch() {
    // Success case: returns configured limit
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/instance");
        then.status(200).json_body(serde_json::json!({
            "configuration": {
                "statuses": { "max_characters": 2000 }
            }
        }));
    });
    let provider = make_provider(&server);
    let limit = provider.fetch_instance_char_limit().await;
    assert_eq!(limit, 2000, "Should return instance-configured limit");

    // Failure case: defaults to 500
    let server2 = MockServer::start();
    server2.mock(|when, then| {
        when.method(GET).path("/instance");
        then.status(500);
    });
    let provider2 = make_provider(&server2);
    let limit2 = provider2.fetch_instance_char_limit().await;
    assert_eq!(limit2, 500, "Should default to 500 on fetch failure");
}

// Issue 4 — Retry-After header capped at 3600 seconds
#[tokio::test]
async fn test_retry_after_bounded_to_max() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(429).header("Retry-After", "86400").body("Rate limit exceeded");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Test", "mastodon", None, None, None).await;
    match result.unwrap_err() {
        ProviderError::RateLimit(d) => assert_eq!(d.as_secs(), 3600, "Retry-After must be capped at 3600s"),
        other => panic!("Expected RateLimit, got {:?}", other),
    }
}

// Issue 4 — missing Retry-After defaults to 60 seconds
#[tokio::test]
async fn test_retry_after_missing_defaults_to_60() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(429).body("Rate limit exceeded");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Test", "mastodon", None, None, None).await;
    match result.unwrap_err() {
        ProviderError::RateLimit(d) => assert_eq!(d.as_secs(), 60, "Missing Retry-After should default to 60s"),
        other => panic!("Expected RateLimit, got {:?}", other),
    }
}

// Issue 6 — get_engagement returns platform_url from published post
#[tokio::test]
async fn test_get_engagement_returns_platform_url() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/statuses/103704874086360371");
        then.status(200).json_body(serde_json::json!({
            "id": "103704874086360371",
            "url": "https://mastodon.social/@alice/103704874086360371",
            "favourites_count": 5,
            "reblogs_count": 2,
            "replies_count": 1
        }));
    });
    let provider = make_provider(&server);
    let engagement = provider.get_engagement("103704874086360371", "mastodon").await.unwrap();
    assert_eq!(
        engagement.platform_url,
        Some("https://mastodon.social/@alice/103704874086360371".to_string()),
        "get_engagement must return the post URL so scheduled posts can recover it after publish"
    );
}

// Issue 7 — parse_link_next extracts the next URL from a Link header
#[test]
fn test_parse_link_next_extracts_url() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "link",
        "<https://mastodon.social/api/v1/scheduled_statuses?max_id=3221>; rel=\"next\""
            .parse()
            .unwrap(),
    );
    let result = parse_link_next(&headers);
    assert_eq!(
        result,
        Some("https://mastodon.social/api/v1/scheduled_statuses?max_id=3221".to_string())
    );
}

#[test]
fn test_parse_link_next_returns_none_when_no_next() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "link",
        "<https://mastodon.social/api/v1/scheduled_statuses?min_id=1>; rel=\"prev\""
            .parse()
            .unwrap(),
    );
    assert_eq!(parse_link_next(&headers), None);
}

#[test]
fn test_parse_link_next_returns_none_when_header_absent() {
    let headers = reqwest::header::HeaderMap::new();
    assert_eq!(parse_link_next(&headers), None);
}

// Issue 7 — get_queue follows pagination: second page uses a distinct path so mocks don't overlap
#[tokio::test]
async fn test_get_queue_fetches_all_pages() {
    let server = MockServer::start();
    let page2_path = "/scheduled_statuses/page2";

    server.mock(|when, then| {
        when.method(GET).path("/scheduled_statuses");
        then.status(200)
            .header("Link", &format!("<{}{}>; rel=\"next\"", server.base_url(), page2_path))
            .json_body(serde_json::json!([{
                "id": "3221",
                "scheduled_at": "2024-06-01T12:00:00.000Z",
                "params": { "text": "Page 1 post" }
            }]));
    });

    server.mock(|when, then| {
        when.method(GET).path(page2_path);
        then.status(200)
            .json_body(serde_json::json!([{
                "id": "3222",
                "scheduled_at": "2024-06-02T12:00:00.000Z",
                "params": { "text": "Page 2 post" }
            }]));
    });

    let provider = make_provider(&server);
    let queue = provider.get_queue().await.unwrap();
    assert_eq!(queue.len(), 2, "get_queue must follow pagination and return all posts");
    assert_eq!(queue[0].post_id, "3221");
    assert_eq!(queue[1].post_id, "3222");
}

// Issue 3 — create() factory validates SSRF before constructing provider
#[tokio::test]
async fn test_create_rejects_private_ip_instance() {
    let result = MastodonProvider::create("192.168.1.1", "token".to_string()).await;
    assert!(result.is_err(), "create() must reject private IP instances");
    match result.unwrap_err() {
        ProviderError::InvalidInstance(_) => {}
        other => panic!("Expected InvalidInstance, got {:?}", other),
    }
}

// 9.3.4 — SSRF: private IP instances are rejected before any HTTP request
#[tokio::test]
async fn test_rejects_private_ip_mastodon_instance() {
    let result = validate_instance_domain("192.168.1.1").await;
    assert!(result.is_err(), "Private IP should be rejected");
    match result.unwrap_err() {
        ProviderError::InvalidInstance(_) => {}
        other => panic!("Expected InvalidInstance, got {:?}", other),
    }
}

// Loopback should also be rejected
#[tokio::test]
async fn test_rejects_loopback_mastodon_instance() {
    let result = validate_instance_domain("127.0.0.1").await;
    assert!(result.is_err(), "Loopback should be rejected");
    match result.unwrap_err() {
        ProviderError::InvalidInstance(_) => {}
        other => panic!("Expected InvalidInstance, got {:?}", other),
    }
}
