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
mod tests;
