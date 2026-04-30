// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Ayrshare scheduling provider
pub struct AyrshareProvider {
    /// Shared HTTP client with configured timeouts
    client: reqwest::Client,
    /// API key for authentication
    api_key: String,
    /// Base URL for Ayrshare API (configurable for testing)
    #[cfg(test)]
    base_url: String,
}

impl AyrshareProvider {
    /// Create a new AyrshareProvider
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            #[cfg(test)]
            base_url: "https://app.ayrshare.com".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://app.ayrshare.com"
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
impl SchedulingProvider for AyrshareProvider {
    fn name(&self) -> &str {
        "ayrshare"
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
                let url = format!("{}/api/post", self.base_url());

                let mut body = serde_json::json!({
                    "post": content,
                    "platforms": [platform],
                });

                if let Some(profile) = profile_id {
                    body["profileKey"] = serde_json::json!(profile);
                }

                if let Some(scheduled) = scheduled_for {
                    body["scheduleDate"] = serde_json::json!(scheduled.to_rfc3339());
                }

                if let Some(image) = image_url {
                    body["mediaUrls"] = serde_json::json!([image]);
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

                let post_id = json["id"]
                    .as_str()
                    .ok_or_else(|| ProviderError::Unknown("Missing id in response".to_string()))?
                    .to_string();

                Ok(PostScheduleResult { scheduler_id: post_id, platform_url: None })
            },
            3,
        )
        .await
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/api/profiles", self.base_url());

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

        let profiles_array = json["profiles"]
            .as_array()
            .ok_or_else(|| ProviderError::Unknown("Missing profiles array".to_string()))?;

        let profiles = profiles_array
            .iter()
            .filter_map(|p| {
                Some(SchedulerProfile {
                    id: p["profileKey"].as_str()?.to_string(),
                    name: p["title"].as_str()?.to_string(),
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

    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/api/post/{}", self.base_url(), post_id);

        let response = self
            .client
            .delete(&url)
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
        let url = format!("{}/api/history", self.base_url());

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
                let scheduled_for = p["scheduleDate"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))?;

                let content = p["post"].as_str()?.to_string();
                let content_preview = if content.chars().count() > 80 {
                    let truncated: String = content.chars().take(80).collect();
                    format!("{}...", truncated)
                } else {
                    content
                };

                Some(crate::types::QueuedPost {
                    post_id: p["id"].as_str()?.to_string(),
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
        let url = format!("{}/api/analytics/post/{}", self.base_url(), post_id);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .query(&[("platforms", platform)])
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
            reposts: json["retweets"].as_u64().unwrap_or(0),
            replies: json["comments"].as_u64().unwrap_or(0),
            impressions: json["impressions"].as_u64(),
            platform_url: None,
        })
    }

    fn post_url(&self, platform: &str, post_id: &str) -> Option<String> {
        match platform {
            "x" | "twitter" => Some(format!("https://x.com/i/web/status/{}", post_id)),
            "bluesky" => {
                // Bluesky URLs require the handle, which we don't have here
                // Return None - will be enhanced when handle is available from Ayrshare
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
