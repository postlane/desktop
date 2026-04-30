// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
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

    /// Build the JSON request body for the `/api/v1/posts` endpoint.
    fn build_schedule_body(
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> serde_json::Value {
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

        if let Some(scheduled) = scheduled_for {
            body["scheduledFor"] = serde_json::json!(scheduled.to_rfc3339());
            body["timezone"] = serde_json::json!("UTC");
        } else {
            body["publishNow"] = serde_json::json!(true);
        }

        if let Some(image) = image_url {
            body["imageUrl"] = serde_json::json!(image);
        }

        body
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

        let body = Self::build_schedule_body(content, platform, scheduled_for, image_url, profile_id);

        with_retry(
            || async {
                let url = format!("{}/api/v1/posts", self.base_url());

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

                // lgtm[rust/cleartext-logging] -- logs response body only; api_key is in the request Authorization header and never echoed back
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

        // lgtm[rust/cleartext-logging] -- logs response body only; api_key is in the request Authorization header and never echoed back
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
            platform_url: None,
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
mod tests;
#[cfg(test)]
mod tests_schedule;
