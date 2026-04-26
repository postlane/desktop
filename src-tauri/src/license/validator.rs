// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Validated user identity from the license backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// A single repo entry in the license response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoLicenseInfo {
    pub uuid: String,
    pub name: String,
    pub status: String,
}

/// The result of a license validation attempt
#[derive(Debug, Clone)]
pub enum LicenseState {
    /// Backend confirmed the token is valid
    Valid { user: UserInfo, repos: Vec<RepoLicenseInfo> },
    /// Backend returned 401 — token is expired or revoked
    Expired,
    /// Backend unreachable; cached state returned
    Offline { cached_at: DateTime<Utc> },
    /// No token configured
    Unconfigured,
}

/// Persisted license cache schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseCache {
    pub version: u32,
    pub validated_at: DateTime<Utc>,
    pub user: UserInfo,
    pub repos: Vec<RepoLicenseInfo>,
}

fn cache_path() -> Result<PathBuf, String> {
    Ok(postlane_dir()?.join("license_cache.json"))
}

fn read_license_cache() -> Option<LicenseCache> {
    let path = cache_path().ok()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str::<LicenseCache>(&content).ok().filter(|c| c.version == 1)
}

pub fn write_license_cache(cache: &LicenseCache) -> Result<(), String> {
    let path = cache_path()?;
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| format!("Failed to serialize license cache: {}", e))?;
    crate::init::atomic_write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write license cache: {}", e))
}

/// Returns true if the cache is older than 7 days (soft warning threshold).
pub fn is_cache_expired(cache: &LicenseCache) -> bool {
    cache.validated_at < Utc::now() - Duration::days(7)
}

#[derive(Deserialize)]
struct LicenseValidateResponse {
    user: UserInfo,
    repos: Vec<RepoLicenseInfo>,
}

/// Validates a license token against the backend.
/// On network failure, falls back to cached state.
/// Testable via `base_url` injection.
pub async fn validate_token_with_client(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<LicenseState, String> {
    if token.is_empty() {
        return Ok(LicenseState::Unconfigured);
    }
    let url = format!("{}/v1/license/validate", base_url);
    let resp = client.post(&url).bearer_auth(token).send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let body: LicenseValidateResponse = r.json().await
                .map_err(|e| format!("Invalid response: {}", e))?;
            let cache = LicenseCache {
                version: 1,
                validated_at: Utc::now(),
                user: body.user.clone(),
                repos: body.repos.clone(),
            };
            let _ = write_license_cache(&cache);
            Ok(LicenseState::Valid { user: body.user, repos: body.repos })
        }
        Ok(r) if r.status().as_u16() == 401 => Ok(LicenseState::Expired),
        _ => {
            if let Some(cache) = read_license_cache() {
                Ok(LicenseState::Offline { cached_at: cache.validated_at })
            } else {
                Ok(LicenseState::Unconfigured)
            }
        }
    }
}

/// Runs the 24-hour license revalidation loop.
/// `interval` is parameterised for test injection (use 100ms in tests, 24h in production).
/// Logs a warning when the token is expired; 503/network errors are silent (cache used).
pub async fn start_revalidation_loop(
    interval: std::time::Duration,
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> ! {
    let mut timer = tokio::time::interval(interval);
    timer.tick().await; // discard the immediate first tick
    loop {
        timer.tick().await;
        if let Ok(LicenseState::Expired) = validate_token_with_client(token, client, base_url).await {
            log::warn!("License expired — sign in again at postlane.dev/login");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;
    use std::sync::{Mutex, OnceLock};

    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn lock() -> &'static Mutex<()> { TEST_MUTEX.get_or_init(|| Mutex::new(())) }

    fn mock_valid_response() -> serde_json::Value {
        serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": "Alice", "avatar_url": null },
            "repos": [{ "uuid": "r1", "name": "my-repo", "status": "active" }]
        })
    }

    fn test_user() -> UserInfo {
        UserInfo { id: "u1".into(), display_name: "Alice".into(), avatar_url: None }
    }

    #[tokio::test]
    async fn test_validate_token_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(mock_valid_response());
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        assert!(matches!(state, LicenseState::Valid { .. }));
    }

    #[tokio::test]
    async fn test_validate_token_expired() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(401);
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        assert!(matches!(state, LicenseState::Expired), "401 should return Expired");
    }

    #[tokio::test]
    async fn test_validate_token_offline() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let cache = LicenseCache {
            version: 1,
            validated_at: Utc::now() - Duration::hours(1),
            user: test_user(),
            repos: vec![],
        };
        let json = serde_json::to_string_pretty(&cache).unwrap();
        std::fs::write(&path, json).unwrap();
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(503);
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        assert!(matches!(state, LicenseState::Offline { .. }), "503 should return Offline");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_license_cache_expired() {
        let cache = LicenseCache {
            version: 1,
            validated_at: Utc::now() - Duration::days(8),
            user: test_user(),
            repos: vec![],
        };
        assert!(is_cache_expired(&cache), "8-day-old cache should be expired");
        let fresh = LicenseCache { validated_at: Utc::now() - Duration::days(6), ..cache };
        assert!(!is_cache_expired(&fresh), "6-day-old cache should not be expired");
    }

    #[tokio::test]
    async fn test_validate_token_empty_returns_unconfigured() {
        let client = build_client();
        let state = validate_token_with_client("", &client, "https://unused.example.com").await.unwrap();
        assert!(matches!(state, LicenseState::Unconfigured));
    }

    /// Confirms the revalidation interval fires validate_token at the configured cadence.
    /// Uses a 100ms interval and waits 350ms — expects at least 2 calls to the backend.
    #[tokio::test]
    async fn test_24hr_revalidation_interval() {
        let server = MockServer::start();
        let mock_handle = server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(mock_valid_response());
        });

        let client = build_client();
        let base_url = server.base_url();

        let handle = tauri::async_runtime::spawn({
            let client = client.clone();
            let base_url = base_url.clone();
            async move {
                start_revalidation_loop(
                    std::time::Duration::from_millis(100),
                    "test-token",
                    &client,
                    &base_url,
                )
                .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        handle.abort();

        assert!(
            mock_handle.hits() >= 2,
            "expected at least 2 validate calls in 350ms at 100ms interval, got {}",
            mock_handle.hits()
        );
    }
}
