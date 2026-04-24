// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
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
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60)
                .min(3600);
            return Err(ProviderError::RateLimit(std::time::Duration::from_secs(retry_after)));
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
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_provider(server: &MockServer) -> OutstandProvider {
        let mut p = OutstandProvider::new("test-key".to_string());
        p.base_url = server.base_url();
        p
    }

    #[tokio::test]
    async fn test_schedule_post_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/posts");
            then.status(200).json_body(serde_json::json!({
                "data": { "id": "out_abc123", "postUrl": "https://linkedin.com/posts/out_abc123" }
            }));
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello", "linkedin", None, None, Some("acc-1")).await;
        assert!(result.is_ok(), "{:?}", result);
        let res = result.unwrap();
        assert_eq!(res.scheduler_id, "out_abc123");
        assert_eq!(res.platform_url, Some("https://linkedin.com/posts/out_abc123".to_string()));
    }

    #[tokio::test]
    async fn test_schedule_post_unauthorised() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/posts");
            then.status(403);
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
    }

    #[tokio::test]
    async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/posts");
            then.status(429).header("Retry-After", "60");
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        match result {
            Err(ProviderError::RateLimit(d)) => assert_eq!(d.as_secs(), 60),
            other => panic!("expected RateLimit, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cancel_post_not_supported() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(DELETE).path("/posts/post-1");
            then.status(405);
        });
        let provider = make_provider(&server);
        let result = provider.cancel_post("post-1", "linkedin").await;
        assert!(matches!(result, Err(ProviderError::NotSupported(_))), "{:?}", result);
    }

    #[tokio::test]
    async fn test_get_engagement_partial() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/posts/post-1/analytics");
            then.status(200).json_body(serde_json::json!({
                "data": { "likes": 10, "shares": null, "comments": null, "reach": null }
            }));
        });
        let provider = make_provider(&server);
        let result = provider.get_engagement("post-1", "linkedin").await;
        assert!(result.is_ok(), "{:?}", result);
        let eng = result.unwrap();
        assert_eq!(eng.likes, 10);
        assert_eq!(eng.reposts, 0);
        assert_eq!(eng.impressions, None);
    }
}
