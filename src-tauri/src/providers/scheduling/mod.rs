// SPDX-License-Identifier: BUSL-1.1

pub mod ayrshare;
pub mod buffer;
pub mod zernio;

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
    NotSupported,
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
            ProviderError::NotSupported => write!(f, "Operation not supported"),
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
}

/// Trait for scheduling provider implementations
/// All methods are async and the trait requires Send + Sync for thread safety
#[async_trait]
pub trait SchedulingProvider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;

    /// Schedule a post
    /// Returns the scheduler-assigned post ID
    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<String, ProviderError>;

    /// List available profiles (social accounts)
    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError>;

    /// Cancel a scheduled post
    async fn cancel_post(&self, post_id: &str, platform: &str) -> Result<(), ProviderError>;

    /// Get the queue of scheduled posts
    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError>;

    /// Test connection with the provider
    async fn test_connection(&self) -> Result<(), ProviderError>;

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

/// Build a shared reqwest::Client for provider HTTP requests
///
/// Creates a client with:
/// - 10 second request timeout
/// - 5 second connect timeout
///
/// Panics if client cannot be built (configuration error at startup)
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build reqwest client")
}

/// Retry a function with exponential backoff
///
/// Retries up to max_retries times with exponential backoff:
/// - Initial delay: 1 second
/// - Doubles each attempt (1s → 2s → 4s)
/// - On RateLimit error: uses the duration from Retry-After header instead
/// - On final failure: returns the last error
pub async fn with_retry<F, Fut, T>(
    f: F,
    max_retries: u32,
) -> Result<T, ProviderError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let mut attempt = 0;

    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if !e.is_retryable() {
                    return Err(e);
                }

                attempt += 1;

                if attempt > max_retries {
                    return Err(e);
                }

                let wait_duration = match &e {
                    ProviderError::RateLimit(duration) => *duration,
                    _ => Duration::from_secs(2u64.pow(attempt - 1)),
                };

                tokio::time::sleep(wait_duration).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_build_client_creates_client_with_timeouts() {
        // Test: build_client should create a reqwest::Client
        // with 10s request timeout and 5s connect timeout
        let client = build_client();

        // Verify client was created (if this compiles and doesn't panic, it worked)
        // The actual timeout values are internal to reqwest::Client and can't be
        // easily inspected, but we can verify the function doesn't panic
        assert!(std::any::type_name_of_val(&client).contains("Client"));
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_on_first_attempt() {
        let result = with_retry(|| async { Ok::<i32, ProviderError>(42) }, 3).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_with_retry_succeeds_after_failures() {
        let attempt = Arc::new(AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let result = with_retry(
            || {
                let attempt = attempt_clone.clone();
                async move {
                    let current = attempt.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err(ProviderError::NetworkError("Temporary failure".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            },
            3,
        )
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_with_retry_returns_last_error_after_max_retries() {
        let attempt = Arc::new(AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let result = with_retry(
            || {
                let attempt = attempt_clone.clone();
                async move {
                    attempt.fetch_add(1, Ordering::SeqCst);
                    Err::<i32, ProviderError>(ProviderError::NetworkError(
                        "Persistent failure".to_string(),
                    ))
                }
            },
            3,
        )
        .await;

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ProviderError::NetworkError("Persistent failure".to_string())
        );
        // Should try: initial attempt + 3 retries = 4 total
        assert_eq!(attempt.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn test_with_retry_uses_rate_limit_duration() {
        use std::time::Instant;

        let attempt = Arc::new(AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let start = Instant::now();
        let result = with_retry(
            || {
                let attempt = attempt_clone.clone();
                async move {
                    let current = attempt.fetch_add(1, Ordering::SeqCst);
                    if current == 0 {
                        // First attempt fails with rate limit - wait 1 second
                        Err(ProviderError::RateLimit(Duration::from_secs(1)))
                    } else {
                        Ok(42)
                    }
                }
            },
            3,
        )
        .await;

        let elapsed = start.elapsed();

        assert_eq!(result.unwrap(), 42);
        // Should have waited ~1 second (allow some tolerance)
        assert!(elapsed >= Duration::from_millis(900));
        assert!(elapsed < Duration::from_millis(1500));
    }

    #[tokio::test]
    async fn test_with_retry_exponential_backoff() {
        use std::time::Instant;

        let attempt = Arc::new(AtomicU32::new(0));
        let attempt_clone = attempt.clone();

        let start = Instant::now();
        let result = with_retry(
            || {
                let attempt = attempt_clone.clone();
                async move {
                    let current = attempt.fetch_add(1, Ordering::SeqCst);
                    if current < 2 {
                        Err(ProviderError::NetworkError("Temporary".to_string()))
                    } else {
                        Ok(42)
                    }
                }
            },
            3,
        )
        .await;

        let elapsed = start.elapsed();

        assert_eq!(result.unwrap(), 42);
        // Should have waited 1s + 2s = 3s total (allow tolerance)
        assert!(elapsed >= Duration::from_millis(2800));
        assert!(elapsed < Duration::from_millis(3500));
    }
}
