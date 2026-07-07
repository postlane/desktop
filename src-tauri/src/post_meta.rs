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

/// Unsplash photographer attribution required by Unsplash API Terms of Service.
/// Written to meta.json when a photo is selected via search; null for non-Unsplash images.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct ImageAttribution {
    pub photographer_name: String,
    pub photographer_url: String,
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
    /// User-set image URL for this post. skip_serializing_if ensures the merge-save in
    /// PostMeta::save() does not overwrite an existing image_url with null when this is None.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// Unsplash `download_location` URL — must be called once per Unsplash API ToS.
    /// Absent for non-Unsplash images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_download_location: Option<String>,
    /// ISO 8601 UTC timestamp of when the download trigger was fired.
    /// Null means the trigger has not been fired yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_download_triggered_at: Option<String>,
    /// Image provider identifier (e.g. "unsplash"). Null for manually pasted URLs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_source: Option<String>,
    /// Photographer attribution required by Unsplash ToS. Null for non-Unsplash images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_attribution: Option<ImageAttribution>,
    /// Skill command that generated this post (e.g. "draft-changelog", "draft-show-hn").
    /// Written by the skill, not the app — skip_serializing_if preserves a skill-written
    /// value when the app merges its own fields into meta.json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
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
        crate::init::read_json_file(path)
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
        let mut original = PostMeta {
            edited_platforms: Some(vec!["x".to_string(), "bluesky".to_string()]),
            model_name: Some("claude-opus-4".to_string()),
            status: Some(PostStatus::Failed),
            error: Some("network error".to_string()),
            ..Default::default()
        };
        original.sent_platforms.insert("x".to_string(), "2026-05-01T00:00:00Z".to_string());
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
        let meta = PostMeta { repo_path: Some("/workspace/child-repo".to_string()), ..Default::default() };
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
    fn test_post_meta_save_creates_parent_dir_when_absent() {
        // Line 93: create_dir_all is called when the parent directory does not yet exist.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let nested = dir.path().join("a").join("b").join("meta.json");
        let meta = PostMeta {
            model_name: Some("claude-test".to_string()),
            ..Default::default()
        };
        meta.save(&nested).expect("save should create parent dirs automatically");
        assert!(nested.exists(), "meta.json must exist after save with missing parent");
    }

    #[test]
    fn test_post_meta_command_absent_reads_as_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{"status":"ok"}"#).expect("write");
        let meta = PostMeta::load(&path).expect("load");
        assert!(meta.command.is_none(), "absent command field must deserialise as None");
    }

    #[test]
    fn test_post_meta_command_round_trips() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        let meta = PostMeta {
            command: Some("draft-changelog".to_string()),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        let loaded = PostMeta::load(&path).expect("load");
        assert_eq!(
            loaded.command.as_deref(),
            Some("draft-changelog"),
            "command must survive save/load roundtrip"
        );
    }

    #[test]
    fn test_post_meta_command_preserved_on_save_when_none() {
        // Skill writes meta.json with command; app later saves PostMeta without command set.
        // The command field must survive because skip_serializing_if = "Option::is_none"
        // keeps it out of the overlay, so the merge leaves the original value in place.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{"command":"draft-changelog","model_name":"claude-test"}"#)
            .expect("write initial");
        // App saves without knowing the command
        let meta = PostMeta {
            model_name: Some("claude-sonnet-4-6".to_string()),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        let raw = fs::read_to_string(&path).expect("read back");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(
            v["command"].as_str(),
            Some("draft-changelog"),
            "command must survive app save when PostMeta.command is None"
        );
    }

    #[test]
    fn test_post_meta_save_merges_into_existing_file() {
        // Lines 103-109: the merge branch runs when the existing file is a JSON object.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        // Write an initial file with a field PostMeta doesn't own.
        fs::write(&path, r#"{"extra_field":"keep-me","model_name":"old-model"}"#)
            .expect("write initial");
        let meta = PostMeta {
            model_name: Some("new-model".to_string()),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        let raw = fs::read_to_string(&path).expect("read back");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(v["extra_field"].as_str(), Some("keep-me"), "unknown fields must survive merge");
        assert_eq!(v["model_name"].as_str(), Some("new-model"), "PostMeta field must overwrite old value");
    }

    #[test]
    fn test_post_status_sent_round_trips() {
        let json = r#"{"status": "sent"}"#;
        let meta: PostMeta = serde_json::from_str(json).expect("should parse");
        assert_eq!(meta.status, Some(PostStatus::Sent), "\"sent\" must deserialise as PostStatus::Sent");
        let serialised = serde_json::to_string(&meta).expect("serialise");
        assert!(serialised.contains("\"sent\""), "PostStatus::Sent must serialise as \"sent\"");
    }

    // --- 21.8.7: image attribution fields ---

    #[test]
    fn test_post_meta_image_attribution_round_trips() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        let meta = PostMeta {
            image_download_location: Some("https://api.unsplash.com/photos/abc123/download".to_string()),
            image_download_triggered_at: Some("2026-05-21T10:00:00Z".to_string()),
            image_source: Some("unsplash".to_string()),
            image_attribution: Some(ImageAttribution {
                photographer_name: "Jane Doe".to_string(),
                photographer_url: "https://unsplash.com/@janedoe".to_string(),
            }),
            ..Default::default()
        };
        meta.save(&path).expect("save");
        let loaded = PostMeta::load(&path).expect("load");
        assert_eq!(
            loaded.image_download_location.as_deref(),
            Some("https://api.unsplash.com/photos/abc123/download")
        );
        assert_eq!(loaded.image_download_triggered_at.as_deref(), Some("2026-05-21T10:00:00Z"));
        assert_eq!(loaded.image_source.as_deref(), Some("unsplash"));
        let attr = loaded.image_attribution.expect("image_attribution must be present");
        assert_eq!(attr.photographer_name, "Jane Doe");
        assert_eq!(attr.photographer_url, "https://unsplash.com/@janedoe");
    }

    #[test]
    fn test_post_meta_image_attribution_none_when_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("meta.json");
        fs::write(&path, r#"{"status":"ready"}"#).expect("write");
        let meta = PostMeta::load(&path).expect("load");
        assert!(meta.image_attribution.is_none(), "absent image_attribution must be None");
        assert!(meta.image_download_location.is_none());
        assert!(meta.image_download_triggered_at.is_none());
        assert!(meta.image_source.is_none());
    }

    #[test]
    fn test_post_meta_image_fields_omitted_when_none() {
        let meta = PostMeta::default();
        let json = serde_json::to_string(&meta).expect("serialise");
        assert!(!json.contains("image_download_location"), "absent field must not appear in JSON");
        assert!(!json.contains("image_attribution"), "absent field must not appear in JSON");
        assert!(!json.contains("image_download_triggered_at"), "absent field must not appear in JSON");
        assert!(!json.contains("image_source"), "absent field must not appear in JSON");
    }
}
