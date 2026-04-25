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
    let postlane_dir = repo_path.join(".postlane");
    let entries = match std::fs::read_dir(&postlane_dir) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut posts = Vec::new();
    for entry in entries.flatten() {
        if let Some(post) = read_post_for_sync(repo_uuid, &entry.path(), cutoff) {
            posts.push(post);
        }
    }
    posts
}

fn read_post_for_sync(
    repo_uuid: &str,
    folder: &Path,
    cutoff: DateTime<Utc>,
) -> Option<PostForSync> {
    let meta_path = folder.join("meta.json");
    let content = std::fs::read_to_string(&meta_path).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&content).ok()?;
    if meta.get("status")?.as_str()? != "sent" {
        return None;
    }
    let sent_at_str = meta.get("sent_at")?.as_str()?;
    let sent_at: DateTime<Utc> = sent_at_str.parse().ok()?;
    if sent_at < cutoff {
        return None;
    }
    let provider = meta.get("provider")?.as_str()?.to_string();
    let scheduler_ids = meta.get("scheduler_ids")?.as_object()?;
    let (platform, post_id) = scheduler_ids.iter().next().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))?;
    Some(PostForSync {
        repo_uuid: repo_uuid.to_string(),
        post_folder: folder.file_name()?.to_str()?.to_string(),
        provider,
        platform,
        platform_post_id: post_id,
    })
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
    let keys = get_credential_keyring_key(&post.provider, Some(&post.repo_uuid));
    let mut api_key: Option<String> = None;
    for key in &keys {
        if let Ok(Some(k)) = app.keyring().get_password("postlane", key) {
            api_key = Some(k);
            break;
        }
    }
    let api_key = api_key.ok_or_else(|| {
        format!("No {} credentials for repo {}", post.provider, post.repo_uuid)
    })?;
    crate::account_config::build_scheduling_provider(&post.provider, api_key)
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
mod tests {
    use super::*;
    use crate::providers::scheduling::{
        Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider,
    };
    use async_trait::async_trait;
    use httpmock::prelude::*;
    use std::io::Write;
    use tempfile::TempDir;

    struct MockEngagementProvider { likes: u64, reposts: u64, replies: u64 }

    #[async_trait]
    impl SchedulingProvider for MockEngagementProvider {
        fn name(&self) -> &str { "mock" }
        async fn schedule_post(&self, _: &str, _: &str, _: Option<chrono::DateTime<Utc>>, _: Option<&str>, _: Option<&str>) -> Result<PostScheduleResult, ProviderError> {
            Err(ProviderError::NotSupported("mock".into()))
        }
        async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> { Err(ProviderError::NotSupported("mock".into())) }
        async fn cancel_post(&self, _: &str, _: &str) -> Result<(), ProviderError> { Err(ProviderError::NotSupported("mock".into())) }
        async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> { Err(ProviderError::NotSupported("mock".into())) }
        async fn test_connection(&self) -> Result<(), ProviderError> { Err(ProviderError::NotSupported("mock".into())) }
        async fn get_engagement(&self, _: &str, _: &str) -> Result<Engagement, ProviderError> {
            Ok(Engagement { likes: self.likes, reposts: self.reposts, replies: self.replies, impressions: Some(500), platform_url: None })
        }
        fn post_url(&self, _: &str, _: &str) -> Option<String> { None }
    }

    #[tokio::test]
    async fn test_fetch_snapshots_uses_provider_engagement_not_zeros() {
        let posts = vec![PostForSync {
            repo_uuid: "r1".into(), post_folder: "p1".into(),
            provider: "mock".into(), platform: "x".into(), platform_post_id: "id1".into(),
        }];
        let snapshots = fetch_snapshots_for_posts(&posts, |_| {
            Ok(Box::new(MockEngagementProvider { likes: 42, reposts: 7, replies: 3 }) as Box<dyn SchedulingProvider>)
        }).await;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].likes, 42, "Should use provider data, not hard-coded zero");
        assert_eq!(snapshots[0].shares, 7);
        assert_eq!(snapshots[0].comments, 3);
    }

    #[tokio::test]
    async fn test_fetch_snapshots_falls_back_to_zero_on_provider_error() {
        let posts = vec![PostForSync {
            repo_uuid: "r1".into(), post_folder: "p1".into(),
            provider: "nokey".into(), platform: "x".into(), platform_post_id: "id1".into(),
        }];
        let snapshots = fetch_snapshots_for_posts(&posts, |_| {
            Err("No credentials".to_string())
        }).await;
        assert_eq!(snapshots[0].likes, 0);
        assert_eq!(snapshots[0].shares, 0);
    }

    fn make_sent_meta(provider: &str, platform: &str, post_id: &str, sent_at: &str) -> String {
        serde_json::json!({
            "status": "sent",
            "provider": provider,
            "sent_at": sent_at,
            "scheduler_ids": { platform: post_id }
        }).to_string()
    }

    fn write_post(dir: &TempDir, folder: &str, meta: &str) {
        let path = dir.path().join(".postlane").join(folder);
        std::fs::create_dir_all(&path).unwrap();
        let mut f = std::fs::File::create(path.join("meta.json")).unwrap();
        f.write_all(meta.as_bytes()).unwrap();
    }

    #[tokio::test]
    async fn test_engagement_sync_writes_snapshots() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/engagement/sync");
            then.status(200).json_body(serde_json::json!({ "inserted": 1 }));
        });
        let client = build_client();
        let snapshots = vec![EngagementSnapshot {
            repo_uuid: "r1".into(), post_folder: "p1".into(),
            provider: "zernio".into(), platform_post_id: "id1".into(),
            platform: "x".into(), likes: 5, shares: 2, comments: 1, impressions: None,
            fetched_at: Utc::now(),
        }];
        let result = post_snapshots(&snapshots, "tok", &client, &server.base_url()).await;
        assert_eq!(result.unwrap(), 1);
        mock.assert();
    }

    #[tokio::test]
    async fn test_engagement_sync_handles_not_supported() {
        let tmp = TempDir::new().unwrap();
        let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
        write_post(&tmp, "post-1", &make_sent_meta("webhook", "x", "id1", &recent));
        let cutoff = Utc::now() - Duration::days(30);
        let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].provider, "webhook");
    }

    #[test]
    fn test_read_posts_for_sync_skips_old_posts() {
        let tmp = TempDir::new().unwrap();
        let old = (Utc::now() - Duration::days(31)).to_rfc3339();
        write_post(&tmp, "old-post", &make_sent_meta("zernio", "x", "id1", &old));
        let cutoff = Utc::now() - Duration::days(30);
        let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
        assert!(posts.is_empty(), "Old posts should be excluded");
    }

    #[test]
    fn test_read_posts_for_sync_skips_non_sent() {
        let tmp = TempDir::new().unwrap();
        let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
        let meta = serde_json::json!({ "status": "ready", "sent_at": recent }).to_string();
        write_post(&tmp, "draft-post", &meta);
        let cutoff = Utc::now() - Duration::days(30);
        let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
        assert!(posts.is_empty());
    }
}
