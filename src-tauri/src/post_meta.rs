// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Deserializes a value as `T::default()` when the JSON value is `null` or the field is absent.
/// `#[serde(default)]` alone handles absent fields but not explicit `null` — pre-M19 meta.json
/// files wrote explicit nulls for map fields, so both cases must be handled.
fn default_on_null<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    let opt: Option<T> = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

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
    #[serde(default, deserialize_with = "default_on_null")]
    pub sent_platforms: HashMap<String, String>,
    /// platform → scheduler-assigned post ID; written by approve_post on success.
    #[serde(default, deserialize_with = "default_on_null")]
    pub scheduler_ids: HashMap<String, String>,
    /// platform → published post URL; written by approve_post on success.
    #[serde(default, deserialize_with = "default_on_null")]
    pub platform_urls: HashMap<String, String>,
    /// ISO8601 scheduled send time set by the user.
    pub scheduled_for: Option<String>,
    /// LLM model that generated the post draft.
    pub model_name: Option<String>,
    /// Some(PostStatus::Failed) when approve_post encounters a scheduler error.
    pub status: Option<PostStatus>,
    /// Error message when status = Some(PostStatus::Failed).
    pub error: Option<String>,
    /// Child repo path that triggered this draft (workspace mode only).
    /// `None` in single-repo mode or in pre-20.8 `meta.json` files — never treated as error.
    pub repo_path: Option<String>,
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

    /// Atomically write meta.json: merge self's fields into existing JSON, then write via `.tmp`.
    /// Fields present in the file but absent from PostMeta (e.g. image_url, platforms) are preserved.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create dir {:?}: {}", parent, e))?;
        }
        let mut base: serde_json::Value = if path.exists() {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
            serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        let overlay = serde_json::to_value(self)
            .map_err(|e| format!("Failed to serialise PostMeta: {}", e))?;
        if let (serde_json::Value::Object(base_map), serde_json::Value::Object(overlay_map)) =
            (&mut base, overlay)
        {
            for (k, v) in overlay_map {
                base_map.insert(k, v);
            }
        }
        let json = serde_json::to_string_pretty(&base)
            .map_err(|e| format!("Failed to serialise merged PostMeta: {}", e))?;
        let tmp = path.with_extension("json.tmp");
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
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
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
    }

    #[test]
    fn test_post_meta_load_absent_edited_platforms_deserialises_as_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{}"#).expect("write json");
        let meta = PostMeta::load(&path).expect("load");
        assert!(meta.edited_platforms.is_none(), "absent field must be None, not Some([])");
    }

    #[test]
    fn test_post_meta_load_empty_edited_platforms_deserialises_as_some_empty() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{"edited_platforms": []}"#).expect("write json");
        let meta = PostMeta::load(&path).expect("load");
        assert_eq!(meta.edited_platforms, Some(vec![]), "empty array must be Some([]), not None");
    }

    #[test]
    fn test_post_meta_save_writes_atomically() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        let meta = PostMeta {
            model_name: Some("claude-test".to_string()),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        // .tmp must not remain after successful save
        let tmp = path.with_extension("json.tmp");
        assert!(!tmp.exists(), ".tmp file must be cleaned up after rename");
        assert!(path.exists(), "meta.json must exist after save");
    }

    #[test]
    fn test_post_meta_round_trips() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
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
    fn test_post_meta_absent_repo_path_reads_as_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        // Pre-20.8 meta.json with no repo_path field
        fs::write(&path, r#"{"model_name":"claude-test"}"#).expect("write");
        let meta = PostMeta::load(&path).expect("load");
        assert!(meta.repo_path.is_none(), "absent repo_path must be None, not an error");
    }

    #[test]
    fn test_post_meta_repo_path_round_trips() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        let mut meta = PostMeta::default();
        meta.repo_path = Some("/workspace/child-repo".to_string());
        meta.save(&path).expect("save");
        let loaded = PostMeta::load(&path).expect("load");
        assert_eq!(loaded.repo_path, Some("/workspace/child-repo".to_string()));
    }

    #[test]
    fn test_post_meta_save_preserves_unknown_fields() {
        // Regression: save() used to overwrite the file completely, losing fields written by
        // /draft-post (image_url, platforms, trigger) that PostMeta doesn't declare.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{
            "status": "ready",
            "platforms": ["x", "bluesky"],
            "trigger": "launch announcement",
            "image_url": "https://example.com/img.png",
            "model_name": "claude-sonnet-4-5"
        }"#).expect("write initial");
        let mut meta = PostMeta::load(&path).expect("load");
        meta.edited_platforms = Some(vec!["x".to_string()]);
        meta.edited_at = Some("2026-05-18T10:00:00Z".to_string());
        meta.save(&path).expect("save");
        let raw = fs::read_to_string(&path).expect("read back");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(v["image_url"].as_str(), Some("https://example.com/img.png"), "image_url must survive save");
        assert_eq!(v["platforms"].as_array().map(|a| a.len()), Some(2), "platforms must survive save");
        assert_eq!(v["trigger"].as_str(), Some("launch announcement"), "trigger must survive save");
        assert_eq!(v["edited_platforms"][0].as_str(), Some("x"), "edited_platforms must be written");
    }

    #[test]
    fn test_post_meta_load_tolerates_explicit_null_for_map_fields() {
        // Pre-M19 meta.json files wrote explicit null for scheduler_ids, platform_urls,
        // and sent_platforms. #[serde(default)] only handles absent fields, not null.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{
            "status": "ready",
            "platforms": ["x"],
            "schedule": null,
            "scheduler_ids": null,
            "platform_results": null,
            "platform_urls": null,
            "sent_platforms": null
        }"#).expect("write json");
        let meta = PostMeta::load(&path).expect("explicit null map fields must not error");
        assert!(meta.scheduler_ids.is_empty(), "null scheduler_ids must deserialise as empty map");
        assert!(meta.platform_urls.is_empty(), "null platform_urls must deserialise as empty map");
        assert!(meta.sent_platforms.is_empty(), "null sent_platforms must deserialise as empty map");
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
