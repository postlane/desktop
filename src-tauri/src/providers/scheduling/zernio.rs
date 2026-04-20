// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
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

    /// Map Postlane platform names to Zernio's API platform identifiers.
    fn zernio_platform(platform: &str) -> &str {
        match platform {
            "x" => "twitter",
            other => other, // "bluesky", "mastodon", etc. are passed through as-is
        }
    }

    /// Helper to check HTTP status and return appropriate error
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();

        if status == 401 {
            return Err(ProviderError::AuthError("Invalid API key".to_string()));
        }

        if status == 429 {
            // Extract Retry-After header if present
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60); // Default to 60 seconds if header missing/invalid

            return Err(ProviderError::RateLimit(std::time::Duration::from_secs(retry_after)));
        }

        Ok(())
    }
}

#[async_trait]
impl SchedulingProvider for ZernioProvider {
    fn name(&self) -> &str {
        "zernio"
    }

    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        use super::with_retry;

        // Wrap in retry logic
        with_retry(
            || async {
                let url = format!("{}/api/v1/posts", self.base_url());

                // Build the platforms array Zernio expects:
                // [{"platform": "twitter", "accountId": "..."}]
                let mut platform_entry = serde_json::json!({
                    "platform": Self::zernio_platform(platform),
                });
                if let Some(account) = profile_id {
                    if !account.is_empty() {
                        platform_entry["accountId"] = serde_json::json!(account);
                    }
                }

                let mut body = serde_json::json!({
                    "content": content,
                    "platforms": [platform_entry],
                });

                // publishNow for immediate posts; scheduledFor + timezone for scheduled posts.
                if let Some(scheduled) = scheduled_for {
                    body["scheduledFor"] = serde_json::json!(scheduled.to_rfc3339());
                    body["timezone"] = serde_json::json!("UTC");
                } else {
                    body["publishNow"] = serde_json::json!(true);
                }

                if let Some(image) = image_url {
                    body["imageUrl"] = serde_json::json!(image);
                }

                let response = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

                Self::check_response_status(&response)?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    return Err(ProviderError::HttpError { status, body: body_text });
                }

                let json: serde_json::Value = response
                    .json()
                    .await
                    .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

                log::debug!("Zernio schedule_post response: {}", json);

                // Zernio returns the post ID at post._id
                let scheduler_id = json["post"]["_id"]
                    .as_str()
                    .ok_or_else(|| ProviderError::Unknown(
                        format!("Missing post._id in response. Full response: {}", json)
                    ))?
                    .to_string();

                // For publishNow posts Zernio includes platformPostUrl immediately.
                // For scheduled posts this field appears after publish time.
                let platform_url = json["post"]["platforms"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|p| p["platformPostUrl"].as_str())
                    .map(String::from);

                Ok(PostScheduleResult { scheduler_id, platform_url })
            },
            3,
        )
        .await
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        // Zernio's connected social accounts live at /api/v1/accounts (not /api/v1/profiles).
        // Each account is per-platform: { _id, username, platform }.
        let url = format!("{}/api/v1/accounts", self.base_url());

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        let status = response.status();
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

        log::debug!("Zernio list_profiles response: {}", json);

        let accounts_array = json["accounts"]
            .as_array()
            .ok_or_else(|| ProviderError::Unknown(
                format!("Missing accounts array in response. Full response: {}", json)
            ))?;

        let profiles = accounts_array
            .iter()
            .filter_map(|a| {
                let id = a["_id"].as_str()?.to_string();
                let name = a["username"].as_str()
                    .or_else(|| a["name"].as_str())?
                    .to_string();
                // Normalise Zernio's "twitter" to postlane's internal "x"
                let platform = match a["platform"].as_str()? {
                    "twitter" => "x",
                    other => other,
                }.to_string();
                Some(SchedulerProfile {
                    id,
                    name,
                    platforms: vec![platform],
                })
            })
            .collect();

        Ok(profiles)
    }

    async fn cancel_post(&self, post_id: &str, platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/api/v1/posts/{}/cancel", self.base_url(), post_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&serde_json::json!({
                "platform": platform
            }))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        let status = response.status();
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

        Ok(())
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        let url = format!("{}/api/v1/queue", self.base_url());

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        let status = response.status();
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

        let queue_array = json["posts"]
            .as_array()
            .ok_or_else(|| ProviderError::Unknown("Missing posts array".to_string()))?;

        let queue = queue_array
            .iter()
            .filter_map(|p| {
                let scheduled_for = p["scheduled_for"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))?;

                let content = p["content"].as_str()?.to_string();
                let content_preview = if content.chars().count() > 80 {
                    let truncated: String = content.chars().take(80).collect();
                    format!("{}...", truncated)
                } else {
                    content
                };

                Some(crate::types::QueuedPost {
                    post_id: p["post_id"].as_str()?.to_string(),
                    platform: p["platform"].as_str()?.to_string(),
                    scheduled_for,
                    content_preview,
                })
            })
            .collect();

        Ok(queue)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        // Test connection by making a lightweight API call to list profiles
        // If this succeeds, we know the API key is valid
        self.list_profiles().await?;
        Ok(())
    }

    async fn get_engagement(
        &self,
        post_id: &str,
        platform: &str,
    ) -> Result<Engagement, ProviderError> {
        let url = format!("{}/api/v1/posts/{}/engagement", self.base_url(), post_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .query(&[("platform", platform)])
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        let status = response.status();
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

        Ok(Engagement {
            likes: json["likes"].as_u64().unwrap_or(0),
            reposts: json["reposts"].as_u64().unwrap_or(0),
            replies: json["replies"].as_u64().unwrap_or(0),
            impressions: json["impressions"].as_u64(),
        })
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

    #[tokio::test]
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
}
