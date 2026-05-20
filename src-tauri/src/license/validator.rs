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

/// In test builds, individual tests redirect cache_path() to a private TempDir
/// so concurrent nextest processes never share the same file.
#[cfg(test)]
static TEST_CACHE_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<PathBuf>>> =
    std::sync::OnceLock::new();

fn cache_path() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let maybe = TEST_CACHE_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(path) = maybe {
            return Ok(path);
        }
    }
    Ok(postlane_dir()?.join("license_cache.json"))
}

pub fn read_license_cache() -> Option<LicenseCache> {
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


fn parse_rejection_reason(body: &str) -> String {
    if body.is_empty() {
        return "(no body)".to_string();
    }
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["reason"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.to_string())
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
        Ok(r) if r.status().as_u16() == 401 => {
            let body = r.text().await.unwrap_or_default();
            log::warn!("[validate] 401 from backend — reason: {}", parse_rejection_reason(&body));
            Ok(LicenseState::Expired)
        }
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
mod tests;
