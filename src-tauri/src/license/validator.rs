// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Validated user identity from the license backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub display_name: Option<String>,
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

fn warn_on_cache_write(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("[license] failed to write cache: {}", e);
    }
}

/// Returns true if the cache is older than 7 days (soft warning threshold).
pub fn is_cache_expired(cache: &LicenseCache) -> bool {
    cache.validated_at < Utc::now() - Duration::days(7)
}


#[derive(Deserialize)]
struct LicenseValidateResponse {
    user: UserInfo,
    #[serde(default)]
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
            warn_on_cache_write(write_license_cache(&cache));
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

/// Validates a license token, treating a >30-day-old offline cache as `Expired`.
/// Use this in background revalidation loops. Do NOT use in `handle_activate` —
/// the user's activation attempt should never be blocked by a stale cache.
pub async fn validate_token_enforcing_expiry(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<LicenseState, String> {
    let state = validate_token_with_client(token, client, base_url).await?;
    if let LicenseState::Offline { cached_at } = &state {
        if *cached_at < Utc::now() - Duration::days(30) {
            log::warn!("License cache is more than 30 days old — treating as expired");
            return Ok(LicenseState::Expired);
        }
    }
    Ok(state)
}

/// Runs the 24-hour license revalidation loop.
/// `interval` is parameterised for test injection (use 100ms in tests, 24h in production).
/// Calls `on_expired` when the backend returns 401; 503/network errors are silent (cache used).
pub async fn start_revalidation_loop(
    interval: std::time::Duration,
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    on_expired: impl Fn() + Send + 'static,
) -> ! {
    let mut timer = tokio::time::interval(interval);
    timer.tick().await; // discard the immediate first tick
    loop {
        timer.tick().await;
        if let Ok(LicenseState::Expired) = validate_token_enforcing_expiry(token, client, base_url).await {
            log::warn!("License expired — sign in again at postlane.dev/login");
            on_expired();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;
    use std::sync::{Arc, Mutex, OnceLock};

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
        UserInfo { id: "u1".into(), display_name: Some("Alice".into()), avatar_url: None }
    }

    #[tokio::test]
    async fn test_validate_token_success() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(mock_valid_response());
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        let _ = std::fs::remove_file(&path);
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

    /// validate_token_enforcing_expiry upgrades Offline to Expired when cache is >30 days old
    /// (§review-security-medium). Uses the mutex to prevent other tests writing fresh cache.
    #[tokio::test]
    async fn test_enforcing_variant_returns_expired_for_stale_offline_cache() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let stale = LicenseCache {
            version: 1,
            validated_at: Utc::now() - Duration::days(31),
            user: test_user(),
            repos: vec![],
        };
        let json = serde_json::to_string_pretty(&stale).unwrap();
        std::fs::write(&path, json).unwrap();
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(503);
        });
        let client = build_client();
        let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(state, LicenseState::Expired),
            "enforcing variant must return Expired for 31-day-old offline cache"
        );
    }

    /// validate_token_enforcing_expiry must treat a >30-day-old offline cache as Expired
    /// and a <30-day-old cache as Offline. Holds the mutex to prevent concurrent cache writes.
    #[tokio::test]
    async fn test_enforcing_expiry_30_day_boundary() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(503);
        });
        let client = build_client();

        let fresh = LicenseCache { version: 1, validated_at: Utc::now() - Duration::days(29), user: test_user(), repos: vec![] };
        std::fs::write(&path, serde_json::to_string_pretty(&fresh).unwrap()).unwrap();
        let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
        assert!(matches!(state, LicenseState::Offline { .. }), "29-day-old cache must not be hard-expired");

        let stale = LicenseCache { version: 1, validated_at: Utc::now() - Duration::days(31), user: test_user(), repos: vec![] };
        std::fs::write(&path, serde_json::to_string_pretty(&stale).unwrap()).unwrap();
        let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(state, LicenseState::Expired), "31-day-old cache must be hard-expired");
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
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
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
                    || {},
                )
                .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert!(
            mock_handle.hits() >= 2,
            "expected at least 2 validate calls in 350ms at 100ms interval, got {}",
            mock_handle.hits()
        );
    }

    /// Confirms on_expired callback is invoked when the backend returns 401.
    #[tokio::test]
    async fn test_revalidation_loop_calls_on_expired_callback() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(401);
        });

        let expired_called = Arc::new(AtomicBool::new(false));
        let expired_called_clone = expired_called.clone();

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
                    move || {
                        expired_called_clone.store(true, Ordering::SeqCst);
                    },
                )
                .await
            }
        });

        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        handle.abort();

        assert!(
            expired_called.load(Ordering::SeqCst),
            "on_expired callback should have been called when 401 received"
        );
    }

    #[test]
    fn warn_on_cache_write_is_noop_on_ok() {
        warn_on_cache_write(Ok(()));
    }

    #[test]
    fn warn_on_cache_write_does_not_panic_on_error() {
        warn_on_cache_write(Err("no space left on device".to_string()));
    }

    /// Web endpoint can return `display_name: null` — must not fail deserialization.
    #[tokio::test]
    async fn test_validate_token_null_display_name_succeeds() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(serde_json::json!({
                "valid": true,
                "user": { "id": "u1", "display_name": null, "avatar_url": null },
                "repos": []
            }));
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(state, LicenseState::Valid { .. }), "null display_name must parse successfully");
    }

    /// Web endpoint may omit `repos` field — must not fail deserialization.
    #[tokio::test]
    async fn test_validate_token_missing_repos_field_succeeds() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(serde_json::json!({
                "valid": true,
                "user": { "id": "u1", "display_name": "Alice", "avatar_url": null }
            }));
        });
        let client = build_client();
        let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(state, LicenseState::Valid { .. }), "missing repos field must parse successfully");
    }
}
