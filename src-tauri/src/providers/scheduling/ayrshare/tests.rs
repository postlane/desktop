// SPDX-License-Identifier: BUSL-1.1
use super::*;


#[test]
fn test_ayrshare_provider_stores_client() {
    // Test: AyrshareProvider should store a reqwest::Client at instantiation
    let provider = AyrshareProvider::new("test-api-key".to_string());

    // Verify the provider was created and has a name
    assert_eq!(provider.name(), "ayrshare");
}

#[test]
fn test_ayrshare_provider_instantiation() {
    // Test: Creating AyrshareProvider should not panic
    let _provider = AyrshareProvider::new("api-key-xyz".to_string());
    // If we get here without panic, the test passes
}

#[tokio::test]
async fn test_list_profiles_success() {
    use httpmock::prelude::*;

    // Mock server
    let server = MockServer::start();

    // Mock the profiles endpoint
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/profiles")
            .header("Authorization", "Bearer test-api-key");
        then.status(200)
            .header("content-type", "application/json")
            .json_body(serde_json::json!({
                "profiles": [
                    {
                        "profileKey": "profile-1",
                        "title": "My X Account",
                        "platforms": ["twitter"]
                    },
                    {
                        "profileKey": "profile-2",
                        "title": "My Bluesky",
                        "platforms": ["bluesky"]
                    }
                ]
            }));
    });

    // Create provider with mock server URL
    let mut provider = AyrshareProvider::new("test-api-key".to_string());
    provider.base_url = server.base_url();

    // Call list_profiles
    let result = provider.list_profiles().await;

    // Verify success
    assert!(result.is_ok());
    let profiles = result.unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "profile-1");
    assert_eq!(profiles[0].name, "My X Account");
    assert_eq!(profiles[0].platforms, vec!["twitter"]);

    // Verify the mock was called
    mock.assert();
}

#[tokio::test]
async fn test_list_profiles_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/api/profiles");
        then.status(401)
            .json_body(serde_json::json!({
                "error": "Invalid API key"
            }));
    });

    let mut provider = AyrshareProvider::new("invalid-key".to_string());
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
        when.method(GET).path("/api/profiles");
        then.status(200)
            .json_body(serde_json::json!({"profiles": []}));
    });

    let mut provider = AyrshareProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.test_connection().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_test_connection_auth_failure() {
    use httpmock::prelude::*;

    let server = MockServer::start();

    server.mock(|when, then| {
        when.method(GET).path("/api/profiles");
        then.status(401);
    });

    let mut provider = AyrshareProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let result = provider.test_connection().await;
    assert!(result.is_err());
}
