// SPDX-License-Identifier: BUSL-1.1

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SendResult {
    pub success: bool,
    pub platform_results: Option<HashMap<String, String>>,
    pub error: Option<String>,
    pub fallback_provider: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostMeta {
    pub status: String,
    pub platforms: Vec<String>,
    pub schedule: Option<String>,
    pub trigger: Option<String>,
    pub scheduler_ids: Option<HashMap<String, String>>,
    pub platform_results: Option<HashMap<String, String>>,
    pub platform_urls: Option<HashMap<String, String>>,
    pub error: Option<String>,
    pub image_url: Option<String>,
    pub image_source: Option<String>,
    pub image_attribution: Option<String>,
    pub llm_model: Option<String>,
    pub created_at: Option<String>,
    pub sent_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QueuedPost {
    pub post_id: String,
    pub platform: String,
    pub scheduled_for: DateTime<Utc>,
    pub content_preview: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SchedulerProfile {
    pub id: String,
    pub name: String,
    pub platforms: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Engagement {
    pub likes: u64,
    pub reposts: u64,
    pub replies: u64,
    pub impressions: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RepoHealthStatus {
    pub id: String,
    pub reachable: bool,
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_meta_serialization() {
        let meta = PostMeta {
            status: "draft".to_string(),
            platforms: vec!["x".to_string(), "bluesky".to_string()],
            schedule: Some("2024-01-01T00:00:00Z".to_string()),
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            platform_urls: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: Some("claude-3-opus".to_string()),
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            sent_at: None,
        };

        let json = serde_json::to_string(&meta).expect("Failed to serialize");
        let deserialized: PostMeta = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.status, "draft");
        assert_eq!(deserialized.platforms.len(), 2);
    }

    #[test]
    fn test_partial_platform_results_deserializes() {
        let json = r#"{
            "status": "sent",
            "platforms": ["x", "bluesky"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": {"x": "123", "bluesky": "456"},
            "platform_results": {"x": "success"},
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        let meta: PostMeta = serde_json::from_str(json).expect("Should deserialize partial results");
        assert_eq!(meta.platform_results.unwrap().get("x").unwrap(), "success");
    }

    #[test]
    fn test_queued_post_with_datetime() {
        let preview = "This is a test post preview that is exactly eighty characters long right";
        let post = QueuedPost {
            post_id: "test-id".to_string(),
            platform: "x".to_string(),
            scheduled_for: Utc::now(),
            content_preview: preview.to_string(),
        };

        let json = serde_json::to_string(&post).expect("Failed to serialize");
        let deserialized: QueuedPost = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.post_id, "test-id");
        assert_eq!(deserialized.content_preview.len(), preview.len());
    }
}
