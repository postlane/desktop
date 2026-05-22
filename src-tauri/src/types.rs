// SPDX-License-Identifier: BUSL-1.1

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::post_meta::ImageAttribution;

/// Canonical post type used for both draft and published queries.
/// `status` discriminates: 'ready'/'failed' for drafts, 'sent'/'queued' for published.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Post {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_path: String,
    pub post_folder: String,
    pub status: String,
    pub platforms: Vec<String>,
    pub schedule: Option<String>,
    pub schedule_source: Option<String>,
    pub platform_results: Option<HashMap<String, String>>,
    pub llm_model: Option<String>,
    pub created_at: Option<String>,
    // Draft-only (None for published posts)
    pub trigger: Option<String>,
    pub error: Option<String>,
    pub image_url: Option<String>,
    // Published-only (None for draft posts)
    pub scheduler_ids: Option<HashMap<String, String>>,
    pub platform_urls: Option<HashMap<String, String>>,
    pub provider: Option<String>,
    pub sent_at: Option<String>,
    // M19 additions — populated by rebuilt get_all_drafts (19.0.15)
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub model_name: Option<String>,
    /// UTC ISO8601 timestamp the post is scheduled to publish; None if not scheduled.
    /// Distinct from `schedule` (the raw string from meta.json) — this is the canonical field.
    #[serde(default)]
    pub scheduled_for: Option<String>,
    /// Single platform key for this row (e.g. "x") — draft rows are one per platform.
    #[serde(default)]
    pub platform: String,
    /// Post body text read from the platform .md file.
    #[serde(default)]
    pub text: String,
    /// ISO8601 timestamp of most recent user edit; `None` if post has never been edited.
    #[serde(default)]
    pub edited_at: Option<String>,
    /// Unsplash photographer attribution; `None` for non-Unsplash images.
    #[serde(default)]
    pub image_attribution: Option<ImageAttribution>,
}

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
    pub image_attribution: Option<ImageAttribution>,
    pub llm_model: Option<String>,
    pub created_at: Option<String>,
    pub sent_at: Option<String>,
    /// UTC timestamp of when the project voice guide was last saved.
    /// Written to meta.json so it is possible to trace which guide version was active
    /// when each post was generated. Missing field (None) means guide was unknown at creation time.
    #[serde(default)]
    pub voice_guide_version: Option<String>,
    /// "default" = auto-set from default post time; "user" = set by user in the UI; None = unknown.
    #[serde(default)]
    pub schedule_source: Option<String>,
    /// IANA timezone identifier the user had selected when setting the schedule (e.g. "America/New_York").
    /// Stored so the UI can display the original local time. None means unknown or system default.
    #[serde(default)]
    pub schedule_timezone: Option<String>,
}

/// Per-platform published post row — returned by `get_org_published` (M19 History view).
/// One row per (post_folder, platform) pair; sorted by `sent_at` descending.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PublishedPost {
    pub text: String,
    pub platform: String,
    pub repo_path: String,
    pub post_folder: String,
    pub sent_at: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub scheduler_ids: HashMap<String, String>,
    #[serde(default)]
    pub platform_urls: HashMap<String, String>,
    /// Per-platform payload from the scheduler; empty map in v1.
    /// `serde_json::Value` accepts any future v4 per-platform shape without a schema change.
    #[serde(default)]
    pub platform_results: HashMap<String, serde_json::Value>,
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
            voice_guide_version: None,
            schedule_source: None,
            schedule_timezone: None,
        };

        let json = serde_json::to_string(&meta).expect("Failed to serialize");
        let deserialized: PostMeta = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.status, "draft");
        assert_eq!(deserialized.platforms.len(), 2);
    }

    // 21.8.7: image_attribution must be a struct, not Option<String>
    #[test]
    fn test_post_meta_image_attribution_is_struct() {
        let json = r#"{
            "status": "ready",
            "platforms": ["x"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": "https://images.unsplash.com/photo-abc",
            "image_source": "unsplash",
            "image_attribution": {
                "photographer_name": "Jane Doe",
                "photographer_url": "https://unsplash.com/@janedoe"
            },
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;
        let meta: PostMeta = serde_json::from_str(json).expect("must deserialize attribution struct");
        let attr = meta.image_attribution.expect("image_attribution must be Some");
        assert_eq!(attr.photographer_name, "Jane Doe");
        assert_eq!(attr.photographer_url, "https://unsplash.com/@janedoe");
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
        assert_eq!(meta.platform_results.expect("platform_results should be Some").get("x").expect("key 'x' should exist"), "success");
    }

    #[test]
    fn test_post_draft_fields_serialization() {
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "Repo".to_string(),
            repo_path: "/path".to_string(), post_folder: "my-post".to_string(),
            status: "ready".to_string(), platforms: vec!["x".to_string()],
            schedule: None, schedule_source: None, platform_results: None, llm_model: None, created_at: None,
            trigger: Some("Launch".to_string()), error: None, image_url: None,
            scheduler_ids: None, platform_urls: None, provider: None, sent_at: None,
            project_id: None, model_name: None, scheduled_for: None,
            platform: String::default(), text: String::default(), edited_at: None,
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.status, "ready");
        assert_eq!(back.trigger.as_deref(), Some("Launch"));
        assert!(back.sent_at.is_none());
    }

    #[test]
    fn test_post_published_fields_serialization() {
        let mut ids = HashMap::new();
        ids.insert("x".to_string(), "tw-123".to_string());
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "Repo".to_string(),
            repo_path: "/path".to_string(), post_folder: "my-post".to_string(),
            status: "sent".to_string(), platforms: vec!["x".to_string()],
            schedule: None, schedule_source: None, platform_results: None, llm_model: None, created_at: None,
            trigger: None, error: None, image_url: None,
            scheduler_ids: Some(ids), platform_urls: None,
            provider: Some("zernio".to_string()), sent_at: Some("2024-01-01T00:00:00Z".to_string()),
            project_id: None, model_name: None, scheduled_for: None,
            platform: String::default(), text: String::default(), edited_at: None,
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.status, "sent");
        assert_eq!(back.provider.as_deref(), Some("zernio"));
        assert!(back.trigger.is_none());
    }

    #[test]
    fn test_draft_post_includes_project_id() {
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "R".to_string(),
            repo_path: "/r".to_string(), post_folder: "f".to_string(),
            status: "ready".to_string(), platforms: vec![],
            schedule: None, schedule_source: None, platform_results: None,
            llm_model: None, model_name: None, created_at: None,
            trigger: None, error: None, image_url: None,
            scheduler_ids: None, platform_urls: None, provider: None, sent_at: None,
            project_id: Some("proj-abc".to_string()), scheduled_for: None,
            platform: String::default(), text: String::default(), edited_at: None,
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.project_id, Some("proj-abc".to_string()));
    }

    #[test]
    fn test_draft_post_project_id_null_when_absent() {
        let json = r#"{"repo_id":"r","repo_name":"R","repo_path":"/r","post_folder":"f","status":"ready","platforms":[],"schedule":null,"schedule_source":null,"platform_results":null,"llm_model":null,"created_at":null,"trigger":null,"error":null,"image_url":null,"scheduler_ids":null,"platform_urls":null,"provider":null,"sent_at":null}"#;
        let post: Post = serde_json::from_str(json).expect("deserializes");
        assert_eq!(post.project_id, None);
    }

    #[test]
    fn test_draft_post_includes_model_name() {
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "R".to_string(),
            repo_path: "/r".to_string(), post_folder: "f".to_string(),
            status: "ready".to_string(), platforms: vec![],
            schedule: None, schedule_source: None, platform_results: None,
            llm_model: None, model_name: Some("claude-sonnet-4-5".to_string()), created_at: None,
            trigger: None, error: None, image_url: None,
            scheduler_ids: None, platform_urls: None, provider: None, sent_at: None,
            project_id: None, scheduled_for: None,
            platform: String::default(), text: String::default(), edited_at: None,
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.model_name, Some("claude-sonnet-4-5".to_string()));
    }

    #[test]
    fn test_draft_post_model_name_null_when_absent() {
        let json = r#"{"repo_id":"r","repo_name":"R","repo_path":"/r","post_folder":"f","status":"ready","platforms":[],"schedule":null,"schedule_source":null,"platform_results":null,"llm_model":null,"created_at":null,"trigger":null,"error":null,"image_url":null,"scheduler_ids":null,"platform_urls":null,"provider":null,"sent_at":null}"#;
        let post: Post = serde_json::from_str(json).expect("deserializes");
        assert_eq!(post.model_name, None);
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

    #[test]
    fn test_published_post_includes_project_id() {
        let post = PublishedPost {
            project_id: Some("proj-abc".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&post).expect("serialize");
        let back: PublishedPost = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.project_id, Some("proj-abc".to_string()));
    }

    #[test]
    fn test_published_post_project_id_null_when_absent() {
        let json = r#"{"text":"","platform":"x","repo_path":"","post_folder":"","sent_at":""}"#;
        let post: PublishedPost = serde_json::from_str(json).expect("deserialize");
        assert_eq!(post.project_id, None);
    }

    #[test]
    fn test_published_post_scheduler_ids_default_empty_when_absent() {
        let json = r#"{"text":"","platform":"x","repo_path":"","post_folder":"","sent_at":""}"#;
        let post: PublishedPost = serde_json::from_str(json).expect("deserialize");
        assert!(post.scheduler_ids.is_empty());
    }

    #[test]
    fn test_published_post_platform_urls_default_empty_when_absent() {
        let json = r#"{"text":"","platform":"x","repo_path":"","post_folder":"","sent_at":""}"#;
        let post: PublishedPost = serde_json::from_str(json).expect("deserialize");
        assert!(post.platform_urls.is_empty());
    }

    #[test]
    fn test_draft_post_includes_scheduled_for() {
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "R".to_string(),
            repo_path: "/r".to_string(), post_folder: "f".to_string(),
            status: "ready".to_string(), platforms: vec![],
            schedule: None, schedule_source: None, platform_results: None,
            llm_model: None, model_name: None, created_at: None,
            trigger: None, error: None, image_url: None,
            scheduler_ids: None, platform_urls: None, provider: None, sent_at: None,
            project_id: None, scheduled_for: Some("2026-06-01T10:00:00Z".to_string()),
            platform: String::default(), text: String::default(), edited_at: None,
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.scheduled_for.as_deref(), Some("2026-06-01T10:00:00Z"));
    }

    #[test]
    fn test_draft_post_scheduled_for_null_when_absent() {
        let json = r#"{"repo_id":"r","repo_name":"R","repo_path":"/r","post_folder":"f","status":"ready","platforms":[],"schedule":null,"schedule_source":null,"platform_results":null,"llm_model":null,"created_at":null,"trigger":null,"error":null,"image_url":null,"scheduler_ids":null,"platform_urls":null,"provider":null,"sent_at":null}"#;
        let post: Post = serde_json::from_str(json).expect("deserializes");
        assert_eq!(post.scheduled_for, None);
    }

    #[test]
    fn test_draft_post_edited_at_round_trips() {
        let post = Post {
            repo_id: "r1".to_string(), repo_name: "R".to_string(),
            repo_path: "/r".to_string(), post_folder: "f".to_string(),
            status: "ready".to_string(), platforms: vec![],
            schedule: None, schedule_source: None, platform_results: None,
            llm_model: None, model_name: None, created_at: None,
            trigger: None, error: None, image_url: None,
            scheduler_ids: None, platform_urls: None, provider: None, sent_at: None,
            project_id: None, scheduled_for: None,
            edited_at: Some("2026-05-10T12:00:00Z".to_string()),
            platform: String::default(), text: String::default(),
            image_attribution: None,
        };
        let json = serde_json::to_string(&post).expect("serializes");
        let back: Post = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.edited_at.as_deref(), Some("2026-05-10T12:00:00Z"));
    }

    #[test]
    fn test_draft_post_edited_at_null_when_absent() {
        let json = r#"{"repo_id":"r","repo_name":"R","repo_path":"/r","post_folder":"f","status":"ready","platforms":[],"schedule":null,"schedule_source":null,"platform_results":null,"llm_model":null,"created_at":null,"trigger":null,"error":null,"image_url":null,"scheduler_ids":null,"platform_urls":null,"provider":null,"sent_at":null}"#;
        let post: Post = serde_json::from_str(json).expect("deserializes");
        assert_eq!(post.edited_at, None);
    }

    #[test]
    fn test_published_post_platform_results_default_empty_when_absent() {
        let json = r#"{"text":"","platform":"x","repo_path":"","post_folder":"","sent_at":""}"#;
        let post: PublishedPost = serde_json::from_str(json).expect("deserialize");
        assert!(post.platform_results.is_empty());
    }
}
