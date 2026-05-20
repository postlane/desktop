// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Outstand scheduling provider.
/// Pricing: $5/month for 1,000 posts, then $0.01 per additional post.
pub struct OutstandProvider {
    client: reqwest::Client,
    api_key: String,
    #[cfg(test)]
    base_url: String,
}

impl OutstandProvider {
    /// Create a new OutstandProvider.
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            #[cfg(test)]
            base_url: "https://api.outstand.so/v1".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://api.outstand.so/v1"
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check HTTP status and return appropriate `ProviderError`.
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();
        if status == 401 || status == 403 {
            return Err(ProviderError::AuthError("Invalid Outstand API key".to_string()));
        }
        if status == 429 {
            return Err(ProviderError::RateLimit(parse_retry_after(response)));
        }
        Ok(())
    }
}

#[async_trait]
impl SchedulingProvider for OutstandProvider {
    fn name(&self) -> &str {
        "outstand"
    }

    async fn schedule_post(
        &self,
        content: &str,
        _platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        let mut body = serde_json::json!({
            "containers": [{ "content": content }],
            "socialAccountIds": profile_id.map(|id| vec![id]).unwrap_or_default(),
        });
        if let Some(dt) = scheduled_for {
            body["scheduledAt"] = serde_json::json!(dt.to_rfc3339());
        }
        let url = format!("{}/posts", self.base_url());
        let response = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body: body_text });
        }
        let json: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
        let data = &json["data"];
        let scheduler_id = data["id"].as_str()
            .ok_or_else(|| ProviderError::Unknown(format!("Missing data.id in response: {}", json)))?
            .to_string();
        let platform_url = data["postUrl"].as_str().map(String::from);
        Ok(PostScheduleResult { scheduler_id, platform_url })
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/social-accounts", self.base_url());
        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse accounts: {}", e)))?;
        let profiles = json["data"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|a| {
                Some(SchedulerProfile {
                    id: a["id"].as_str()?.to_string(),
                    name: a["name"].as_str().unwrap_or("").to_string(),
                    platforms: vec![a["platform"].as_str().unwrap_or("unknown").to_string()],
                })
            })
            .collect();
        Ok(profiles)
    }

    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/posts/{}", self.base_url(), post_id);
        let response = self.client.delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        let status = response.status();
        if status == 405 {
            return Err(ProviderError::NotSupported(
                "Outstand does not support cancelling this post.".to_string(),
            ));
        }
        Self::check_response_status(&response)?;
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status: status.as_u16(), body });
        }
        Ok(())
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        let url = format!("{}/posts", self.base_url());
        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .query(&[("status", "scheduled")])
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse queue: {}", e)))?;
        let posts = json["data"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(|p| {
                let scheduled_for = p["scheduledAt"].as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&Utc))?;
                let content = p["containers"].as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|c| c["content"].as_str())
                    .unwrap_or("")
                    .to_string();
                let content_preview = if content.chars().count() > 80 {
                    format!("{}...", content.chars().take(80).collect::<String>())
                } else {
                    content
                };
                Some(crate::types::QueuedPost {
                    post_id: p["id"].as_str()?.to_string(),
                    platform: p["platform"].as_str().unwrap_or("unknown").to_string(),
                    scheduled_for,
                    content_preview,
                })
            })
            .collect();
        Ok(posts)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        let url = format!("{}/usage", self.base_url());
        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        Ok(())
    }

    async fn get_engagement(&self, post_id: &str, _platform: &str) -> Result<Engagement, ProviderError> {
        let url = format!("{}/posts/{}/analytics", self.base_url(), post_id);
        let response = self.client.get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = response.json().await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse engagement: {}", e)))?;
        let data = &json["data"];
        Ok(Engagement {
            likes: data["likes"].as_u64().unwrap_or(0),
            reposts: data["shares"].as_u64().unwrap_or(0),
            replies: data["comments"].as_u64().unwrap_or(0),
            impressions: data["reach"].as_u64(),
            platform_url: None,
        })
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests;
