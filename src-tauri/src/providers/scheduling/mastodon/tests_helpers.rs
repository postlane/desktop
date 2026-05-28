// SPDX-License-Identifier: BUSL-1.1
use super::*;
use super::queue_parser::{map_scheduled_status, parse_link_next};

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

// mastodon/mod.rs lines 36-42 — new() constructor is only called by create(); test it directly
#[test]
fn test_new_sets_correct_api_base() {
    let provider = MastodonProvider::new("mastodon.social", "my-token".to_string());
    assert_eq!(provider.api_base, "https://mastodon.social/api/v1");
    assert_eq!(provider.access_token, "my-token");
}

// mastodon/mod.rs lines 340-342 — post_url always returns None
#[test]
fn test_post_url_always_returns_none() {
    let server = MockServer::start();
    let provider = make_provider(&server);
    assert_eq!(provider.post_url("mastodon", "12345"), None);
}

// mastodon line 91 — fetch_instance_char_limit network error → defaults to 500
#[tokio::test]
async fn test_fetch_instance_char_limit_returns_500_on_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let limit = provider.fetch_instance_char_limit().await;
    assert_eq!(limit, 500, "network error must fall back to 500");
}

// mastodon network error paths — lines 167, 198, 229, 272, 314
#[tokio::test]
async fn test_schedule_post_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let result = provider.schedule_post("Hello", "mastodon", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_cancel_post_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let result = provider.cancel_post("post-1", "mastodon").await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_queue_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let result = provider.get_queue().await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_network_error() {
    let provider = MastodonProvider {
        client: build_client(),
        api_base: "http://127.0.0.1:1".to_string(),
        access_token: "tok".into(),
    };
    let result = provider.get_engagement("post-1", "mastodon").await;
    assert!(matches!(result, Err(ProviderError::NetworkError(_))), "{:?}", result);
}

// mastodon JSON parse error paths — lines 180, 242, 287, 327
#[tokio::test]
async fn test_schedule_post_invalid_json_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(200).body("not json");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "mastodon", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_invalid_json_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(200).body("not json");
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

// mastodon line 248 — list_profiles when display_name is empty uses acct fallback
#[tokio::test]
async fn test_list_profiles_uses_acct_when_display_name_empty() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(200).json_body(serde_json::json!({
            "id": "42",
            "display_name": "",  // empty → falls back to acct
            "acct": "@alice@mastodon.social"
        }));
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(result.is_ok(), "{:?}", result);
    let profiles = result.unwrap();
    assert_eq!(profiles[0].name, "@alice@mastodon.social");
}

#[tokio::test]
async fn test_get_queue_invalid_json_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/scheduled_statuses");
        then.status(200).body("not json");
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_invalid_json_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/statuses/post-j");
        then.status(200).body("not json");
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("post-j", "mastodon").await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

// mastodon non-2xx HTTP error paths — lines 171-174, 207-211, 233-236, 276-279, 318-321

#[tokio::test]
async fn test_schedule_post_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello", "mastodon", None, None, None).await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_cancel_post_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/scheduled_statuses/post-del");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.cancel_post("post-del", "mastodon").await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_get_queue_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/scheduled_statuses");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.get_queue().await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/statuses/post-eng");
        then.status(500).body("server error");
    });
    let provider = make_provider(&server);
    let result = provider.get_engagement("post-eng", "mastodon").await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

// fetch_instance_char_limit — 200 with invalid JSON → defaults to 500
#[tokio::test]
async fn test_fetch_instance_char_limit_returns_500_on_invalid_json() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/instance");
        then.status(200).body("not json");
    });
    let provider = make_provider(&server);
    let limit = provider.fetch_instance_char_limit().await;
    assert_eq!(limit, 500, "invalid JSON body must fall back to 500");
}

// fetch_instance_char_limit — 200 with JSON but no max_characters → defaults to 500
#[tokio::test]
async fn test_fetch_instance_char_limit_returns_500_when_max_characters_absent() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/instance");
        then.status(200).json_body(serde_json::json!({ "version": "4.0.0" }));
    });
    let provider = make_provider(&server);
    let limit = provider.fetch_instance_char_limit().await;
    assert_eq!(limit, 500, "missing max_characters must fall back to 500");
}

// queue_parser — empty URL in rel="next" must not be returned
#[test]
fn test_parse_link_next_returns_none_when_url_is_empty() {
    // "<>" produces an empty URL after trimming — must not be treated as valid
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("link", "<>; rel=\"next\"".parse().unwrap());
    assert_eq!(parse_link_next(&headers), None);
}

// queue_parser — text longer than 80 chars is truncated with trailing "..."
#[test]
fn test_map_scheduled_status_truncates_long_text() {
    let long_text = "a".repeat(100);
    let item = serde_json::json!({
        "id": "123",
        "scheduled_at": "2024-06-01T12:00:00.000Z",
        "params": { "text": long_text }
    });
    let result = map_scheduled_status(&item).unwrap();
    assert!(result.content_preview.ends_with("..."), "long text must be truncated with '...'");
    assert_eq!(
        result.content_preview.chars().count(),
        83,
        "preview must be 80 chars + '...' = 83 chars total"
    );
}

// mastodon/mod.rs lines 137-139 — name() was never called by any test
#[test]
fn test_name_returns_mastodon() {
    let provider = MastodonProvider::new("mastodon.social", "token".to_string());
    assert_eq!(provider.name(), "mastodon");
}

// mastodon/mod.rs lines 130+132 — validate_instance_domain non-private IP path
#[tokio::test]
async fn test_validate_instance_domain_passes_for_public_ip() {
    // 203.0.113.1 is a TEST-NET-3 address (RFC 5737) - public, non-private.
    // lookup_host on an IP literal returns the address directly without DNS.
    let result = validate_instance_domain("203.0.113.1").await;
    assert!(result.is_ok(), "public IP must pass validation: {:?}", result);
}

// mastodon/mod.rs line 32 — create() success path with public IP
#[tokio::test]
async fn test_create_succeeds_for_public_ip() {
    let result = MastodonProvider::create("203.0.113.1", "tok".to_string()).await;
    assert!(result.is_ok(), "create() must succeed for public IP: {:?}", result);
    assert_eq!(result.unwrap().api_base, "https://203.0.113.1/api/v1");
}
