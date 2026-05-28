// SPDX-License-Identifier: BUSL-1.1

pub mod ayrshare;
pub mod buffer;
pub mod http_client;
pub mod mastodon;
pub mod outstand;
pub mod publer;
pub mod substack_notes;
pub mod upload_post;
pub mod webhook;
pub mod zernio;

pub use http_client::{build_client, parse_retry_after, with_retry};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

/// Error types for scheduling provider operations
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    /// HTTP error from scheduler API (4xx/5xx)
    HttpError { status: u16, body: String },
    /// Rate limit hit (429 response)
    /// Duration indicates how long to wait before retry (from Retry-After header)
    RateLimit(Duration),
    /// Network error (connection refused, timeout, DNS failure)
    NetworkError(String),
    /// Authentication error (invalid API key)
    AuthError(String),
    /// Operation not supported by this provider
    NotSupported(String),
    /// Instance domain is invalid (e.g. resolves to a private IP)
    InvalidInstance(String),
    /// Unknown error
    Unknown(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::HttpError { status, body } => {
                write!(f, "HTTP error {}: {}", status, body)
            }
            ProviderError::RateLimit(duration) => {
                write!(f, "Rate limit hit, retry after {:?}", duration)
            }
            ProviderError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ProviderError::AuthError(msg) => write!(f, "Authentication error: {}", msg),
            ProviderError::NotSupported(msg) => write!(f, "Operation not supported: {}", msg),
            ProviderError::InvalidInstance(msg) => write!(f, "Invalid instance: {}", msg),
            ProviderError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    /// Returns true for transient errors that are safe to retry.
    /// Unknown errors are NOT retried — the request likely already succeeded
    /// on the provider's side, so retrying would create duplicate posts.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ProviderError::NetworkError(_) | ProviderError::RateLimit(_)
        ) || matches!(self, ProviderError::HttpError { status, .. } if *status >= 500)
    }
}

/// Scheduler profile (social media account connected to the scheduler)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SchedulerProfile {
    pub id: String,
    pub name: String,
    pub platforms: Vec<String>,
}

/// Engagement metrics for a post
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Engagement {
    pub likes: u64,
    pub reposts: u64,
    pub replies: u64,
    pub impressions: Option<u64>,
    /// Public URL of the post, if returned by the provider.
    /// Populated by Mastodon's get_engagement so scheduled posts can recover
    /// their URL after they publish (it is None at schedule time).
    #[serde(default)]
    pub platform_url: Option<String>,
}

/// Result from scheduling a post
#[derive(Debug, Clone)]
pub struct PostScheduleResult {
    /// Scheduler-internal post ID (stored for cancellation / engagement lookup)
    pub scheduler_id: String,
    /// Public URL of the post on the platform, if the provider returned it
    pub platform_url: Option<String>,
}

/// Trait for scheduling provider implementations
/// All methods are async and the trait requires Send + Sync for thread safety
#[async_trait]
pub trait SchedulingProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Schedule a post
    /// Returns the scheduler ID and optional platform URL (available immediately for publishNow posts)
    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError>;

    /// List available profiles (social accounts)
    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError>;

    /// Cancel a scheduled post
    async fn cancel_post(&self, post_id: &str, platform: &str) -> Result<(), ProviderError>;

    /// Get the queue of scheduled posts
    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError>;

    /// Test connection with the provider.
    /// Default implementation calls list_profiles(); providers with a lighter
    /// test endpoint should override this.
    async fn test_connection(&self) -> Result<(), ProviderError> {
        self.list_profiles().await?;
        Ok(())
    }

    /// Get engagement metrics for a post
    async fn get_engagement(
        &self,
        post_id: &str,
        platform: &str,
    ) -> Result<Engagement, ProviderError>;

    /// Get the public URL for a post on a platform
    /// Returns None if the URL cannot be determined
    fn post_url(&self, platform: &str, post_id: &str) -> Option<String>;
}

/// Builds a scheduling provider from its name and API key.
pub fn build_scheduling_provider(
    name: &str,
    api_key: String,
) -> Result<Box<dyn SchedulingProvider>, String> {
    Ok(match name {
        "zernio" => Box::new(zernio::ZernioProvider::new(api_key)),
        "buffer" => Box::new(buffer::BufferProvider::new(api_key)),
        "ayrshare" => Box::new(ayrshare::AyrshareProvider::new(api_key)),
        "publer" => Box::new(publer::PublerProvider::new(api_key)),
        "outstand" => Box::new(outstand::OutstandProvider::new(api_key)),
        "substack_notes" => Box::new(substack_notes::SubstackNotesProvider::new(api_key)),
        "upload_post" => Box::new(upload_post::UploadPostProvider::new(api_key)),
        "webhook" => Box::new(webhook::WebhookProvider::new(api_key)),
        other => return Err(format!("Unknown scheduler provider: {}", other)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_scheduling_provider ─────────────────────────────────────────────

    #[test]
    fn test_build_scheduling_provider_unknown_returns_error() {
        let err = build_scheduling_provider("nonexistent-provider", "key".to_string())
            .err()
            .expect("should return error for unknown provider");
        assert!(err.contains("Unknown scheduler provider"), "Error: {}", err);
    }

    #[test]
    fn test_build_scheduling_provider_known_succeeds() {
        assert!(build_scheduling_provider("zernio", "test-key".to_string()).is_ok());
    }

    // build_scheduling_provider — all 7 non-zernio providers must construct without error
    #[test]
    fn test_build_scheduling_provider_all_known_providers_succeed() {
        let providers = [
            "buffer", "ayrshare", "publer", "outstand",
            "substack_notes", "upload_post", "webhook",
        ];
        for name in providers {
            assert!(
                build_scheduling_provider(name, "test-key".to_string()).is_ok(),
                "build_scheduling_provider must succeed for '{}'", name
            );
        }
    }

    // ── ProviderError Display ────────────────────────────────────────────────

    #[test]
    fn test_provider_error_display_all_variants() {
        use std::time::Duration;
        let cases: Vec<(&str, ProviderError)> = vec![
            ("404", ProviderError::HttpError { status: 404, body: "not found".into() }),
            ("Rate limit", ProviderError::RateLimit(Duration::from_secs(30))),
            ("timeout", ProviderError::NetworkError("timeout".into())),
            ("bad key", ProviderError::AuthError("bad key".into())),
            ("no cancel", ProviderError::NotSupported("no cancel".into())),
            ("private ip", ProviderError::InvalidInstance("private ip".into())),
            ("weird", ProviderError::Unknown("weird".into())),
        ];
        for (needle, err) in cases {
            let msg = format!("{}", err);
            assert!(msg.contains(needle), "Display for {:?} must contain '{}', got '{}'", err, needle, msg);
        }
    }

    // ── ProviderError::is_retryable ───────────────────────────────────────────

    // NetworkError and RateLimit are retryable
    #[test]
    fn test_is_retryable_returns_true_for_network_error() {
        assert!(ProviderError::NetworkError("timeout".into()).is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_true_for_rate_limit() {
        assert!(ProviderError::RateLimit(Duration::from_secs(60)).is_retryable());
    }

    // HttpError >= 500 is retryable; < 500 is not
    #[test]
    fn test_is_retryable_returns_true_for_http_500() {
        assert!(ProviderError::HttpError { status: 500, body: String::new() }.is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_true_for_http_503() {
        assert!(ProviderError::HttpError { status: 503, body: String::new() }.is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_false_for_http_400() {
        assert!(!ProviderError::HttpError { status: 400, body: String::new() }.is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_false_for_http_422() {
        assert!(!ProviderError::HttpError { status: 422, body: String::new() }.is_retryable());
    }

    // Non-transient errors are never retried
    #[test]
    fn test_is_retryable_returns_false_for_auth_error() {
        assert!(!ProviderError::AuthError("bad key".into()).is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_false_for_not_supported() {
        assert!(!ProviderError::NotSupported("not supported".into()).is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_false_for_invalid_instance() {
        assert!(!ProviderError::InvalidInstance("private ip".into()).is_retryable());
    }

    #[test]
    fn test_is_retryable_returns_false_for_unknown() {
        assert!(!ProviderError::Unknown("unexpected".into()).is_retryable());
    }

    // A provider that only overrides list_profiles — test_connection should use the default.
    struct MinimalProvider {
        profiles: Vec<SchedulerProfile>,
    }

    #[async_trait]
    impl SchedulingProvider for MinimalProvider {
        fn name(&self) -> &str { "minimal" }
        async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
            Ok(self.profiles.clone())
        }
        async fn schedule_post(&self, _: &str, _: &str, _: Option<chrono::DateTime<chrono::Utc>>, _: Option<&str>, _: Option<&str>) -> Result<PostScheduleResult, ProviderError> {
            Err(ProviderError::NotSupported("not needed".into()))
        }
        async fn cancel_post(&self, _: &str, _: &str) -> Result<(), ProviderError> {
            Err(ProviderError::NotSupported("not needed".into()))
        }
        async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
            Ok(vec![])
        }
        async fn get_engagement(&self, _: &str, _: &str) -> Result<Engagement, ProviderError> {
            Err(ProviderError::NotSupported("not needed".into()))
        }
        fn post_url(&self, _: &str, _: &str) -> Option<String> { None }
    }

    #[tokio::test]
    async fn default_test_connection_delegates_to_list_profiles() {
        let provider = MinimalProvider { profiles: vec![] };
        // If SchedulingProvider has a default test_connection this compiles and passes.
        // Without the default the trait requires an explicit impl → compile error.
        let result = provider.test_connection().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn default_test_connection_surfaces_list_profiles_error() {
        struct FailingProvider;
        #[async_trait]
        impl SchedulingProvider for FailingProvider {
            fn name(&self) -> &str { "failing" }
            async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
                Err(ProviderError::AuthError("bad key".into()))
            }
            async fn schedule_post(&self, _: &str, _: &str, _: Option<chrono::DateTime<chrono::Utc>>, _: Option<&str>, _: Option<&str>) -> Result<PostScheduleResult, ProviderError> {
                Err(ProviderError::NotSupported("".into()))
            }
            async fn cancel_post(&self, _: &str, _: &str) -> Result<(), ProviderError> {
                Err(ProviderError::NotSupported("".into()))
            }
            async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> { Ok(vec![]) }
            async fn get_engagement(&self, _: &str, _: &str) -> Result<Engagement, ProviderError> {
                Err(ProviderError::NotSupported("".into()))
            }
            fn post_url(&self, _: &str, _: &str) -> Option<String> { None }
        }
        let result = FailingProvider.test_connection().await;
        assert!(matches!(result, Err(ProviderError::AuthError(_))));
    }

}
