// SPDX-License-Identifier: BUSL-1.1

use crate::providers::scheduling::{build_client, SchedulingProvider};
use crate::scheduler_credentials::get_credential_keyring_key;
use crate::storage::ReposConfig;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::AppHandle;
use tauri_plugin_keyring::KeyringExt;

const API_BASE: &str = "https://api.postlane.dev";

/// A single engagement snapshot for a published post
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EngagementSnapshot {
    pub repo_uuid: String,
    pub post_folder: String,
    pub provider: String,
    pub platform_post_id: String,
    pub platform: String,
    pub likes: u64,
    pub shares: u64,
    pub comments: u64,
    pub impressions: Option<u64>,
    pub fetched_at: DateTime<Utc>,
}

/// A post that is eligible for engagement sync (published in past 30 days)
#[derive(Debug)]
pub struct PostForSync {
    pub repo_uuid: String,
    pub post_folder: String,
    pub provider: String,
    pub platform: String,
    pub platform_post_id: String,
}

/// Reads published posts eligible for sync from a single repo directory.
pub fn read_posts_for_sync(
    repo_uuid: &str,
    repo_path: &Path,
    cutoff: DateTime<Utc>,
) -> Vec<PostForSync> {
    let posts_dir = repo_path.join(".postlane").join("posts");
    let entries = match std::fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    entries.flatten()
        .flat_map(|entry| read_post_for_sync(repo_uuid, &entry.path(), cutoff))
        .collect()
}

fn read_post_for_sync(
    repo_uuid: &str,
    folder: &Path,
    cutoff: DateTime<Utc>,
) -> Vec<PostForSync> {
    let meta_path = folder.join("meta.json");
    let content = match std::fs::read_to_string(&meta_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let meta: serde_json::Value = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(_) => return vec![],
    };
    if meta.get("status").and_then(|s| s.as_str()) != Some("sent") { return vec![]; }
    let sent_at_str = match meta.get("sent_at").and_then(|s| s.as_str()) {
        Some(s) => s,
        None => return vec![],
    };
    let sent_at: DateTime<Utc> = match sent_at_str.parse() {
        Ok(dt) => dt,
        Err(_) => return vec![],
    };
    if sent_at < cutoff { return vec![]; }
    let provider = match meta.get("provider").and_then(|p| p.as_str()) {
        Some(p) => p.to_string(),
        None => return vec![],
    };
    let scheduler_ids = match meta.get("scheduler_ids").and_then(|s| s.as_object()) {
        Some(ids) => ids,
        None => return vec![],
    };
    let post_folder = match folder.file_name().and_then(|n| n.to_str()) {
        Some(f) => f.to_string(),
        None => return vec![],
    };
    scheduler_ids.iter()
        .filter_map(|(platform, id_val)| {
            let platform_post_id = id_val.as_str()?.to_string();
            if platform_post_id.is_empty() { return None; }
            Some(PostForSync {
                repo_uuid: repo_uuid.to_string(),
                post_folder: post_folder.clone(),
                provider: provider.clone(),
                platform: platform.clone(),
                platform_post_id,
            })
        })
        .collect()
}

fn zero_snapshot(post: &PostForSync) -> EngagementSnapshot {
    EngagementSnapshot {
        repo_uuid: post.repo_uuid.clone(),
        post_folder: post.post_folder.clone(),
        provider: post.provider.clone(),
        platform_post_id: post.platform_post_id.clone(),
        platform: post.platform.clone(),
        likes: 0, shares: 0, comments: 0, impressions: None,
        fetched_at: Utc::now(),
    }
}

/// Fetches engagement for a single post from its provider.
/// `ProviderError::NotSupported` records a zero-valued snapshot rather than skipping.
pub async fn fetch_snapshot(
    post: &PostForSync,
    provider: &dyn SchedulingProvider,
) -> EngagementSnapshot {
    match provider.get_engagement(&post.platform_post_id, &post.platform).await {
        Ok(e) => EngagementSnapshot {
            repo_uuid: post.repo_uuid.clone(),
            post_folder: post.post_folder.clone(),
            provider: post.provider.clone(),
            platform_post_id: post.platform_post_id.clone(),
            platform: post.platform.clone(),
            likes: e.likes,
            shares: e.reposts,
            comments: e.replies,
            impressions: e.impressions,
            fetched_at: Utc::now(),
        },
        Err(_) => zero_snapshot(post),
    }
}

/// POSTs engagement snapshots to the backend in batches of 100.
pub async fn post_snapshots(
    snapshots: &[EngagementSnapshot],
    license_token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<usize, String> {
    if snapshots.is_empty() {
        return Ok(0);
    }
    let mut total = 0usize;
    for chunk in snapshots.chunks(100) {
        let resp = client
            .post(format!("{}/v1/engagement/sync", base_url))
            .bearer_auth(license_token)
            .json(&serde_json::json!({ "snapshots": chunk }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;
        if resp.status().is_success() {
            total += chunk.len();
        }
    }
    Ok(total)
}

/// Builds a provider for a post by looking up credentials from the OS keyring.
fn build_provider_for_post(
    post: &PostForSync,
    app: &AppHandle,
) -> Result<Box<dyn SchedulingProvider>, String> {
    use tauri_plugin_keyring::KeyringExt;
    let key = get_credential_keyring_key(&post.provider, &post.repo_uuid);
    let api_key = app.keyring()
        .get_password("postlane", &key)
        .map_err(|e| format!("Failed to retrieve credential: {}", e))?
        .ok_or_else(|| {
        format!("No {} credentials for repo {}", post.provider, post.repo_uuid)
    })?;
    crate::providers::scheduling::build_scheduling_provider(&post.provider, api_key)
}

/// Fetches engagement snapshots for all posts using a provider builder function.
/// Testable independently of AppHandle.
async fn fetch_snapshots_for_posts<F>(posts: &[PostForSync], build: F) -> Vec<EngagementSnapshot>
where
    F: Fn(&PostForSync) -> Result<Box<dyn SchedulingProvider>, String>,
{
    let mut snapshots = Vec::with_capacity(posts.len());
    for post in posts {
        let snapshot = match build(post) {
            Ok(provider) => fetch_snapshot(post, &*provider).await,
            Err(_) => zero_snapshot(post),
        };
        snapshots.push(snapshot);
    }
    snapshots
}

/// Reads all repos and collects posts eligible for engagement sync.
fn collect_posts_for_sync(repos: &ReposConfig) -> Vec<PostForSync> {
    let cutoff = Utc::now() - Duration::days(30);
    repos.repos.iter()
        .filter(|r| r.active)
        .flat_map(|r| read_posts_for_sync(&r.id, Path::new(&r.path), cutoff))
        .collect()
}

/// Main entry point: syncs engagement for all published posts from last 30 days.
pub async fn sync_engagement(app: &AppHandle) -> Result<usize, String> {
    let license_token = app.keyring()
        .get_password("postlane", "license")
        .map_err(|e| format!("Keyring error: {}", e))?
        .ok_or_else(|| "Not signed in".to_string())?;
    let repos = crate::storage::read_repos_with_recovery(
        &crate::init::postlane_dir()?.join("repos.json"),
    ).map_err(|e| format!("Failed to read repos: {:?}", e))?;
    let posts = collect_posts_for_sync(&repos);
    if posts.is_empty() {
        return Ok(0);
    }
    let client = build_client();
    let snapshots = fetch_snapshots_for_posts(&posts, |post| build_provider_for_post(post, app)).await;
    post_snapshots(&snapshots, &license_token, &client, API_BASE).await
}

#[cfg(test)]
mod tests;
