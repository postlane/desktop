// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::OnceCell;

/// How long to wait between Publer job-status polls.
/// Set to 0 in tests so the timeout test completes instantly.
#[cfg(not(test))]
const POLL_INTERVAL_SECS: u64 = 5;
#[cfg(test)]
const POLL_INTERVAL_SECS: u64 = 0;

/// Maximum number of job-status polls before giving up.
const MAX_POLLS: u32 = 5;

/// Publer scheduling provider.
/// Workspace ID is fetched lazily from `/api/v1/workspaces` on first use.
pub struct PublerProvider {
    client: reqwest::Client,
    api_key: String,
    workspace_id: OnceCell<String>,
    #[cfg(test)]
    base_url: String,
}

impl PublerProvider {
    /// Create a new PublerProvider. Workspace ID is resolved on first API call.
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            workspace_id: OnceCell::new(),
            #[cfg(test)]
            base_url: "https://app.publer.com/api/v1".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://app.publer.com/api/v1"
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Resolve and cache the workspace ID, fetching it if not yet known.
    async fn workspace_id(&self) -> Result<&str, ProviderError> {
        let client = &self.client;
        let api_key = &self.api_key;
        let base_url = self.base_url().to_string();
        self.workspace_id
            .get_or_try_init(|| async move {
                fetch_workspace_id(client, api_key, &base_url).await
            })
            .await
            .map(|s| s.as_str())
    }

    /// Build the Authorization and workspace headers for every request.
    fn auth_headers(&self, workspace_id: &str) -> [(&'static str, String); 2] {
        [
            ("Authorization", format!("Bearer-API {}", self.api_key)),
            ("Publer-Workspace-Id", workspace_id.to_string()),
        ]
    }

    /// Check HTTP status and return the appropriate `ProviderError`.
    /// Publer uses `X-RateLimit-Reset` (Unix timestamp) rather than `Retry-After`.
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();
        if status == 401 {
            return Err(ProviderError::AuthError("Invalid Publer API key".to_string()));
        }
        if status == 403 {
            return Err(ProviderError::AuthError(
                "Publer returned 403. API access may require a paid plan — check publer.com/plans or switch to a different scheduler.".to_string(),
            ));
        }
        if status == 429 {
            let reset = response
                .headers()
                .get("X-RateLimit-Reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            let duration = match reset {
                Some(ts) => {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    std::time::Duration::from_secs(ts.saturating_sub(now).clamp(1, 3600))
                }
                None => std::time::Duration::from_secs(60),
            };
            return Err(ProviderError::RateLimit(duration));
        }
        Ok(())
    }

    /// Poll `GET /job_status/{job_id}` until the job completes or all attempts are exhausted.
    async fn poll_job(&self, job_id: &str, workspace_id: &str) -> Result<String, ProviderError> {
        let url = format!("{}/job_status/{}", self.base_url(), job_id);
        let headers = self.auth_headers(workspace_id);
        for attempt in 0..MAX_POLLS {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;
            }
            let response = self.client.get(&url)
                .header(headers[0].0, &headers[0].1)
                .header(headers[1].0, &headers[1].1)
                .send()
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
            let json: serde_json::Value = response.json().await
                .map_err(|e| ProviderError::Unknown(format!("Failed to parse job status: {}", e)))?;
            if json["status"].as_str() == Some("completed") {
                return json["post"]["id"].as_str()
                    .map(String::from)
                    .ok_or_else(|| ProviderError::Unknown(
                        format!("Missing post.id in completed job. Response: {}", json)
                    ));
            }
        }
        log::warn!("Publer job {} did not complete within {}s", job_id, MAX_POLLS * POLL_INTERVAL_SECS as u32);
        Err(ProviderError::Unknown(format!(
            "Publer job {} did not complete after {} polls. Check your Publer dashboard for post status.",
            job_id, MAX_POLLS
        )))
    }
}

/// Fetch the first workspace ID for this API key.
async fn fetch_workspace_id(
    client: &reqwest::Client,
    api_key: &str,
    base_url: &str,
) -> Result<String, ProviderError> {
    let url = format!("{}/workspaces", base_url);
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer-API {}", api_key))
        .send()
        .await
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
    PublerProvider::check_response_status(&response)?;
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ProviderError::Unknown(format!("Failed to parse workspaces: {}", e)))?;
    json.as_array()
        .and_then(|arr| arr.first())
        .and_then(|w| w["id"].as_str())
        .map(String::from)
        .ok_or_else(|| ProviderError::Unknown("No workspaces found for this Publer API key".to_string()))
}

#[async_trait]
impl SchedulingProvider for PublerProvider {
    fn name(&self) -> &str {
        "publer"
    }

    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let body = build_schedule_body(content, platform, scheduled_for, profile_id);
        let url = format!("{}/posts/schedule", self.base_url());
        let response = self.client.post(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
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
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse schedule response: {}", e)))?;
        let job_id = json["job_id"].as_str()
            .ok_or_else(|| ProviderError::Unknown(format!("Missing job_id in response: {}", json)))?;
        let post_id = self.poll_job(job_id, &workspace_id).await?;
        Ok(PostScheduleResult { scheduler_id: post_id, platform_url: None })
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let url = format!("{}/accounts", self.base_url());
        let response = self.client.get(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
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
        let profiles = json.as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|a| {
                Some(SchedulerProfile {
                    id: a["id"].as_str()?.to_string(),
                    name: a["name"].as_str().or_else(|| a["username"].as_str())?.to_string(),
                    platforms: vec![a["type"].as_str().unwrap_or("unknown").to_string()],
                })
            })
            .collect();
        Ok(profiles)
    }

    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let url = format!("{}/posts/{}", self.base_url(), post_id);
        let response = self.client.delete(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        let status = response.status();
        if status == 404 || status == 405 {
            return Err(ProviderError::NotSupported(
                "Publer does not support cancelling this post.".to_string(),
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
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let url = format!("{}/posts", self.base_url());
        let response = self.client.get(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
            .query(&[("state", "scheduled")])
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
        let posts = json["posts"].as_array().unwrap_or(&vec![]).iter()
            .filter_map(parse_queued_post)
            .collect();
        Ok(posts)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let url = format!("{}/users/me", self.base_url());
        let response = self.client.get(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
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
        let workspace_id = self.workspace_id().await?.to_string();
        let headers = self.auth_headers(&workspace_id);
        let url = format!("{}/posts/{}", self.base_url(), post_id);
        let response = self.client.get(&url)
            .header(headers[0].0, &headers[0].1)
            .header(headers[1].0, &headers[1].1)
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
        let analytics = &json["analytics"];
        Ok(Engagement {
            likes: analytics["likes"].as_u64().unwrap_or(0),
            reposts: analytics["shares"].as_u64().unwrap_or(0),
            replies: analytics["comments"].as_u64().unwrap_or(0),
            impressions: analytics["reach"].as_u64(),
            platform_url: None,
        })
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        None
    }
}

/// Build the Publer schedule-post request body.
fn build_schedule_body(
    content: &str,
    platform: &str,
    scheduled_for: Option<DateTime<Utc>>,
    profile_id: Option<&str>,
) -> serde_json::Value {
    let account_entry = if let Some(id) = profile_id {
        let mut entry = serde_json::json!({ "id": id });
        if let Some(dt) = scheduled_for {
            entry["scheduled_at"] = serde_json::json!(dt.to_rfc3339());
        }
        entry
    } else {
        serde_json::json!({})
    };
    serde_json::json!({
        "bulk": {
            "state": "scheduled",
            "posts": [{
                "networks": { platform: { "type": "feed", "text": content } },
                "accounts": [account_entry]
            }]
        }
    })
}

/// Parse a Publer post object into a `QueuedPost`.
fn parse_queued_post(p: &serde_json::Value) -> Option<crate::types::QueuedPost> {
    let post_id = p["id"].as_str()?.to_string();
    let platform = p["account_id"].as_str().unwrap_or("unknown").to_string();
    let scheduled_for = p["scheduled_at"].as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))?;
    let content = p["text"].as_str().unwrap_or("").to_string();
    let content_preview = if content.chars().count() > 80 {
        format!("{}...", content.chars().take(80).collect::<String>())
    } else {
        content
    };
    Some(crate::types::QueuedPost { post_id, platform, scheduled_for, content_preview })
}


#[cfg(test)]
mod tests;
