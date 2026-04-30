// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Mastodon direct-API scheduling provider.
///
/// Posts directly to the Mastodon API without an intermediary scheduler.
/// Each instance is a separate OAuth server — `access_token` is obtained
/// via the OAuth flow in `mastodon_commands.rs`.
#[derive(Debug)]
pub struct MastodonProvider {
    /// Shared HTTP client with configured timeouts
    client: reqwest::Client,
    /// Precomputed API base URL: `https://{instance}/api/v1`
    api_base: String,
    /// OAuth access token for the authenticated user
    access_token: String,
}

impl MastodonProvider {
    /// Create a new MastodonProvider, validating the instance against SSRF rules first.
    ///
    /// Rejects private IP ranges before making any HTTP request.
    /// Use this in all production code paths.
    pub async fn create(instance: &str, access_token: String) -> Result<Self, ProviderError> {
        validate_instance_domain(instance).await?;
        Ok(Self::new(instance, access_token))
    }

    /// Construct without SSRF validation — for tests only.
    pub(crate) fn new(instance: &str, access_token: String) -> Self {
        Self {
            client: build_client(),
            api_base: format!("https://{}/api/v1", instance),
            access_token,
        }
    }

    fn base_url(&self) -> &str {
        &self.api_base
    }

    /// Check for known error status codes and return the appropriate error.
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();
        if status == 401 {
            return Err(ProviderError::AuthError("Invalid access token".to_string()));
        }
        if status == 429 {
            return Err(ProviderError::RateLimit(parse_retry_after(response)));
        }
        Ok(())
    }

    /// Parse a Mastodon status or scheduled_status response into a `PostScheduleResult`.
    ///
    /// Immediate posts return a `Status` object (has `url`, no `scheduled_at`).
    /// Scheduled posts return a `ScheduledStatus` object (has `scheduled_at`, no `url`).
    fn parse_schedule_response(json: &serde_json::Value) -> Result<PostScheduleResult, ProviderError> {
        let scheduler_id = json["id"]
            .as_str()
            .ok_or_else(|| ProviderError::Unknown(format!("Missing id in response: {}", json)))?
            .to_string();

        // scheduled_at present and non-null → ScheduledStatus; otherwise → immediate Status
        let is_scheduled = json.get("scheduled_at")
            .map(|v| !v.is_null())
            .unwrap_or(false);

        let platform_url = if is_scheduled {
            None
        } else {
            json["url"].as_str().map(String::from)
        };

        Ok(PostScheduleResult { scheduler_id, platform_url })
    }

    /// Fetch the instance character limit from `GET /api/v1/instance`.
    ///
    /// Returns `configuration.statuses.max_characters` or 500 on any failure.
    pub async fn fetch_instance_char_limit(&self) -> u32 {
        let url = format!("{}/instance", self.base_url());
        let response = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => return 500,
        };
        if !response.status().is_success() {
            return 500;
        }
        let json: serde_json::Value = match response.json().await {
            Ok(j) => j,
            Err(_) => return 500,
        };
        json["configuration"]["statuses"]["max_characters"]
            .as_u64()
            .map(|v| v as u32)
            .unwrap_or(500)
    }
}

/// Validates that a Mastodon instance hostname does not resolve to a private IP.
///
/// Rejects loopback, RFC 1918, link-local, and ULA IPv6 addresses to prevent SSRF.
pub async fn validate_instance_domain(instance: &str) -> Result<(), ProviderError> {
    let addr = format!("{}:443", instance);
    let addrs: Vec<_> = tokio::net::lookup_host(&addr)
        .await
        .map_err(|e| ProviderError::InvalidInstance(format!("Cannot resolve {}: {}", instance, e)))?
        .collect();

    if addrs.is_empty() {
        return Err(ProviderError::InvalidInstance(
            format!("Instance {} resolved to no addresses", instance),
        ));
    }

    for socket_addr in &addrs {
        if crate::security::ssrf_check::is_private_ip(socket_addr.ip()) {
            return Err(ProviderError::InvalidInstance(format!(
                "Instance {} resolves to a private IP address ({})",
                instance,
                socket_addr.ip()
            )));
        }
    }
    Ok(())
}

#[async_trait]
impl SchedulingProvider for MastodonProvider {
    fn name(&self) -> &str {
        "mastodon"
    }

    /// Post directly to the Mastodon instance.
    ///
    /// Passes `scheduled_at` in ISO 8601 when scheduling a future post;
    /// omits it for immediate publishing.
    async fn schedule_post(
        &self,
        content: &str,
        _platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        _profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        let url = format!("{}/statuses", self.base_url());

        let mut body = serde_json::json!({ "status": content });
        if let Some(scheduled) = scheduled_for {
            body["scheduled_at"] = serde_json::json!(scheduled.to_rfc3339());
        }

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
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

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        Self::parse_schedule_response(&json)
    }

    /// Cancel a scheduled Mastodon post via `DELETE /api/v1/scheduled_statuses/{id}`.
    ///
    /// Returns `NotSupported` when the post was published immediately (not scheduled)
    /// because immediate Mastodon posts cannot be deleted via the API outside of a client.
    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/scheduled_statuses/{}", self.base_url(), post_id);

        let response = self
            .client
            .delete(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if response.status() == 404 {
            return Err(ProviderError::NotSupported(
                "Mastodon posts cannot be deleted via Postlane. Delete the post from your Mastodon client.".to_string(),
            ));
        }

        Self::check_response_status(&response)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }

        Ok(())
    }

    /// Fetch the authenticated account via `GET /api/v1/accounts/verify_credentials`.
    ///
    /// Mastodon is single-account per token — returns exactly one `SchedulerProfile`.
    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/accounts/verify_credentials", self.base_url());

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        let id = json["id"].as_str().unwrap_or("").to_string();
        let name = json["display_name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| json["acct"].as_str().unwrap_or(""))
            .to_string();

        Ok(vec![SchedulerProfile {
            id,
            name,
            platforms: vec!["mastodon".to_string()],
        }])
    }

    /// Fetch the list of scheduled posts from `GET /api/v1/scheduled_statuses`.
    ///
    /// Follows `Link: rel="next"` pagination until all pages are exhausted.
    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        let mut all_posts = Vec::new();
        let mut next_url = Some(format!("{}/scheduled_statuses", self.base_url()));

        while let Some(url) = next_url {
            let response = self
                .client
                .get(&url)
                .bearer_auth(&self.access_token)
                .send()
                .await
                .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

            Self::check_response_status(&response)?;

            if !response.status().is_success() {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(ProviderError::HttpError { status, body });
            }

            next_url = parse_link_next(response.headers());

            let items: Vec<serde_json::Value> = response
                .json()
                .await
                .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

            all_posts.extend(items.iter().filter_map(map_scheduled_status));
        }

        Ok(all_posts)
    }

    /// Test the connection by verifying credentials.
    ///
    /// Fetch engagement metrics for a published post via `GET /api/v1/statuses/{post_id}`.
    ///
    /// Maps `favourites_count` → likes, `reblogs_count` → reposts, `replies_count` → replies.
    /// `impressions` is always `None` — Mastodon does not expose impression counts.
    async fn get_engagement(
        &self,
        post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        let url = format!("{}/statuses/{}", self.base_url(), post_id);

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        Self::check_response_status(&response)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;

        Ok(Engagement {
            likes: json["favourites_count"].as_u64().unwrap_or(0),
            reposts: json["reblogs_count"].as_u64().unwrap_or(0),
            replies: json["replies_count"].as_u64().unwrap_or(0),
            impressions: None, // Mastodon does not expose impression counts
            platform_url: json["url"].as_str().map(String::from),
        })
    }

    /// Returns `None` — the public URL is captured from the `schedule_post` response
    /// and stored in `meta.json`; it cannot be reconstructed from post_id alone.
    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        None
    }
}

/// Parses the `Link` header and returns the URL for `rel="next"`, if present.
///
/// Mastodon pagination uses RFC 5988 link headers:
/// `<https://host/path?max_id=123>; rel="next"`
fn parse_link_next(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let link = headers.get("link")?.to_str().ok()?;
    for part in link.split(',') {
        let part = part.trim();
        if part.contains(r#"rel="next""#) {
            if let Some(url) = part.split(';').next() {
                let url = url.trim().trim_start_matches('<').trim_end_matches('>');
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

/// Maps a Mastodon scheduled_status JSON object to a `QueuedPost`.
fn map_scheduled_status(item: &serde_json::Value) -> Option<crate::types::QueuedPost> {
    let post_id = item["id"].as_str()?.to_string();
    let scheduled_for = item["scheduled_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))?;

    let text = item["params"]["text"].as_str().unwrap_or("");
    let content_preview = if text.chars().count() > 80 {
        let truncated: String = text.chars().take(80).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    };

    Some(crate::types::QueuedPost {
        post_id,
        platform: "mastodon".to_string(),
        scheduled_for,
        content_preview,
    })
}


#[cfg(test)]
use httpmock::prelude::*;

#[cfg(test)]
fn make_provider(server: &MockServer) -> MastodonProvider {
    MastodonProvider {
        client: build_client(),
        api_base: server.base_url(),
        access_token: "test-token".to_string(),
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_helpers;
