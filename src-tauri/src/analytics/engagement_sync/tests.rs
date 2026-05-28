// SPDX-License-Identifier: BUSL-1.1
use super::*;
use crate::providers::scheduling::{
    Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider,
};
use async_trait::async_trait;
use httpmock::prelude::*;
use std::io::Write;
use tempfile::TempDir;

struct MockEngagementProvider { likes: u64, reposts: u64, replies: u64 }

/// Provider whose `get_engagement` always returns a network error — used to verify
/// that `fetch_snapshot` falls back to a zero-valued snapshot on failure.
struct FailingEngagementProvider;

#[async_trait]
impl SchedulingProvider for FailingEngagementProvider {
    fn name(&self) -> &str { "failing-mock" }
    async fn schedule_post(&self, _: &str, _: &str, _: Option<chrono::DateTime<Utc>>, _: Option<&str>, _: Option<&str>) -> Result<PostScheduleResult, ProviderError> {
        Err(ProviderError::NotSupported("failing-mock".into()))
    }
    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> { Err(ProviderError::NotSupported("failing-mock".into())) }
    async fn cancel_post(&self, _: &str, _: &str) -> Result<(), ProviderError> { Err(ProviderError::NotSupported("failing-mock".into())) }
    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> { Err(ProviderError::NotSupported("failing-mock".into())) }
    async fn test_connection(&self) -> Result<(), ProviderError> { Err(ProviderError::NotSupported("failing-mock".into())) }
    async fn get_engagement(&self, _: &str, _: &str) -> Result<Engagement, ProviderError> {
        Err(ProviderError::NetworkError("simulated provider failure".into()))
    }
    fn post_url(&self, _: &str, _: &str) -> Option<String> { None }
}

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
    let path = dir.path().join(".postlane").join("posts").join(folder);
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

#[test]
fn test_read_posts_for_sync_reads_from_posts_subdirectory() {
    let tmp = TempDir::new().unwrap();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    // Correct location: .postlane/posts/
    let post_dir = tmp.path().join(".postlane").join("posts").join("correct-post");
    std::fs::create_dir_all(&post_dir).unwrap();
    let mut f = std::fs::File::create(post_dir.join("meta.json")).unwrap();
    f.write_all(make_sent_meta("zernio", "x", "id1", &recent).as_bytes()).unwrap();
    // Wrong location: .postlane/ directly (should be ignored)
    let wrong_dir = tmp.path().join(".postlane").join("wrong-post");
    std::fs::create_dir_all(&wrong_dir).unwrap();
    let mut f2 = std::fs::File::create(wrong_dir.join("meta.json")).unwrap();
    f2.write_all(make_sent_meta("zernio", "x", "id2", &recent).as_bytes()).unwrap();
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert_eq!(posts.len(), 1, "must only find posts in .postlane/posts/");
    assert_eq!(posts[0].platform_post_id, "id1");
}

#[tokio::test]
async fn test_post_snapshots_returns_zero_for_empty_slice() {
    let client = build_client();
    let result = post_snapshots(&[], "tok", &client, "http://localhost:1").await;
    assert_eq!(result.unwrap(), 0, "empty slice must return 0 without any HTTP call");
}

#[tokio::test]
async fn test_post_snapshots_returns_err_on_network_failure() {
    // Port 1 is not listening — connection refused = network error
    let client = build_client();
    let snap = EngagementSnapshot {
        repo_uuid: "r1".into(), post_folder: "p1".into(),
        provider: "zernio".into(), platform_post_id: "id1".into(),
        platform: "x".into(), likes: 0, shares: 0, comments: 0, impressions: None,
        fetched_at: Utc::now(),
    };
    let result = post_snapshots(&[snap], "tok", &client, "http://127.0.0.1:1").await;
    assert!(result.is_err(), "connection refused must return Err");
    assert!(result.unwrap_err().contains("Network error"), "error must say Network error");
}

#[test]
fn test_read_posts_for_sync_skips_missing_provider_field() {
    let tmp = TempDir::new().unwrap();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    let meta = serde_json::json!({
        "status": "sent",
        "sent_at": recent,
        "scheduler_ids": { "x": "id1" }
        // no "provider" field
    }).to_string();
    write_post(&tmp, "no-provider-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "missing provider must be skipped");
}

#[test]
fn test_read_posts_for_sync_skips_empty_platform_post_id() {
    let tmp = TempDir::new().unwrap();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    let meta = serde_json::json!({
        "status": "sent",
        "provider": "zernio",
        "sent_at": recent,
        "scheduler_ids": { "x": "" }   // empty post ID
    }).to_string();
    write_post(&tmp, "empty-id-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "empty platform_post_id must be skipped");
}

#[test]
fn test_read_posts_for_sync_skips_missing_scheduler_ids() {
    let tmp = TempDir::new().unwrap();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    let meta = serde_json::json!({
        "status": "sent",
        "provider": "zernio",
        "sent_at": recent
        // no scheduler_ids field
    }).to_string();
    write_post(&tmp, "no-ids-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "missing scheduler_ids must be skipped");
}

#[test]
fn test_read_post_for_sync_skips_post_with_no_sent_at() {
    let tmp = TempDir::new().unwrap();
    let meta = serde_json::json!({
        "status": "sent",
        "provider": "zernio",
        "scheduler_ids": { "x": "id1" }
        // no sent_at field
    }).to_string();
    write_post(&tmp, "no-sent-at-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "missing sent_at must be skipped");
}

#[test]
fn test_read_post_for_sync_skips_post_with_invalid_sent_at() {
    let tmp = TempDir::new().unwrap();
    let meta = serde_json::json!({
        "status": "sent",
        "provider": "zernio",
        "sent_at": "not-a-date",
        "scheduler_ids": { "x": "id1" }
    }).to_string();
    write_post(&tmp, "invalid-sent-at-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "unparseable sent_at must be skipped");
}

#[test]
fn test_read_post_for_sync_skips_post_sent_before_cutoff() {
    let tmp = TempDir::new().unwrap();
    // 31 days ago is before a 30-day cutoff
    let old_date = (Utc::now() - Duration::days(31)).to_rfc3339();
    let meta = serde_json::json!({
        "status": "sent",
        "provider": "zernio",
        "sent_at": old_date,
        "scheduler_ids": { "x": "id1" }
    }).to_string();
    write_post(&tmp, "before-cutoff-post", &meta);
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "post sent before cutoff must be excluded");
}

#[test]
fn test_read_posts_for_sync_skips_malformed_meta_json() {
    let tmp = TempDir::new().unwrap();
    let post_dir = tmp.path().join(".postlane").join("posts").join("bad-post");
    std::fs::create_dir_all(&post_dir).unwrap();
    std::fs::write(post_dir.join("meta.json"), "{ not json }").unwrap();
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "malformed meta.json must be skipped without panic");
}

// engagement_sync line 48 — posts dir absent → empty vec, no panic
#[test]
fn test_read_posts_for_sync_returns_empty_when_posts_dir_absent() {
    let tmp = TempDir::new().unwrap();
    // Deliberately do NOT create .postlane/posts/ — read_dir must return Err
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "absent posts dir must return empty vec without panic");
}

// engagement_sync line 63 — folder exists but has no meta.json → skipped
#[test]
fn test_read_post_for_sync_skips_folder_with_missing_meta_json() {
    let tmp = TempDir::new().unwrap();
    // Create the posts dir with a subdirectory that has no meta.json inside
    let post_dir = tmp.path().join(".postlane").join("posts").join("empty-post");
    std::fs::create_dir_all(&post_dir).unwrap();
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert!(posts.is_empty(), "folder without meta.json must be skipped");
}

// engagement_sync line 137 — provider.get_engagement() errors → zero-valued snapshot
#[tokio::test]
async fn test_fetch_snapshot_falls_back_to_zero_when_provider_engagement_fails() {
    let post = PostForSync {
        repo_uuid: "r1".into(),
        post_folder: "p1".into(),
        provider: "zernio".into(),
        platform: "x".into(),
        platform_post_id: "id1".into(),
    };
    let provider = FailingEngagementProvider;
    let snapshot = fetch_snapshot(&post, &provider).await;
    assert_eq!(snapshot.likes, 0, "engagement failure must produce zero likes");
    assert_eq!(snapshot.shares, 0, "engagement failure must produce zero shares");
    assert_eq!(snapshot.comments, 0, "engagement failure must produce zero comments");
    assert_eq!(snapshot.impressions, None);
    assert_eq!(snapshot.platform_post_id, "id1", "post identity must be preserved in zero snapshot");
}

#[test]
fn test_read_posts_for_sync_multi_platform_creates_one_entry_per_platform() {
    let tmp = TempDir::new().unwrap();
    let recent = (Utc::now() - Duration::hours(1)).to_rfc3339();
    let meta = serde_json::json!({
        "status": "sent", "provider": "zernio", "sent_at": recent,
        "scheduler_ids": { "x": "xid1", "linkedin": "lid1" }
    }).to_string();
    let post_dir = tmp.path().join(".postlane").join("posts").join("multi-post");
    std::fs::create_dir_all(&post_dir).unwrap();
    std::fs::write(post_dir.join("meta.json"), &meta).unwrap();
    let cutoff = Utc::now() - Duration::days(30);
    let posts = read_posts_for_sync("repo-1", tmp.path(), cutoff);
    assert_eq!(posts.len(), 2, "multi-platform post must create one entry per platform");
    let platforms: std::collections::HashSet<_> = posts.iter().map(|p| p.platform.as_str()).collect();
    assert!(platforms.contains("x"));
    assert!(platforms.contains("linkedin"));
}
