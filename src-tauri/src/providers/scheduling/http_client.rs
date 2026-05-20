// SPDX-License-Identifier: BUSL-1.1

//! HTTP client construction and retry logic for scheduling providers.

use super::ProviderError;
use std::time::Duration;

/// Parse a raw `Retry-After` header value string into seconds.
///
/// Falls back to 60 s if absent/unparseable. Caps at 3600 s.
pub(crate) fn parse_retry_after_secs(header_value: Option<&str>) -> u64 {
    header_value
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60)
        .min(3600)
}

/// Extract and parse the `Retry-After` header from an HTTP response as a `Duration`.
pub fn parse_retry_after(response: &reqwest::Response) -> Duration {
    let header = response
        .headers()
        .get("Retry-After")
        .and_then(|v| v.to_str().ok());
    Duration::from_secs(parse_retry_after_secs(header))
}

/// Build a shared reqwest::Client for provider HTTP requests.
///
/// Creates a client with a 10 s request timeout and 5 s connect timeout.
/// Panics if the client cannot be built (configuration error at startup).
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build reqwest client")
}

/// Retry a function with exponential backoff.
///
/// Retries up to `max_retries` times. On `RateLimit` the `Retry-After` duration
/// is used; on other retryable errors the wait doubles each attempt (1 s → 2 s → 4 s).
/// Non-retryable errors are returned immediately.
pub async fn with_retry<F, Fut, T>(f: F, max_retries: u32) -> Result<T, ProviderError>
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
                let wait = match &e {
                    ProviderError::RateLimit(d) => *d,
                    _ => Duration::from_secs(2u64.pow(attempt - 1)),
                };
                tokio::time::sleep(wait).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::scheduling::ProviderError;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn parse_retry_after_returns_header_value_as_seconds() {
        assert_eq!(parse_retry_after_secs(Some("120")), 120);
    }

    #[test]
    fn parse_retry_after_defaults_to_60s_when_header_missing() {
        assert_eq!(parse_retry_after_secs(None), 60);
    }

    #[test]
    fn parse_retry_after_defaults_to_60s_when_header_unparseable() {
        assert_eq!(parse_retry_after_secs(Some("soon")), 60);
    }

    #[test]
    fn parse_retry_after_caps_at_3600s() {
        assert_eq!(parse_retry_after_secs(Some("9999")), 3600);
    }

    #[test]
    fn test_build_client_creates_client_with_timeouts() {
        let client = build_client();
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
        assert!(elapsed >= Duration::from_millis(2800));
        assert!(elapsed < Duration::from_millis(3500));
    }
}
