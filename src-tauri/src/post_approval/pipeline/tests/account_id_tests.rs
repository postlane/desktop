// SPDX-License-Identifier: BUSL-1.1
// Tests for fetch_and_cache_account_id — caching scheduler account IDs.
use super::super::*;
use crate::providers::scheduling::{
    Engagement, PostScheduleResult, ProviderError, SchedulerProfile, SchedulingProvider,
};
use crate::types::QueuedPost;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

struct MockProvider {
    profiles: Result<Vec<SchedulerProfile>, String>,
}

#[async_trait]
impl SchedulingProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }
    async fn schedule_post(
        &self,
        _: &str,
        _: &str,
        _: Option<DateTime<Utc>>,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        unimplemented!()
    }
    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        self.profiles.clone().map_err(ProviderError::Unknown)
    }
    async fn cancel_post(&self, _: &str, _: &str) -> Result<(), ProviderError> {
        unimplemented!()
    }
    async fn get_queue(&self) -> Result<Vec<QueuedPost>, ProviderError> {
        unimplemented!()
    }
    async fn get_engagement(&self, _: &str, _: &str) -> Result<Engagement, ProviderError> {
        unimplemented!()
    }
    fn post_url(&self, _: &str, _: &str) -> Option<String> {
        None
    }
}

fn make_config_dir(json: &str) -> tempfile::TempDir {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    std::fs::create_dir_all(dir.path().join(".postlane")).expect("mkdir .postlane");
    std::fs::write(dir.path().join(".postlane/config.json"), json).expect("write config.json");
    dir
}

#[tokio::test]
async fn test_fetch_and_cache_returns_id_when_profile_found() {
    let dir = make_config_dir(r#"{"version":1,"scheduler":{}}"#);
    let provider = MockProvider {
        profiles: Ok(vec![SchedulerProfile {
            id: "acc-bs-1".to_string(),
            name: "myhandle.bsky.social".to_string(),
            platforms: vec!["bluesky".to_string()],
        }]),
    };

    let result = fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;
    assert_eq!(result, Some("acc-bs-1".to_string()));
}

#[tokio::test]
async fn test_fetch_and_cache_writes_id_to_config_json() {
    let dir = make_config_dir(r#"{"version":1,"scheduler":{}}"#);
    let provider = MockProvider {
        profiles: Ok(vec![SchedulerProfile {
            id: "acc-bs-1".to_string(),
            name: "myhandle.bsky.social".to_string(),
            platforms: vec!["bluesky".to_string()],
        }]),
    };

    fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".postlane/config.json")).expect("read"),
    )
    .expect("parse");
    assert_eq!(
        config["scheduler"]["account_ids"]["bluesky"].as_str(),
        Some("acc-bs-1"),
        "account_id must be cached in config.json"
    );
}

#[tokio::test]
async fn test_fetch_and_cache_returns_none_when_no_matching_platform() {
    let dir = make_config_dir(r#"{"version":1,"scheduler":{}}"#);
    let provider = MockProvider {
        profiles: Ok(vec![SchedulerProfile {
            id: "acc-x-1".to_string(),
            name: "xuser".to_string(),
            platforms: vec!["x".to_string()],
        }]),
    };

    let result = fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;
    assert_eq!(result, None, "no bluesky profile → None");
}

// Upload Post returns one profile covering multiple platforms (same username for bluesky + x).
// This differs from providers like Zernio that return one profile per platform.
#[tokio::test]
async fn test_fetch_and_cache_returns_id_from_multi_platform_profile() {
    let dir = make_config_dir(r#"{"version":1,"scheduler":{}}"#);
    let provider = MockProvider {
        profiles: Ok(vec![SchedulerProfile {
            id: "myhandle".to_string(),
            name: "myhandle".to_string(),
            platforms: vec!["bluesky".to_string(), "x".to_string()],
        }]),
    };

    let result = fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;
    assert_eq!(
        result,
        Some("myhandle".to_string()),
        "multi-platform profile must match on any listed platform"
    );

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".postlane/config.json")).expect("read"),
    )
    .expect("parse");
    assert_eq!(
        config["scheduler"]["account_ids"]["bluesky"].as_str(),
        Some("myhandle"),
    );
}

#[tokio::test]
async fn test_fetch_and_cache_returns_none_when_list_profiles_fails() {
    let dir = make_config_dir(r#"{"version":1,"scheduler":{}}"#);
    let provider = MockProvider {
        profiles: Err("HTTP 401 Unauthorized".to_string()),
    };

    let result = fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;
    assert_eq!(result, None, "list_profiles error must not panic — returns None");
}

#[tokio::test]
async fn test_fetch_and_cache_preserves_existing_account_ids_when_caching() {
    let dir = make_config_dir(
        r#"{"version":1,"scheduler":{"account_ids":{"x":"acc-x-existing"}}}"#,
    );
    let provider = MockProvider {
        profiles: Ok(vec![SchedulerProfile {
            id: "acc-bs-1".to_string(),
            name: "myhandle.bsky.social".to_string(),
            platforms: vec!["bluesky".to_string()],
        }]),
    };

    fetch_and_cache_account_id("bluesky", &provider, dir.path()).await;

    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join(".postlane/config.json")).expect("read"),
    )
    .expect("parse");
    assert_eq!(
        config["scheduler"]["account_ids"]["x"].as_str(),
        Some("acc-x-existing"),
        "existing x account_id must be preserved"
    );
    assert_eq!(
        config["scheduler"]["account_ids"]["bluesky"].as_str(),
        Some("acc-bs-1"),
        "new bluesky account_id must also be written"
    );
}
