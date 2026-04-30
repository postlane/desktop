// SPDX-License-Identifier: BUSL-1.1
use super::*;


#[test]
fn test_zernio_provider_stores_client() {
    // Test: ZernioProvider should store a reqwest::Client at instantiation
    let provider = ZernioProvider::new("test-api-key".to_string());

    // Verify the provider was created and has a name
    assert_eq!(provider.name(), "zernio");
}

#[test]
fn test_zernio_provider_instantiation() {
    // Test: Creating ZernioProvider should not panic
    let _provider = ZernioProvider::new("sk_test_12345".to_string());
    // If we get here without panic, the test passes
}

#[test]
fn test_post_url_returns_correct_format_for_x() {
    // Test: post_url for X/Twitter returns correct URL format
    let provider = ZernioProvider::new("test-key".to_string());

    let url = provider.post_url("x", "1234567890");
    assert_eq!(url, Some("https://x.com/i/web/status/1234567890".to_string()));

    // Also test "twitter" as an alias
    let url = provider.post_url("twitter", "9876543210");
    assert_eq!(url, Some("https://x.com/i/web/status/9876543210".to_string()));
}

#[test]
fn test_post_url_returns_none_for_bluesky() {
    // Test: Bluesky URLs require handle, return None for now
    let provider = ZernioProvider::new("test-key".to_string());

    let url = provider.post_url("bluesky", "test-post-id");
    assert_eq!(url, None);
}

#[test]
fn test_post_url_returns_none_for_mastodon() {
    // Test: Mastodon URLs are instance-specific, return None
    let provider = ZernioProvider::new("test-key".to_string());

    let url = provider.post_url("mastodon", "test-post-id");
    assert_eq!(url, None);
}

#[test]
fn test_post_url_returns_none_for_unsupported_platform() {
    // Test: Unsupported platforms return None instead of panicking
    let provider = ZernioProvider::new("test-key".to_string());

    let url = provider.post_url("unknown-platform", "test-id");
    assert_eq!(url, None);
}

#[tokio::test]
async fn test_list_profiles_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    // Zernio uses /api/v1/accounts and returns an "accounts" array.
    // Each account is per-platform: _id, username, platform.
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/accounts")
            .header("Authorization", "Bearer test-api-key");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "accounts": [
                    {
                        "_id": "acc-twitter-1",
                        "username": "@myhandle",
                        "platform": "twitter",
                        "status": "connected"
                    },
                    {
                        "_id": "acc-bluesky-1",
                        "username": "myhandle.bsky.social",
                        "platform": "bluesky",
                        "status": "connected"
                    }
                ]
            }));
    });

    let mut provider = ZernioProvider::new("test-api-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.list_profiles().await;

    assert!(result.is_ok());
    let profiles = result.unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "acc-twitter-1");
    assert_eq!(profiles[0].name, "@myhandle");
    assert_eq!(profiles[0].platforms, vec!["x"]); // "twitter" normalised to "x"
    assert_eq!(profiles[1].id, "acc-bluesky-1");
    assert_eq!(profiles[1].platforms, vec!["bluesky"]);

    mock.assert();
}

#[tokio::test]
async fn test_list_profiles_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/v1/accounts");
        then.status(401)
            .json_body(serde_json::json!({
                "error": "Invalid API key"
            }));
    });

    let mut provider = ZernioProvider::new("invalid-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.list_profiles().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::AuthError(_) => {},
        other => panic!("Expected AuthError, got {:?}", other),
    }

    mock.assert();
}

#[tokio::test]
async fn test_test_connection_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/api/v1/accounts");
        then.status(200)
            .json_body(serde_json::json!({"accounts": []}));
    });

    let mut provider = ZernioProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.test_connection().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_test_connection_auth_failure() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/api/v1/profiles");
        then.status(401);
    });

    let mut provider = ZernioProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.test_connection().await;
    assert!(result.is_err());
}

