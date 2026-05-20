// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, parse_retry_after, Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Generic webhook provider — covers Zapier, Make (Integromat), and any webhook-capable tool.
///
/// Postlane POSTs a standardised JSON payload to the user-supplied URL.
/// The webhook URL must use `https://` (Security Rule 4).
pub struct WebhookProvider {
    client: reqwest::Client,
    webhook_url: String,
}

impl WebhookProvider {
    /// Create a new WebhookProvider. `webhook_url` is the credential stored in keyring.
    pub fn new(webhook_url: String) -> Self {
        Self {
            client: build_client(),
            webhook_url,
        }
    }

    /// Validate the webhook URL: must use `https://` and must not target private/loopback
    /// addresses (Security Rules 4 and 5). Loopback is allowed in test builds only.
    fn validate_url(url: &str) -> Result<(), ProviderError> {
        #[cfg(test)]
        if url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost") {
            return Ok(());
        }
        if !url.starts_with("https://") {
            return Err(ProviderError::InvalidInstance(
                "Webhook URL must use https://".to_string(),
            ));
        }
        let parsed = url::Url::parse(url)
            .map_err(|_| ProviderError::InvalidInstance("Webhook URL is not a valid URL.".to_string()))?;
        let host = parsed.host_str().unwrap_or("");
        if crate::security::ssrf_check::is_private_host_str(host) {
            return Err(ProviderError::InvalidInstance(
                "Webhook URL must not target a private or loopback address.".to_string(),
            ));
        }
        Ok(())
    }

    /// Check HTTP status and return appropriate `ProviderError`.
    fn check_response_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        let status = response.status();
        if status == 429 {
            return Err(ProviderError::RateLimit(parse_retry_after(response)));
        }
        Ok(())
    }
}

#[async_trait]
impl SchedulingProvider for WebhookProvider {
    fn name(&self) -> &str {
        "webhook"
    }

    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        Self::validate_url(&self.webhook_url)?;
        let payload = serde_json::json!({
            "content": content,
            "platform": platform,
            "profile_id": profile_id,
            "scheduled_at": scheduled_for.map(|dt| dt.to_rfc3339()),
        });
        let response = self.client.post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_response_status(&response)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        Ok(PostScheduleResult { scheduler_id: local_post_id(), platform_url: None })
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        Ok(vec![])
    }

    async fn cancel_post(&self, _post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        Err(ProviderError::NotSupported(
            "Webhook tools have no cancel mechanism. Remove the post directly in your automation platform.".to_string(),
        ))
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        Ok(vec![])
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        Self::validate_url(&self.webhook_url)?;
        let payload = serde_json::json!({ "test": true, "source": "postlane" });
        let response = self.client.post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        if !response.status().is_success() {
            return Err(ProviderError::AuthError(
                format!("Webhook returned {} — verify the URL is correct and the endpoint accepts POST requests.", response.status().as_u16()),
            ));
        }
        Ok(())
    }

    async fn get_engagement(&self, _post_id: &str, _platform: &str) -> Result<Engagement, ProviderError> {
        Err(ProviderError::NotSupported(
            "Engagement data is not available via webhook.".to_string(),
        ))
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        None
    }
}

/// Generate a local timestamp-based ID for webhook posts.
/// Webhook tools do not return a post ID, so we generate one locally for tracking.
fn local_post_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("wh-{:x}", t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_schedule_post_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(200);
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().scheduler_id.starts_with("wh-"));
        mock.assert();
    }

    #[tokio::test]
    async fn test_schedule_post_rejects_http_url() {
        let provider = WebhookProvider::new("http://insecure.example.com/hook".to_string());
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        assert!(matches!(result, Err(ProviderError::InvalidInstance(_))), "{:?}", result);
    }

    #[tokio::test]
    async fn test_schedule_post_rejects_private_ip_ssrf() {
        for url in &[
            "https://169.254.169.254/latest/meta-data/",  // AWS metadata
            "https://10.0.0.1/internal",
            "https://192.168.1.1/admin",
            "https://172.16.0.1/secret",
        ] {
            let provider = WebhookProvider::new(url.to_string());
            let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
            assert!(
                matches!(result, Err(ProviderError::InvalidInstance(_))),
                "expected SSRF rejection for {}, got {:?}", url, result,
            );
        }
    }

    #[tokio::test]
    async fn test_schedule_post_429_returns_rate_limit_error_with_retry_after() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(429).header("Retry-After", "45");
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        match result {
            Err(ProviderError::RateLimit(d)) => assert_eq!(d.as_secs(), 45),
            other => panic!("expected RateLimit, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cancel_post_not_supported() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        let result = provider.cancel_post("any-id", "linkedin").await;
        assert!(matches!(result, Err(ProviderError::NotSupported(_))), "{:?}", result);
    }

    #[tokio::test]
    async fn test_test_connection_non_2xx() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(404);
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let result = provider.test_connection().await;
        assert!(matches!(result, Err(ProviderError::AuthError(_))), "{:?}", result);
    }

    #[test]
    fn test_name_returns_webhook() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        assert_eq!(provider.name(), "webhook");
    }

    #[tokio::test]
    async fn test_schedule_post_non_2xx_returns_http_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(500).body("error");
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let result = provider.schedule_post("Hello", "linkedin", None, None, None).await;
        match result {
            Err(ProviderError::HttpError { status, .. }) => assert_eq!(status, 500),
            other => panic!("expected HttpError(500), got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_list_profiles_returns_empty() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        let result = provider.list_profiles().await;
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty(), "webhook has no profiles");
    }

    #[tokio::test]
    async fn test_get_queue_returns_empty() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        let result = provider.get_queue().await;
        assert!(result.is_ok(), "{:?}", result);
        assert!(result.unwrap().is_empty(), "webhook has no queue");
    }

    #[tokio::test]
    async fn test_get_engagement_not_supported() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        let result = provider.get_engagement("any-id", "linkedin").await;
        assert!(matches!(result, Err(ProviderError::NotSupported(_))), "{:?}", result);
    }

    #[test]
    fn test_post_url_returns_none() {
        let provider = WebhookProvider::new("https://hooks.zapier.com/x".to_string());
        assert_eq!(provider.post_url("linkedin", "post-id"), None);
    }

    #[tokio::test]
    async fn test_test_connection_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(200);
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let result = provider.test_connection().await;
        assert!(result.is_ok(), "{:?}", result);
    }

    #[tokio::test]
    async fn test_local_post_id_is_unique() {
        let id1 = super::local_post_id();
        let id2 = super::local_post_id();
        assert!(id1.starts_with("wh-"), "id must start with wh-");
        assert_ne!(id1, id2, "successive ids must differ");
    }

    #[tokio::test]
    async fn test_schedule_post_with_scheduled_at_and_profile_id() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/hook");
            then.status(200);
        });
        let url = format!("{}/hook", server.base_url());
        let provider = WebhookProvider::new(url);
        let dt = chrono::DateTime::parse_from_rfc3339("2026-06-01T10:00:00Z").unwrap().with_timezone(&chrono::Utc);
        let result = provider.schedule_post("Hello", "linkedin", Some(dt), None, Some("profile-1")).await;
        assert!(result.is_ok(), "{:?}", result);
    }
}
