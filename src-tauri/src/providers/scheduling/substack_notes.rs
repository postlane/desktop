// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Mutex;

/// Substack Notes provider using the reverse-engineered internal Substack API.
///
/// Auth: `connect.sid` browser session cookie.
/// **Known limitations:**
/// - Notes always post immediately; `scheduled_at` is ignored.
/// - Notes cannot be deleted via the API.
/// - This API is unofficial and may break without warning.
pub struct SubstackNotesProvider {
    client: reqwest::Client,
    cookie: String,
    /// Cached Substack username — populated on first `list_profiles` call.
    username: Mutex<Option<String>>,
    #[cfg(test)]
    base_url: String,
}

impl SubstackNotesProvider {
    /// Create a new SubstackNotesProvider.
    pub fn new(cookie: String) -> Self {
        Self {
            client: build_client(),
            cookie,
            username: Mutex::new(None),
            #[cfg(test)]
            base_url: "https://substack.com".to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        "https://substack.com"
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Build the Cookie header value, stripping control characters to prevent header injection.
    fn cookie_header(&self) -> String {
        let safe: String = self.cookie.chars().filter(|c| !c.is_ascii_control()).collect();
        format!("connect.sid={}", safe)
    }

    /// Check HTTP status and return appropriate `ProviderError`.
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();
        if status == 401 || status == 403 {
            return Err(ProviderError::AuthError(
                "Substack session expired. Re-enter your connect.sid cookie in Settings.".to_string(),
            ));
        }
        if status == 429 {
            return Err(ProviderError::RateLimit(parse_retry_after(response)));
        }
        Ok(())
    }

    /// Ensure username is cached; fetch from profile if not yet known.
    async fn ensure_username_cached(&self) -> Result<(), ProviderError> {
        {
            let guard = self.username.lock()
                .map_err(|_| ProviderError::Unknown("username mutex poisoned".to_string()))?;
            if guard.is_some() {
                return Ok(());
            }
        }
        self.list_profiles().await?;
        Ok(())
    }

    /// Read cached username, returning `None` if not yet populated.
    fn cached_username(&self) -> Option<String> {
        self.username.lock().ok()?.clone()
    }
}

#[async_trait]
impl SchedulingProvider for SubstackNotesProvider {
    fn name(&self) -> &str {
        "substack_notes"
    }

    /// Post a Substack Note immediately.
    /// `scheduled_for` is intentionally ignored — Substack Notes do not support scheduling.
    async fn schedule_post(
        &self,
        content: &str,
        _platform: &str,
        _scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        _profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        self.ensure_username_cached().await?;
        let url = format!("{}/api/v1/comment/feed", self.base_url());
        let body = serde_json::json!({ "body": content, "type": "publication" });
        let response = self.client.post(&url)
            .header("Cookie", self.cookie_header())
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
        let post_id = json["id"].as_str()
            .ok_or_else(|| ProviderError::Unknown(format!("Missing id in response: {}", json)))?
            .to_string();
        let platform_url = self.post_url("substack", &post_id);
        Ok(PostScheduleResult { scheduler_id: post_id, platform_url })
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/api/v1/profile", self.base_url());
        let response = self.client.get(&url)
            .header("Cookie", self.cookie_header())
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
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse profile: {}", e)))?;
        let username = json["handle"].as_str()
            .or_else(|| json["username"].as_str())
            .unwrap_or("")
            .to_string();
        let name = json["name"].as_str()
            .or_else(|| json["publicationName"].as_str())
            .unwrap_or(&username)
            .to_string();
        let id = json["id"].as_str().unwrap_or("substack").to_string();
        if !username.is_empty() {
            if let Ok(mut guard) = self.username.lock() {
                *guard = Some(username.clone());
            }
        }
        Ok(vec![SchedulerProfile { id, name, platforms: vec!["substack".to_string()] }])
    }

    async fn cancel_post(&self, _post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        Err(ProviderError::NotSupported(
            "Substack Notes cannot be deleted via the API. Delete the note manually at substack.com.".to_string(),
        ))
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        Ok(vec![])
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        self.list_profiles().await?;
        Ok(())
    }

    async fn get_engagement(&self, post_id: &str, _platform: &str) -> Result<Engagement, ProviderError> {
        let url = format!("{}/api/v1/comment/{}", self.base_url(), post_id);
        let response = self.client.get(&url)
            .header("Cookie", self.cookie_header())
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
        Ok(Engagement {
            likes: json["reactions_count"].as_u64().unwrap_or(0),
            reposts: 0,
            replies: json["children_count"].as_u64().unwrap_or(0),
            impressions: None,
            platform_url: None,
        })
    }

    fn post_url(&self, _platform: &str, post_id: &str) -> Option<String> {
        let username = self.cached_username()?;
        Some(format!("https://substack.com/@{}/note/{}", username, post_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_provider(server: &MockServer) -> SubstackNotesProvider {
        let mut p = SubstackNotesProvider::new("sess-cookie-abc".to_string());
        p.base_url = server.base_url();
        p
    }

    fn profile_mock(server: &MockServer) {
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/profile");
            then.status(200).json_body(serde_json::json!({
                "id": "user-1", "handle": "myhandle", "name": "My Publication"
            }));
        });
    }

    #[tokio::test]
    async fn test_schedule_post_success() {
        let server = MockServer::start();
        profile_mock(&server);
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/comment/feed");
            then.status(200).json_body(serde_json::json!({ "id": "note_abc123" }));
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello Substack", "substack", None, None, None).await;
        assert!(result.is_ok(), "{:?}", result);
        let res = result.unwrap();
        assert_eq!(res.scheduler_id, "note_abc123");
        assert_eq!(res.platform_url, Some("https://substack.com/@myhandle/note/note_abc123".to_string()));
    }

    #[tokio::test]
    async fn test_schedule_post_unauthorised() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/profile");
            then.status(401);
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello", "substack", None, None, None).await;
        assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
    }

    #[tokio::test]
    async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
        let server = MockServer::start();
        profile_mock(&server);
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/comment/feed");
            then.status(429).header("Retry-After", "30");
        });
        let provider = make_provider(&server);
        let result = provider.schedule_post("Hello", "substack", None, None, None).await;
        match result {
            Err(ProviderError::RateLimit(d)) => assert_eq!(d.as_secs(), 30),
            other => panic!("expected RateLimit, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cancel_post_not_supported() {
        let provider = SubstackNotesProvider::new("cookie".to_string());
        let result = provider.cancel_post("note-1", "substack").await;
        match result {
            Err(ProviderError::NotSupported(msg)) => {
                assert!(msg.contains("substack.com"), "message: {}", msg);
            }
            other => panic!("expected NotSupported, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_engagement_partial() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/comment/note-1");
            then.status(200).json_body(serde_json::json!({
                "reactions_count": 7, "children_count": null
            }));
        });
        let provider = make_provider(&server);
        let result = provider.get_engagement("note-1", "substack").await;
        assert!(result.is_ok(), "{:?}", result);
        let eng = result.unwrap();
        assert_eq!(eng.likes, 7);
        assert_eq!(eng.replies, 0);
        assert_eq!(eng.impressions, None);
    }

    #[tokio::test]
    async fn test_get_queue_returns_empty() {
        let provider = SubstackNotesProvider::new("cookie".to_string());
        let result = provider.get_queue().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_post_url_uses_cached_username() {
        let provider = SubstackNotesProvider::new("cookie".to_string());
        *provider.username.lock().unwrap() = Some("myhandle".to_string());
        assert_eq!(
            provider.post_url("substack", "note-123"),
            Some("https://substack.com/@myhandle/note/note-123".to_string())
        );
    }

    #[test]
    fn test_post_url_returns_none_when_username_unknown() {
        let provider = SubstackNotesProvider::new("cookie".to_string());
        assert_eq!(provider.post_url("substack", "note-123"), None);
    }

    #[test]
    fn test_cookie_header_strips_control_characters() {
        let provider = SubstackNotesProvider::new("valid\r\nX-Injected: evil".to_string());
        let header = provider.cookie_header();
        assert!(!header.contains('\r'), "CR must be stripped");
        assert!(!header.contains('\n'), "LF must be stripped");
        assert_eq!(header, "connect.sid=validX-Injected: evil");
    }
}
