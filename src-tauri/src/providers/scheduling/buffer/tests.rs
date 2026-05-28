// SPDX-License-Identifier: BUSL-1.1
use super::*;


#[test]
fn test_buffer_provider_stores_client() {
    // Test: BufferProvider should store a reqwest::Client at instantiation
    let provider = BufferProvider::new("test-api-key".to_string());

    // Verify the provider was created and has a name
    assert_eq!(provider.name(), "buffer");
}

#[test]
fn test_buffer_provider_instantiation() {
    // Test: Creating BufferProvider should not panic
    let _provider = BufferProvider::new("test-key-123".to_string());
    // If we get here without panic, the test passes
}

#[test]
fn test_post_url_for_x() {
    let provider = BufferProvider::new("test-key".to_string());
    let url = provider.post_url("x", "1234567890");
    assert_eq!(url, Some("https://x.com/i/web/status/1234567890".to_string()));
}

#[test]
fn test_post_url_for_facebook() {
    let provider = BufferProvider::new("test-key".to_string());
    let url = provider.post_url("facebook", "123456");
    assert_eq!(url, Some("https://facebook.com/123456".to_string()));
}

#[test]
fn test_post_url_for_linkedin() {
    let provider = BufferProvider::new("test-key".to_string());
    let url = provider.post_url("linkedin", "activity-123");
    assert_eq!(url, Some("https://linkedin.com/feed/update/activity-123".to_string()));
}

#[test]
fn test_post_url_for_instagram() {
    let provider = BufferProvider::new("test-key".to_string());
    let url = provider.post_url("instagram", "ABC123");
    assert_eq!(url, Some("https://instagram.com/p/ABC123".to_string()));
}

#[test]
fn test_post_url_returns_none_for_unsupported() {
    let provider = BufferProvider::new("test-key".to_string());
    assert_eq!(provider.post_url("bluesky", "test"), None);
    assert_eq!(provider.post_url("mastodon", "test"), None);
    assert_eq!(provider.post_url("unknown", "test"), None);
}

#[tokio::test]
async fn test_list_profiles_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/1/profiles.json")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!([
                {
                    "id": "channel-1",
                    "service": "twitter",
                    "formatted_username": "@myaccount"
                },
                {
                    "id": "channel-2",
                    "service": "facebook",
                    "service_username": "My Page"
                }
            ]));
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.list_profiles().await;
    assert!(result.is_ok());

    let profiles = result.unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "channel-1");
    assert_eq!(profiles[0].name, "@myaccount");
    assert_eq!(profiles[0].platforms, vec!["twitter"]);
    assert_eq!(profiles[1].id, "channel-2");
    assert_eq!(profiles[1].name, "My Page");
}

#[tokio::test]
async fn test_list_profiles_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/1/profiles.json");
        then.status(401);
    });

    let mut provider = BufferProvider::new("invalid-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.list_profiles().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::AuthError(_) => {},
        other => panic!("Expected AuthError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_test_connection_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/1/profiles.json");
        then.status(200).json_body(serde_json::json!([]));
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.test_connection().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_schedule_post_uses_authorization_header_not_query_param() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    // This test verifies that API key is sent in Authorization header,
    // NOT as a query parameter (which would be insecure)
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/1/updates/create.json")
            .header("Authorization", "Bearer test-key")
            .matches(|req| {
                // Verify access_token is NOT in query params
                req.query_params.is_none() ||
                !req.query_params.as_ref().unwrap().iter().any(|(k, _)| k == "access_token")
            });
        then.status(200)
            .json_body(serde_json::json!({
                "updates": [
                    {
                        "id": "buffer-post-123"
                    }
                ]
            }));
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Test post",
        "twitter",
        None,
        None,
        Some("channel-1"),
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().scheduler_id, "buffer-post-123");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST)
            .path("/1/updates/create.json")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "updates": [
                    {
                        "id": "buffer-post-123"
                    }
                ]
            }));
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.schedule_post(
        "Test post",
        "twitter",
        None,
        None,
        Some("channel-1"),
    ).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().scheduler_id, "buffer-post-123");
}

#[tokio::test]
async fn test_cancel_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(POST)
            .path("/1/updates/post-456/destroy.json")
            .header("Authorization", "Bearer test-key");
        then.status(200);
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.cancel_post("post-456", "twitter").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_engagement_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET)
            .path("/1/updates/post-789.json")
            .header("Authorization", "Bearer test-key");
        then.status(200)
            .json_body(serde_json::json!({
                "statistics": {
                    "likes": 25,
                    "shares": 8,
                    "comments": 3,
                    "reach": 500
                }
            }));
    });

    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.get_engagement("post-789", "twitter").await;
    assert!(result.is_ok());

    let engagement = result.unwrap();
    assert_eq!(engagement.likes, 25);
    assert_eq!(engagement.reposts, 8);
    assert_eq!(engagement.replies, 3);
    assert_eq!(engagement.impressions, Some(500));
}

// buffer — non-2xx responses return HttpError (lines 99-109, 143-151, 189-197, 285-294)

#[tokio::test]
async fn test_schedule_post_non_2xx_returns_http_error() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/1/updates/create.json");
        then.status(500).body("internal server error");
    });
    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();
    let result = provider.schedule_post("Hello", "twitter", None, None, Some("ch-1")).await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_list_profiles_non_2xx_returns_http_error() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/1/profiles.json");
        then.status(500).body("server error");
    });
    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();
    let result = provider.list_profiles().await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_cancel_post_non_2xx_returns_http_error() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/1/updates/post-del/destroy.json");
        then.status(500).body("server error");
    });
    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();
    let result = provider.cancel_post("post-del", "twitter").await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

#[tokio::test]
async fn test_get_engagement_non_2xx_returns_http_error() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/1/updates/post-eng.json");
        then.status(500).body("server error");
    });
    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();
    let result = provider.get_engagement("post-eng", "twitter").await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 500, .. })), "{:?}", result);
}

// buffer get_queue — content longer than 80 chars is truncated (lines 243-245)
#[tokio::test]
async fn test_get_queue_truncates_long_content_preview() {
    use httpmock::prelude::*;
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/1/profiles.json");
        then.status(200).json_body(serde_json::json!([{"id": "ch-1", "service": "twitter"}]));
    });
    let long_text = "x".repeat(100);
    server.mock(|when, then| {
        when.method(GET).path("/1/profiles/ch-1/updates/pending.json");
        then.status(200).json_body(serde_json::json!({
            "updates": [{
                "id": "u1",
                "scheduled_at": 1_717_200_000_i64,
                "text": long_text
            }]
        }));
    });
    let mut provider = BufferProvider::new("test-key".to_string());
    provider.base_url = server.base_url();
    let result = provider.get_queue().await;
    assert!(result.is_ok(), "{:?}", result);
    let queue = result.unwrap();
    assert_eq!(queue.len(), 1);
    assert!(queue[0].content_preview.ends_with("..."), "long content must be truncated");
    assert!(queue[0].content_preview.chars().count() <= 83, "preview at most 83 chars");
}
