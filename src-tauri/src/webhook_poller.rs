// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingEvent {
    pub id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

// ── Config helpers ────────────────────────────────────────────────────────────

/// Reads `project_id` from `.postlane/config.json` in the given repo.
/// Returns `None` when config is absent or the field is not set.
pub fn project_id_from_config(repo_path: &Path) -> Option<String> {
    let config_path = repo_path.join(".postlane").join("config.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    val.get("project_id")?.as_str().map(|s| s.to_string())
}

// ── Draft creation ────────────────────────────────────────────────────────────

/// Extracts the first commit message from a push event payload.
/// Returns a fallback string when the payload is malformed or has no commits.
fn first_commit_message(payload: &serde_json::Value) -> String {
    payload["commits"]
        .as_array()
        .and_then(|c| c.first())
        .and_then(|c| c["message"].as_str())
        .unwrap_or("Push event")
        .to_string()
}

/// Creates a draft folder inside `repo_path/.postlane/posts/` from a push event.
/// The folder name is derived from `event.id` for idempotency — calling twice
/// with the same event returns the existing path without overwriting it.
/// Returns the path to the created `meta.json`.
pub fn create_draft_from_push(repo_path: &Path, event: &PendingEvent) -> Result<PathBuf, String> {
    let folder_name = format!("webhook-{}", event.id.replace('/', "-"));
    let post_dir = repo_path.join(".postlane/posts").join(&folder_name);

    if post_dir.exists() {
        return Ok(post_dir.join("meta.json"));
    }

    std::fs::create_dir_all(&post_dir)
        .map_err(|e| format!("Failed to create draft dir {}: {}", post_dir.display(), e))?;

    let title = first_commit_message(&event.payload);
    let meta = serde_json::json!({
        "title": title,
        "source": "webhook",
        "event_id": event.id,
    });

    let meta_path = post_dir.join("meta.json");
    let tmp_path = post_dir.join("meta.json.tmp");
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta: {}", e))?;

    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write meta.json.tmp: {}", e))?;

    std::fs::rename(&tmp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(meta_path)
}

// ── HTTP calls ────────────────────────────────────────────────────────────────

/// Fetches pending webhook events for `project_id` from the API.
pub async fn fetch_pending_events(
    api_base: &str,
    project_id: &str,
    token: &str,
) -> Result<Vec<PendingEvent>, String> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/events/pending?project_id={}", api_base, project_id);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("fetch_pending_events request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("fetch_pending_events: HTTP {}", resp.status()));
    }

    #[derive(Deserialize)]
    struct Body { events: Vec<PendingEvent> }
    let body: Body = resp.json().await
        .map_err(|e| format!("fetch_pending_events parse failed: {}", e))?;
    Ok(body.events)
}

/// Marks the given event IDs as delivered via the API.
pub async fn mark_delivered(
    api_base: &str,
    event_ids: Vec<String>,
    token: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/events/pending", api_base);
    let resp = client
        .patch(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "event_ids": event_ids }))
        .send()
        .await
        .map_err(|e| format!("mark_delivered request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("mark_delivered: HTTP {}", resp.status()));
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_push_event(id: &str, commit_msg: &str) -> PendingEvent {
        PendingEvent {
            id: id.to_string(),
            event_type: "push".to_string(),
            payload: serde_json::json!({
                "repository": { "full_name": "acme/my-repo", "owner": { "login": "acme" } },
                "commits": [{ "id": "abc123", "message": commit_msg }],
            }),
        }
    }

    // ── project_id_from_config ───────────────────────────────────────────────

    #[test]
    fn test_project_id_from_config_returns_id_when_present() {
        let dir = std::env::temp_dir().join("postlane_test_poller_proj_id_present");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane")).expect("create .postlane");
        std::fs::write(
            dir.join(".postlane/config.json"),
            r#"{"project_id":"proj-abc-123"}"#,
        ).expect("write config");

        let result = project_id_from_config(&dir);
        assert_eq!(result, Some("proj-abc-123".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_id_from_config_returns_none_when_file_absent() {
        let dir = std::env::temp_dir().join("postlane_test_poller_proj_id_absent");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let result = project_id_from_config(&dir);
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_id_from_config_returns_none_when_field_missing() {
        let dir = std::env::temp_dir().join("postlane_test_poller_proj_id_no_field");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane")).expect("create .postlane");
        std::fs::write(dir.join(".postlane/config.json"), r#"{"scheduler":{}}"#).expect("write");

        let result = project_id_from_config(&dir);
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── create_draft_from_push ───────────────────────────────────────────────

    #[test]
    fn test_create_draft_writes_meta_json() {
        let dir = std::env::temp_dir().join("postlane_test_poller_draft_write");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-001", "fix: typo in readme");

        let meta_path = create_draft_from_push(&dir, &event).expect("should succeed");
        assert!(meta_path.exists(), "meta.json should be created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_create_draft_uses_first_commit_message_as_title() {
        let dir = std::env::temp_dir().join("postlane_test_poller_draft_title");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-002", "feat: add dark mode");

        let meta_path = create_draft_from_push(&dir, &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["title"].as_str(), Some("feat: add dark mode"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_create_draft_sets_source_to_webhook() {
        let dir = std::env::temp_dir().join("postlane_test_poller_draft_source");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-003", "chore: bump deps");

        let meta_path = create_draft_from_push(&dir, &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["source"].as_str(), Some("webhook"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_create_draft_is_idempotent() {
        let dir = std::env::temp_dir().join("postlane_test_poller_draft_idempotent");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-004", "refactor: rename module");

        let path1 = create_draft_from_push(&dir, &event).expect("first call");
        let path2 = create_draft_from_push(&dir, &event).expect("second call");
        assert_eq!(path1, path2, "same path returned on second call");

        let entries: Vec<_> = std::fs::read_dir(dir.join(".postlane/posts"))
            .expect("read posts dir")
            .flatten()
            .collect();
        assert_eq!(entries.len(), 1, "only one draft folder created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_create_draft_fallback_title_when_no_commits() {
        let dir = std::env::temp_dir().join("postlane_test_poller_draft_no_commits");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".postlane/posts")).expect("create posts");
        let event = PendingEvent {
            id: "evt-005".to_string(),
            event_type: "push".to_string(),
            payload: serde_json::json!({ "commits": [] }),
        };

        let meta_path = create_draft_from_push(&dir, &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["title"].as_str(), Some("Push event"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── fetch_pending_events ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_pending_events_returns_events_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/events/pending").query_param("project_id", "proj-1");
            then.status(200).json_body(serde_json::json!({
                "events": [
                    { "id": "evt-1", "event_type": "push", "payload": {} }
                ]
            }));
        });

        let events = fetch_pending_events(&server.base_url(), "proj-1", "tok")
            .await
            .expect("should succeed");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "evt-1");
    }

    #[tokio::test]
    async fn test_fetch_pending_events_returns_empty_list() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/events/pending");
            then.status(200).json_body(serde_json::json!({ "events": [] }));
        });

        let events = fetch_pending_events(&server.base_url(), "proj-1", "tok")
            .await
            .expect("should succeed");
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_pending_events_returns_err_on_non_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/events/pending");
            then.status(401).json_body(serde_json::json!({ "error": "unauthorized" }));
        });

        let result = fetch_pending_events(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_err());
    }

    // ── mark_delivered ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_mark_delivered_sends_correct_event_ids() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/events/pending")
                .json_body(serde_json::json!({ "event_ids": ["evt-1", "evt-2"] }));
            then.status(200).json_body(serde_json::json!({ "updated": 2 }));
        });

        mark_delivered(&server.base_url(), vec!["evt-1".to_string(), "evt-2".to_string()], "tok")
            .await
            .expect("should succeed");

        mock.assert();
    }

    #[tokio::test]
    async fn test_mark_delivered_returns_err_on_non_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/events/pending");
            then.status(400).json_body(serde_json::json!({ "error": "bad" }));
        });

        let result = mark_delivered(&server.base_url(), vec!["evt-1".to_string()], "tok").await;
        assert!(result.is_err());
    }
}
