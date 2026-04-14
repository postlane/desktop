// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Zernio scheduling provider
pub struct ZernioProvider {
    /// Shared HTTP client with configured timeouts
    client: reqwest::Client,
    /// API key for authentication
    api_key: String,
    /// Base URL for Zernio API (configurable for testing)
    #[cfg(test)]
    base_url: String,
}

impl ZernioProvider {
    /// Create a new ZernioProvider
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            #[cfg(test)]
            base_url: "https://api.zernio.com".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://api.zernio.com"
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[async_trait]
impl SchedulingProvider for ZernioProvider {
    fn name(&self) -> &str {
        "zernio"
    }

    async fn schedule_post(
        &self,
        _content: &str,
        _platform: &str,
        _scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        _profile_id: Option<&str>,
    ) -> Result<String, ProviderError> {
        // Stub implementation - will be implemented in 4.4.2
        Err(ProviderError::NotSupported)
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/api/v1/profiles", self.base_url());

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let status = response.status();

        if status == 401 {
            return Err(ProviderError::AuthError("Invalid API key".to_string()));
        }

        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        let profiles_array = json["profiles"]
            .as_array()
            .ok_or_else(|| ProviderError::Unknown("Missing profiles array".to_string()))?;

        let profiles = profiles_array
            .iter()
            .filter_map(|p| {
                Some(SchedulerProfile {
                    id: p["id"].as_str()?.to_string(),
                    name: p["name"].as_str()?.to_string(),
                    platforms: p["platforms"]
                        .as_array()?
                        .iter()
                        .filter_map(|platform| platform.as_str().map(|s| s.to_string()))
                        .collect(),
                })
            })
            .collect();

        Ok(profiles)
    }

    async fn cancel_post(&self, _post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        // Stub implementation - will be implemented in 4.4.4
        Err(ProviderError::NotSupported)
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        // Stub implementation - will be implemented in 4.4.5
        Err(ProviderError::NotSupported)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        // Test connection by making a lightweight API call to list profiles
        // If this succeeds, we know the API key is valid
        self.list_profiles().await?;
        Ok(())
    }

    async fn get_engagement(
        &self,
        _post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        // Stub implementation - will be implemented in 4.4.7
        Err(ProviderError::NotSupported)
    }

    fn post_url(&self, platform: &str, post_id: &str) -> Option<String> {
        match platform {
            "x" | "twitter" => Some(format!("https://x.com/i/web/status/{}", post_id)),
            "bluesky" => {
                // Bluesky URLs require the handle, which we don't have here
                // Return None - will be enhanced when handle is available from Zernio
                None
            }
            "mastodon" => {
                // Mastodon URLs are instance-specific, need the instance domain
                // Return None - provider-specific
                None
            }
            _ => None, // Unsupported platform
        }
    }
}

#[cfg(test)]
mod tests {
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

        // Mock server
        let server = MockServer::start();

        // Mock the profiles endpoint
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/v1/profiles")
                .header("Authorization", "Bearer test-api-key");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({
                    "profiles": [
                        {
                            "id": "profile-1",
                            "name": "My X Account",
                            "platforms": ["x", "twitter"]
                        },
                        {
                            "id": "profile-2",
                            "name": "My Bluesky",
                            "platforms": ["bluesky"]
                        }
                    ]
                }));
        });

        // Create provider with mock server URL
        let mut provider = ZernioProvider::new("test-api-key".to_string());
        provider.base_url = server.base_url();

        // Call list_profiles
        let result = provider.list_profiles().await;

        // Verify success
        assert!(result.is_ok());
        let profiles = result.unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].id, "profile-1");
        assert_eq!(profiles[0].name, "My X Account");
        assert_eq!(profiles[0].platforms, vec!["x", "twitter"]);

        // Verify the mock was called
        mock.assert();
    }

    #[tokio::test]
    async fn test_list_profiles_auth_error() {
        use httpmock::prelude::*;

        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/v1/profiles");
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
            when.method(GET).path("/api/v1/profiles");
            then.status(200)
                .json_body(serde_json::json!({"profiles": []}));
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
}
