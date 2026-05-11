// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    #[default]
    Ok,
    Failed,
    /// Written by approve_post after a successful scheduler call.
    /// Signals to engagement_sync that this post folder has published content.
    Sent,
    /// Catch-all for legacy string values written by pre-M19 code (e.g. "ready", "dismissed").
    #[serde(other)]
    Unknown,
}

/// Canonical representation of `.postlane/posts/{folder}/meta.json`.
/// All five commands that read or write meta.json must use this struct —
/// no command may hand-roll JSON keys or construct the meta path independently.
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct PostMeta {
    /// None = field absent in JSON (pre-M19 post); Some([]) = post-M19, not edited;
    /// Some(["x"]) = edited on x. Must be Option<Vec<String>> — #[serde(default)] on
    /// Vec would erase the None/Some([]) distinction, making pre-M19 post counting wrong.
    pub edited_platforms: Option<Vec<String>>,
    /// ISO8601 of most recent save_post_draft call.
    pub edited_at: Option<String>,
    /// platform → ISO8601 sent_at; written by approve_post on success.
    #[serde(default)]
    pub sent_platforms: HashMap<String, String>,
    /// platform → scheduler-assigned post ID; written by approve_post on success.
    #[serde(default)]
    pub scheduler_ids: HashMap<String, String>,
    /// platform → published post URL; written by approve_post on success.
    #[serde(default)]
    pub platform_urls: HashMap<String, String>,
    /// ISO8601 scheduled send time set by the user.
    pub scheduled_for: Option<String>,
    /// LLM model that generated the post draft.
    pub model_name: Option<String>,
    /// Some(PostStatus::Failed) when approve_post encounters a scheduler error.
    pub status: Option<PostStatus>,
    /// Error message when status = Some(PostStatus::Failed).
    pub error: Option<String>,
}

impl PostMeta {
    /// Canonical path for a post's meta.json: `{repo_path}/.postlane/posts/{post_folder}/meta.json`.
    pub fn path_for(repo_path: &Path, post_folder: &str) -> PathBuf {
        repo_path.join(".postlane/posts").join(post_folder).join("meta.json")
    }

    /// Load and deserialise meta.json; returns `PostMeta::default()` when the file is absent.
    pub fn load(path: &Path) -> Result<PostMeta, String> {
        if !path.exists() {
            return Ok(PostMeta::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {:?}: {}", path, e))
    }

    /// Atomically write meta.json: write to `.tmp` then rename.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialise PostMeta: {}", e))?;
        let tmp = path.with_extension("json.tmp");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
        }
        std::fs::write(&tmp, &json)
            .map_err(|e| format!("Failed to write {:?}: {}", tmp, e))?;
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("Failed to rename {:?} → {:?}: {}", tmp, path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_post_meta_load_returns_default_when_file_absent() {
        let path = Path::new("/nonexistent/path/meta.json");
        let meta = PostMeta::load(path).expect("should not error on absent file");
        assert!(meta.edited_platforms.is_none());
        assert!(meta.sent_platforms.is_empty());
        assert!(meta.scheduler_ids.is_empty());
        assert!(meta.platform_urls.is_empty());
        assert!(meta.status.is_none());
    }

    #[test]
    fn test_post_meta_load_deserialises_all_fields() {
        let dir = std::env::temp_dir().join("postlane_test_post_meta_all");
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("meta.json");
        fs::write(&path, r#"{
            "edited_platforms": ["x"],
            "edited_at": "2026-05-01T10:00:00Z",
            "sent_platforms": {"x": "2026-05-02T10:00:00Z"},
            "scheduler_ids": {"x": "sched-123"},
            "platform_urls": {"x": "https://x.com/post/123"},
            "scheduled_for": "2026-05-02T10:00:00Z",
            "model_name": "claude-sonnet-4-5",
            "status": "failed",
            "error": "scheduler error"
        }"#).expect("write json");
        let meta = PostMeta::load(&path).expect("load");
        assert_eq!(meta.edited_platforms, Some(vec!["x".to_string()]));
        assert_eq!(meta.edited_at.as_deref(), Some("2026-05-01T10:00:00Z"));
        assert_eq!(meta.sent_platforms.get("x").map(String::as_str), Some("2026-05-02T10:00:00Z"));
        assert_eq!(meta.scheduler_ids.get("x").map(String::as_str), Some("sched-123"));
        assert_eq!(meta.platform_urls.get("x").map(String::as_str), Some("https://x.com/post/123"));
        assert_eq!(meta.model_name.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(meta.status, Some(PostStatus::Failed));
        assert_eq!(meta.error.as_deref(), Some("scheduler error"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_post_meta_load_absent_edited_platforms_deserialises_as_none() {
        let dir = std::env::temp_dir().join("postlane_test_post_meta_ep_none");
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("meta.json");
        fs::write(&path, r#"{}"#).expect("write json");
        let meta = PostMeta::load(&path).expect("load");
        assert!(meta.edited_platforms.is_none(), "absent field must be None, not Some([])");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_post_meta_load_empty_edited_platforms_deserialises_as_some_empty() {
        let dir = std::env::temp_dir().join("postlane_test_post_meta_ep_empty");
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("meta.json");
        fs::write(&path, r#"{"edited_platforms": []}"#).expect("write json");
        let meta = PostMeta::load(&path).expect("load");
        assert_eq!(meta.edited_platforms, Some(vec![]), "empty array must be Some([]), not None");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_post_meta_save_writes_atomically() {
        let dir = std::env::temp_dir().join("postlane_test_post_meta_atomic");
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("meta.json");
        let meta = PostMeta {
            model_name: Some("claude-test".to_string()),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        // .tmp must not remain after successful save
        let tmp = path.with_extension("json.tmp");
        assert!(!tmp.exists(), ".tmp file must be cleaned up after rename");
        assert!(path.exists(), "meta.json must exist after save");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_post_meta_round_trips() {
        let dir = std::env::temp_dir().join("postlane_test_post_meta_roundtrip");
        fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("meta.json");
        let mut original = PostMeta::default();
        original.edited_platforms = Some(vec!["x".to_string(), "bluesky".to_string()]);
        original.model_name = Some("claude-opus-4".to_string());
        original.sent_platforms.insert("x".to_string(), "2026-05-01T00:00:00Z".to_string());
        original.status = Some(PostStatus::Failed);
        original.error = Some("network error".to_string());
        original.save(&path).expect("save");
        let loaded = PostMeta::load(&path).expect("load");
        assert_eq!(loaded.edited_platforms, original.edited_platforms);
        assert_eq!(loaded.model_name, original.model_name);
        assert_eq!(loaded.sent_platforms, original.sent_platforms);
        assert_eq!(loaded.status, original.status);
        assert_eq!(loaded.error, original.error);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_post_meta_load_tolerates_legacy_status_values() {
        for legacy in &["ready", "dismissed", "queued"] {
            let json = format!(r#"{{"status": "{}"}}"#, legacy);
            let meta: PostMeta = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("should not fail on legacy status '{}': {}", legacy, e));
            assert_ne!(meta.status, Some(PostStatus::Failed),
                "legacy status '{}' must not be treated as failed", legacy);
        }
    }

    #[test]
    fn test_post_status_sent_round_trips() {
        let json = r#"{"status": "sent"}"#;
        let meta: PostMeta = serde_json::from_str(json).expect("should parse");
        assert_eq!(meta.status, Some(PostStatus::Sent), "\"sent\" must deserialise as PostStatus::Sent");
        let serialised = serde_json::to_string(&meta).expect("serialise");
        assert!(serialised.contains("\"sent\""), "PostStatus::Sent must serialise as \"sent\"");
    }
}
