// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Buffer scheduling provider
pub struct BufferProvider {
    /// Shared HTTP client with configured timeouts
    client: reqwest::Client,
    /// API key for authentication
    api_key: String,
    /// Base URL for Buffer API (configurable for testing)
    #[cfg(test)]
    base_url: String,
}

impl BufferProvider {
    /// Create a new BufferProvider
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            #[cfg(test)]
            base_url: "https://api.bufferapp.com".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://api.bufferapp.com"
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Helper to check HTTP status and return appropriate error
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();

        if status == 401 {
            return Err(ProviderError::AuthError("Invalid API key".to_string()));
        }

        if status == 429 {
            return Err(ProviderError::RateLimit(parse_retry_after(response)));
        }

        Ok(())
    }
}

#[async_trait]
impl SchedulingProvider for BufferProvider {
    fn name(&self) -> &str {
        "buffer"
    }

    async fn schedule_post(
        &self,
        content: &str,
        _platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        use super::with_retry;

        with_retry(
            || async {
                let url = format!("{}/1/updates/create.json", self.base_url());

                let mut body = serde_json::json!({
                    "text": content,
                    "profile_ids[]": profile_id.unwrap_or(""),
                });

                if let Some(scheduled) = scheduled_for {
                    body["scheduled_at"] = serde_json::json!(scheduled.timestamp());
                }

                if let Some(image) = image_url {
                    body["media[photo]"] = serde_json::json!(image);
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

                let update_id = json["updates"][0]["id"]
                    .as_str()
                    .ok_or_else(|| ProviderError::Unknown("Missing update id in response".to_string()))?
                    .to_string();

                Ok(PostScheduleResult { scheduler_id: update_id, platform_url: None })
            },
            3,
        )
        .await
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        // Buffer calls profiles "channels"
        let url = format!("{}/1/profiles.json", self.base_url());

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

        let channels: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        let profiles = channels
            .iter()
            .filter_map(|channel| {
                Some(SchedulerProfile {
                    id: channel["id"].as_str()?.to_string(),
                    name: channel["formatted_username"].as_str()
                        .or_else(|| channel["service_username"].as_str())
                        ?.to_string(),
                    platforms: vec![channel["service"].as_str()?.to_string()],
                })
            })
            .collect();

        Ok(profiles)
    }

    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/1/updates/{}/destroy.json", self.base_url(), post_id);

        let response = self
            .client
            .post(&url)
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

        Ok(())
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        let url = format!("{}/1/profiles.json", self.base_url());

        // First get all profiles
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let channels: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        let mut all_queue = Vec::new();

        // Get pending updates for each profile
        for channel in channels {
            if let Some(channel_id) = channel["id"].as_str() {
                let queue_url = format!("{}/1/profiles/{}/updates/pending.json", self.base_url(), channel_id);

                if let Ok(queue_response) = self
                    .client
                    .get(&queue_url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .send()
                    .await
                {
                    if let Ok(json) = queue_response.json::<serde_json::Value>().await {
                        if let Some(updates) = json["updates"].as_array() {
                            for update in updates {
                                if let Some(scheduled_at) = update["scheduled_at"].as_i64() {
                                    let scheduled_for = chrono::DateTime::from_timestamp(scheduled_at, 0)
                                        .map(|dt| dt.with_timezone(&chrono::Utc));

                                    if let Some(scheduled) = scheduled_for {
                                        let content = update["text"].as_str().unwrap_or("").to_string();
                                        let content_preview = if content.chars().count() > 80 {
                                            let truncated: String = content.chars().take(80).collect();
                                            format!("{}...", truncated)
                                        } else {
                                            content
                                        };

                                        all_queue.push(crate::types::QueuedPost {
                                            post_id: update["id"].as_str().unwrap_or("").to_string(),
                                            platform: channel["service"].as_str().unwrap_or("").to_string(),
                                            scheduled_for: scheduled,
                                            content_preview,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(all_queue)
    }

    async fn get_engagement(
        &self,
        post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        let url = format!("{}/1/updates/{}.json", self.base_url(), post_id);

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

        let statistics = &json["statistics"];

        Ok(Engagement {
            likes: statistics["likes"].as_u64().unwrap_or(0),
            reposts: statistics["shares"].as_u64().unwrap_or(0),
            replies: statistics["comments"].as_u64().unwrap_or(0),
            impressions: statistics["reach"].as_u64(),
            platform_url: None,
        })
    }

    fn post_url(&self, platform: &str, post_id: &str) -> Option<String> {
        match platform {
            "x" | "twitter" => Some(format!("https://x.com/i/web/status/{}", post_id)),
            "bluesky" => None, // Requires handle, not available
            "mastodon" => None, // Instance-specific
            "facebook" => Some(format!("https://facebook.com/{}", post_id)),
            "linkedin" => Some(format!("https://linkedin.com/feed/update/{}", post_id)),
            "instagram" => Some(format!("https://instagram.com/p/{}", post_id)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
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
}
