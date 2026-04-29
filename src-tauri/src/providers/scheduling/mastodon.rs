// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::net::IpAddr;

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
        if is_private_ip(socket_addr.ip()) {
            return Err(ProviderError::InvalidInstance(format!(
                "Instance {} resolves to a private IP address ({})",
                instance,
                socket_addr.ip()
            )));
        }
    }
    Ok(())
}

/// Returns true if the IP falls within a private, loopback, link-local, or ULA range.
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 127
                || o[0] == 10
                || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 169 && o[1] == 254)
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback() || (s[0] & 0xfe00) == 0xfc00
        }
    }
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
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_provider(server: &MockServer) -> MastodonProvider {
        MastodonProvider {
            client: build_client(),
            api_base: server.base_url(),
            access_token: "test-token".to_string(),
        }
    }

    // 9.5.1 — immediate post returns Status shape; post_id and post_url extracted correctly
    #[tokio::test]
    async fn test_schedule_post_immediate_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/statuses");
            then.status(200).json_body(serde_json::json!({
                "id": "103704874086360371",
                "url": "https://mastodon.social/@alice/103704874086360371",
                "content": "<p>Hello world</p>",
                "created_at": "2019-12-05T11:34:47.196Z"
            }));
        });

        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello world", "mastodon", None, None, None).await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let r = result.unwrap();
        assert_eq!(r.scheduler_id, "103704874086360371");
        assert_eq!(r.platform_url, Some("https://mastodon.social/@alice/103704874086360371".to_string()));
        mock.assert();
    }

    // 9.5.2 — scheduled post returns ScheduledStatus shape; post_url is None
    #[tokio::test]
    async fn test_schedule_post_scheduled_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/statuses");
            then.status(200).json_body(serde_json::json!({
                "id": "3221",
                "scheduled_at": "2019-12-05T12:33:01.000Z",
                "params": { "text": "Hello future world" }
            }));
        });

        let provider = make_provider(&server);
        let scheduled = chrono::DateTime::parse_from_rfc3339("2019-12-05T12:33:01Z")
            .unwrap()
            .with_timezone(&Utc);
        let result = provider.schedule_post("Hello future world", "mastodon", Some(scheduled), None, None).await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let r = result.unwrap();
        assert_eq!(r.scheduler_id, "3221");
        assert!(r.platform_url.is_none(), "Scheduled post should have no platform_url");
        mock.assert();
    }

    // 9.5.3 — both response shapes return Ok (no panic on either)
    #[tokio::test]
    async fn test_schedule_post_handles_both_response_shapes() {
        let server = MockServer::start();

        // Immediate (Status shape)
        server.mock(|when, then| {
            when.method(POST).path("/statuses").body_contains("immediate");
            then.status(200).json_body(serde_json::json!({
                "id": "111",
                "url": "https://mastodon.social/@alice/111",
                "created_at": "2024-01-01T00:00:00Z"
            }));
        });

        // Scheduled (ScheduledStatus shape)
        server.mock(|when, then| {
            when.method(POST).path("/statuses").body_contains("scheduled");
            then.status(200).json_body(serde_json::json!({
                "id": "222",
                "scheduled_at": "2024-06-01T10:00:00.000Z",
                "params": { "text": "scheduled content" }
            }));
        });

        let provider = make_provider(&server);
        let future = chrono::DateTime::parse_from_rfc3339("2024-06-01T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let r1 = provider.schedule_post("immediate content", "mastodon", None, None, None).await;
        let r2 = provider.schedule_post("scheduled content", "mastodon", Some(future), None, None).await;

        assert!(r1.is_ok(), "Immediate shape should be Ok: {:?}", r1);
        assert!(r2.is_ok(), "Scheduled shape should be Ok: {:?}", r2);
    }

    // 9.5.4 — cancel scheduled post succeeds with 200
    #[tokio::test]
    async fn test_cancel_post_scheduled_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE).path("/scheduled_statuses/3221");
            then.status(200).json_body(serde_json::json!({}));
        });

        let provider = make_provider(&server);
        let result = provider.cancel_post("3221", "mastodon").await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        mock.assert();
    }

    // 9.5.5 — cancel immediate post returns NotSupported with correct message
    #[tokio::test]
    async fn test_cancel_post_immediate_not_supported() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE).path("/scheduled_statuses/103704874086360371");
            then.status(404);
        });

        let provider = make_provider(&server);
        let result = provider.cancel_post("103704874086360371", "mastodon").await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::NotSupported(msg) => {
                assert!(msg.contains("Mastodon posts cannot be deleted"), "Unexpected message: {}", msg);
            }
            other => panic!("Expected NotSupported, got {:?}", other),
        }
        mock.assert();
    }

    // 9.5.6 — verify_credentials maps to exactly one SchedulerProfile
    #[tokio::test]
    async fn test_list_profiles_returns_single_profile() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/accounts/verify_credentials");
            then.status(200).json_body(serde_json::json!({
                "id": "14715",
                "display_name": "Alice Bobsworth",
                "acct": "alice"
            }));
        });

        let provider = make_provider(&server);
        let result = provider.list_profiles().await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let profiles = result.unwrap();
        assert_eq!(profiles.len(), 1, "Mastodon returns exactly one profile");
        assert_eq!(profiles[0].id, "14715");
        assert_eq!(profiles[0].name, "Alice Bobsworth");
        assert_eq!(profiles[0].platforms, vec!["mastodon"]);
        mock.assert();
    }

    // 9.5.7 — scheduled_statuses array maps to Vec<QueuedPost>
    #[tokio::test]
    async fn test_get_queue_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/scheduled_statuses");
            then.status(200).json_body(serde_json::json!([
                {
                    "id": "3221",
                    "scheduled_at": "2024-06-01T12:00:00.000Z",
                    "params": { "text": "First scheduled post" }
                },
                {
                    "id": "3222",
                    "scheduled_at": "2024-06-02T12:00:00.000Z",
                    "params": { "text": "Second scheduled post" }
                }
            ]));
        });

        let provider = make_provider(&server);
        let result = provider.get_queue().await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let queue = result.unwrap();
        assert_eq!(queue.len(), 2);
        assert_eq!(queue[0].post_id, "3221");
        assert_eq!(queue[0].platform, "mastodon");
        assert_eq!(queue[0].content_preview, "First scheduled post");
        assert_eq!(queue[1].post_id, "3222");
        mock.assert();
    }

    // 9.5.8 — engagement fields map correctly; impressions is None
    #[tokio::test]
    async fn test_get_engagement_maps_mastodon_fields() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/statuses/103704874086360371");
            then.status(200).json_body(serde_json::json!({
                "id": "103704874086360371",
                "favourites_count": 42,
                "reblogs_count": 12,
                "replies_count": 5
            }));
        });

        let provider = make_provider(&server);
        let result = provider.get_engagement("103704874086360371", "mastodon").await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let engagement = result.unwrap();
        assert_eq!(engagement.likes, 42, "favourites_count maps to likes");
        assert_eq!(engagement.reposts, 12, "reblogs_count maps to reposts");
        assert_eq!(engagement.replies, 5, "replies_count maps to replies");
        assert!(engagement.impressions.is_none(), "Mastodon has no impression count");
        mock.assert();
    }

    // 9.5.9 — test_connection returns Ok(()) on 200
    #[tokio::test]
    async fn test_test_connection_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/accounts/verify_credentials");
            then.status(200).json_body(serde_json::json!({
                "id": "14715",
                "display_name": "Alice",
                "acct": "alice"
            }));
        });

        let provider = make_provider(&server);
        let result = provider.test_connection().await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        mock.assert();
    }

    // 9.5.10 — test_connection returns AuthError on 401
    #[tokio::test]
    async fn test_test_connection_auth_error() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/accounts/verify_credentials");
            then.status(401).json_body(serde_json::json!({ "error": "The access token is invalid" }));
        });

        let provider = make_provider(&server);
        let result = provider.test_connection().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::AuthError(_) => {}
            other => panic!("Expected AuthError, got {:?}", other),
        }
        mock.assert();
    }

    // 9.5.11 — 429 with Retry-After header returns RateLimit with the correct duration
    #[tokio::test]
    async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/statuses");
            then.status(429)
                .header("Retry-After", "60")
                .body("Rate limit exceeded");
        });

        let provider = make_provider(&server);
        let result = provider.schedule_post("Test", "mastodon", None, None, None).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::RateLimit(duration) => {
                assert_eq!(duration.as_secs(), 60);
            }
            other => panic!("Expected RateLimit, got {:?}", other),
        }
        mock.assert();
    }

    // 9.5.12 — fetch_instance_char_limit reads max_characters; defaults to 500 on failure
    #[tokio::test]
    async fn test_instance_character_limit_fetch() {
        // Success case: returns configured limit
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/instance");
            then.status(200).json_body(serde_json::json!({
                "configuration": {
                    "statuses": { "max_characters": 2000 }
                }
            }));
        });
        let provider = make_provider(&server);
        let limit = provider.fetch_instance_char_limit().await;
        assert_eq!(limit, 2000, "Should return instance-configured limit");

        // Failure case: defaults to 500
        let server2 = MockServer::start();
        server2.mock(|when, then| {
            when.method(GET).path("/instance");
            then.status(500);
        });
        let provider2 = make_provider(&server2);
        let limit2 = provider2.fetch_instance_char_limit().await;
        assert_eq!(limit2, 500, "Should default to 500 on fetch failure");
    }

    // Issue 4 — Retry-After header capped at 3600 seconds
    #[tokio::test]
    async fn test_retry_after_bounded_to_max() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/statuses");
            then.status(429).header("Retry-After", "86400").body("Rate limit exceeded");
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Test", "mastodon", None, None, None).await;
        match result.unwrap_err() {
            ProviderError::RateLimit(d) => assert_eq!(d.as_secs(), 3600, "Retry-After must be capped at 3600s"),
            other => panic!("Expected RateLimit, got {:?}", other),
        }
    }

    // Issue 4 — missing Retry-After defaults to 60 seconds
    #[tokio::test]
    async fn test_retry_after_missing_defaults_to_60() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/statuses");
            then.status(429).body("Rate limit exceeded");
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Test", "mastodon", None, None, None).await;
        match result.unwrap_err() {
            ProviderError::RateLimit(d) => assert_eq!(d.as_secs(), 60, "Missing Retry-After should default to 60s"),
            other => panic!("Expected RateLimit, got {:?}", other),
        }
    }

    // Issue 6 — get_engagement returns platform_url from published post
    #[tokio::test]
    async fn test_get_engagement_returns_platform_url() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/statuses/103704874086360371");
            then.status(200).json_body(serde_json::json!({
                "id": "103704874086360371",
                "url": "https://mastodon.social/@alice/103704874086360371",
                "favourites_count": 5,
                "reblogs_count": 2,
                "replies_count": 1
            }));
        });
        let provider = make_provider(&server);
        let engagement = provider.get_engagement("103704874086360371", "mastodon").await.unwrap();
        assert_eq!(
            engagement.platform_url,
            Some("https://mastodon.social/@alice/103704874086360371".to_string()),
            "get_engagement must return the post URL so scheduled posts can recover it after publish"
        );
    }

    // Issue 7 — parse_link_next extracts the next URL from a Link header
    #[test]
    fn test_parse_link_next_extracts_url() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "link",
            "<https://mastodon.social/api/v1/scheduled_statuses?max_id=3221>; rel=\"next\""
                .parse()
                .unwrap(),
        );
        let result = parse_link_next(&headers);
        assert_eq!(
            result,
            Some("https://mastodon.social/api/v1/scheduled_statuses?max_id=3221".to_string())
        );
    }

    #[test]
    fn test_parse_link_next_returns_none_when_no_next() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "link",
            "<https://mastodon.social/api/v1/scheduled_statuses?min_id=1>; rel=\"prev\""
                .parse()
                .unwrap(),
        );
        assert_eq!(parse_link_next(&headers), None);
    }

    #[test]
    fn test_parse_link_next_returns_none_when_header_absent() {
        let headers = reqwest::header::HeaderMap::new();
        assert_eq!(parse_link_next(&headers), None);
    }

    // Issue 7 — get_queue follows pagination: second page uses a distinct path so mocks don't overlap
    #[tokio::test]
    async fn test_get_queue_fetches_all_pages() {
        let server = MockServer::start();
        let page2_path = "/scheduled_statuses/page2";

        server.mock(|when, then| {
            when.method(GET).path("/scheduled_statuses");
            then.status(200)
                .header("Link", &format!("<{}{}>; rel=\"next\"", server.base_url(), page2_path))
                .json_body(serde_json::json!([{
                    "id": "3221",
                    "scheduled_at": "2024-06-01T12:00:00.000Z",
                    "params": { "text": "Page 1 post" }
                }]));
        });

        server.mock(|when, then| {
            when.method(GET).path(page2_path);
            then.status(200)
                .json_body(serde_json::json!([{
                    "id": "3222",
                    "scheduled_at": "2024-06-02T12:00:00.000Z",
                    "params": { "text": "Page 2 post" }
                }]));
        });

        let provider = make_provider(&server);
        let queue = provider.get_queue().await.unwrap();
        assert_eq!(queue.len(), 2, "get_queue must follow pagination and return all posts");
        assert_eq!(queue[0].post_id, "3221");
        assert_eq!(queue[1].post_id, "3222");
    }

    // Issue 3 — create() factory validates SSRF before constructing provider
    #[tokio::test]
    async fn test_create_rejects_private_ip_instance() {
        let result = MastodonProvider::create("192.168.1.1", "token".to_string()).await;
        assert!(result.is_err(), "create() must reject private IP instances");
        match result.unwrap_err() {
            ProviderError::InvalidInstance(_) => {}
            other => panic!("Expected InvalidInstance, got {:?}", other),
        }
    }

    // 9.3.4 — SSRF: private IP instances are rejected before any HTTP request
    #[tokio::test]
    async fn test_rejects_private_ip_mastodon_instance() {
        let result = validate_instance_domain("192.168.1.1").await;
        assert!(result.is_err(), "Private IP should be rejected");
        match result.unwrap_err() {
            ProviderError::InvalidInstance(_) => {}
            other => panic!("Expected InvalidInstance, got {:?}", other),
        }
    }

    // Loopback should also be rejected
    #[tokio::test]
    async fn test_rejects_loopback_mastodon_instance() {
        let result = validate_instance_domain("127.0.0.1").await;
        assert!(result.is_err(), "Loopback should be rejected");
        match result.unwrap_err() {
            ProviderError::InvalidInstance(_) => {}
            other => panic!("Expected InvalidInstance, got {:?}", other),
        }
    }
}
