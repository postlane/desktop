// SPDX-License-Identifier: BUSL-1.1

use super::{cache, sites, PostAnalytics};
use crate::providers::scheduling::build_client;
use chrono::Utc;
use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

const API_BASE: &str = "https://api.postlane.dev";

#[derive(Deserialize)]
struct SiteTokenResponse {
    site_token: String,
}

#[derive(Deserialize)]
struct PostAnalyticsEntry {
    #[serde(rename = "utm_content")]
    _utm_content: String,
    sessions: u64,
    unique_sessions: u64,
    top_referrer: Option<String>,
}

fn read_license_token(app: &AppHandle) -> Result<String, String> {
    app.keyring()
        .get_password("postlane", "license")
        .map_err(|e| format!("Failed to read license token: {}", e))?
        .ok_or_else(|| "Not signed in — sign in at postlane.dev to enable analytics".to_string())
}

/// Fetches (or returns cached) site token for a repo from the analytics backend.
async fn fetch_site_token_inner(
    repo_id: &str,
    license_token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<String, String> {
    let url = format!("{}/v1/analytics/site-token?repo_uuid={}", base_url, repo_id);
    let resp = client
        .get(&url)
        .bearer_auth(license_token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("Backend returned {}", resp.status().as_u16()));
    }
    let body: SiteTokenResponse = resp
        .json()
        .await
        .map_err(|e| format!("Invalid response: {}", e))?;
    Ok(body.site_token)
}

/// Fetches aggregated analytics for a post from the analytics backend.
async fn fetch_post_analytics_inner(
    site_token: &str,
    post_folder: &str,
    license_token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<PostAnalytics, String> {
    let from = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
    let url = format!(
        "{}/v1/analytics/posts?site_token={}&utm_content={}&from={}",
        base_url, site_token, post_folder, from
    );
    let resp = client
        .get(&url)
        .bearer_auth(license_token)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Ok(PostAnalytics::default());
    }
    let entries: Vec<PostAnalyticsEntry> = resp.json().await.unwrap_or_default();
    Ok(entries.into_iter().next().map(|e| PostAnalytics {
        sessions: e.sessions,
        unique_sessions: e.unique_sessions,
        top_referrer: e.top_referrer,
    }).unwrap_or_default())
}

/// Tauri command — fetches or creates the site token for a repo.
#[tauri::command]
pub async fn get_site_token(repo_id: String, app: AppHandle) -> Result<String, String> {
    if let Some(cached) = sites::get_cached_site_token(&repo_id) {
        return Ok(cached);
    }
    let license_token = read_license_token(&app)?;
    let client = build_client();
    let token = fetch_site_token_inner(&repo_id, &license_token, &client, API_BASE).await?;
    sites::save_site_token(&repo_id, &token)?;
    Ok(token)
}

/// Tauri command — returns cached or freshly fetched analytics for a post.
/// Never errors the caller — returns zero-valued struct on any failure.
#[tauri::command]
pub async fn get_post_analytics(
    repo_id: String,
    post_folder: String,
    app: AppHandle,
) -> Result<PostAnalytics, String> {
    let site_token = match sites::get_cached_site_token(&repo_id) {
        Some(t) => t,
        None => return Ok(PostAnalytics::default()),
    };
    let key = cache::cache_key(&repo_id, &post_folder);
    let cache = cache::read_analytics_cache();
    if let Some(entry) = cache.entries.get(&key) {
        if cache::is_entry_valid(entry) {
            return Ok(entry.data.clone());
        }
    }
    let license_token = match read_license_token(&app) {
        Ok(t) => t,
        Err(_) => return Ok(PostAnalytics::default()),
    };
    let client = build_client();
    let data = fetch_post_analytics_inner(&site_token, &post_folder, &license_token, &client, API_BASE)
        .await
        .unwrap_or_default();
    let mut updated = cache::read_analytics_cache();
    updated.entries.insert(key, cache::new_entry(data.clone()));
    let _ = cache::write_analytics_cache(&updated);
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_fetch_site_token_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/analytics/site-token");
            then.status(200).json_body(serde_json::json!({ "site_token": "tok-xyz" }));
        });
        let client = build_client();
        let result = fetch_site_token_inner("repo-1", "license-tok", &client, &server.base_url()).await;
        assert_eq!(result.unwrap(), "tok-xyz");
    }

    #[tokio::test]
    async fn test_fetch_site_token_403_returns_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/analytics/site-token");
            then.status(403);
        });
        let client = build_client();
        let result = fetch_site_token_inner("repo-1", "bad-tok", &client, &server.base_url()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_post_analytics_returns_data() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/analytics/posts");
            then.status(200).json_body(serde_json::json!([
                { "utm_content": "my-post", "sessions": 20, "unique_sessions": 15, "top_referrer": "t.co" }
            ]));
        });
        let client = build_client();
        let result = fetch_post_analytics_inner("tok", "my-post", "license", &client, &server.base_url()).await;
        let analytics = result.unwrap();
        assert_eq!(analytics.sessions, 20);
        assert_eq!(analytics.unique_sessions, 15);
        assert_eq!(analytics.top_referrer.as_deref(), Some("t.co"));
    }

    #[tokio::test]
    async fn test_fetch_post_analytics_backend_unavailable_returns_zero() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/analytics/posts");
            then.status(503);
        });
        let client = build_client();
        let result = fetch_post_analytics_inner("tok", "post", "license", &client, &server.base_url()).await;
        let a = result.unwrap();
        assert_eq!(a.sessions, 0);
        assert_eq!(a.unique_sessions, 0);
        assert!(a.top_referrer.is_none());
    }
}
