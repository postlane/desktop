// SPDX-License-Identifier: BUSL-1.1

use crate::security::api_error::format_api_error;
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
        log::debug!("[webhook_poller] draft already exists for event '{}' — skipping", event.id);
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

    log::info!("[webhook_poller] created draft '{}' from event '{}'", folder_name, event.id);
    Ok(meta_path)
}

// ── HTTP calls ────────────────────────────────────────────────────────────────

/// Fetches pending webhook events for `project_id` from the API.
pub async fn fetch_pending_events(
    api_base: &str,
    project_id: &str,
    token: &str,
) -> Result<Vec<PendingEvent>, String> {
    crate::project_validation::validate_project_id(project_id)?;
    let client = reqwest::Client::new();
    let url = format!("{}/v1/events/pending?project_id={}", api_base, project_id);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("fetch_pending_events request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        log::warn!("[webhook_poller] fetch_pending_events returned {} for project '{}'", status, project_id);
        return Err(format_api_error("fetch_pending_events", status, ""));
    }

    #[derive(Deserialize)]
    struct Body { events: Vec<PendingEvent> }
    let body: Body = resp.json().await
        .map_err(|e| format!("fetch_pending_events parse failed: {}", e))?;
    log::info!("[webhook_poller] fetched {} pending event(s) for project '{}'", body.events.len(), project_id);
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
        let status = resp.status().as_u16();
        log::warn!("[webhook_poller] mark_delivered returned {} for {} event(s)", status, event_ids.len());
        return Err(format_api_error("mark_delivered", status, ""));
    }
    log::info!("[webhook_poller] marked {} event(s) as delivered", event_ids.len());
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
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane")).expect("create .postlane");
        std::fs::write(
            dir.path().join(".postlane/config.json"),
            r#"{"project_id":"proj-abc-123"}"#,
        ).expect("write config");

        let result = project_id_from_config(dir.path());
        assert_eq!(result, Some("proj-abc-123".to_string()));
    }

    #[test]
    fn test_project_id_from_config_returns_none_when_file_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let result = project_id_from_config(dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_project_id_from_config_returns_none_when_field_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane")).expect("create .postlane");
        std::fs::write(dir.path().join(".postlane/config.json"), r#"{"scheduler":{}}"#).expect("write");

        let result = project_id_from_config(dir.path());
        assert!(result.is_none());
    }

    // ── create_draft_from_push ───────────────────────────────────────────────

    #[test]
    fn test_create_draft_writes_meta_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-001", "fix: typo in readme");

        let meta_path = create_draft_from_push(dir.path(), &event).expect("should succeed");
        assert!(meta_path.exists(), "meta.json should be created");
    }

    #[test]
    fn test_create_draft_uses_first_commit_message_as_title() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-002", "feat: add dark mode");

        let meta_path = create_draft_from_push(dir.path(), &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["title"].as_str(), Some("feat: add dark mode"));
    }

    #[test]
    fn test_create_draft_sets_source_to_webhook() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-003", "chore: bump deps");

        let meta_path = create_draft_from_push(dir.path(), &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["source"].as_str(), Some("webhook"));
    }

    #[test]
    fn test_create_draft_is_idempotent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane/posts")).expect("create posts");
        let event = make_push_event("evt-004", "refactor: rename module");

        let path1 = create_draft_from_push(dir.path(), &event).expect("first call");
        let path2 = create_draft_from_push(dir.path(), &event).expect("second call");
        assert_eq!(path1, path2, "same path returned on second call");

        let entries: Vec<_> = std::fs::read_dir(dir.path().join(".postlane/posts"))
            .expect("read posts dir")
            .flatten()
            .collect();
        assert_eq!(entries.len(), 1, "only one draft folder created");
    }

    #[test]
    fn test_create_draft_fallback_title_when_no_commits() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(dir.path().join(".postlane/posts")).expect("create posts");
        let event = PendingEvent {
            id: "evt-005".to_string(),
            event_type: "push".to_string(),
            payload: serde_json::json!({ "commits": [] }),
        };

        let meta_path = create_draft_from_push(dir.path(), &event).expect("should succeed");
        let content = std::fs::read_to_string(&meta_path).expect("read meta.json");
        let meta: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(meta["title"].as_str(), Some("Push event"));
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

    // ── §project_id_validation (MEDIUM-3) ────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_pending_events_rejects_project_id_with_injection_chars() {
        // project_id is interpolated into the URL query string. Without validation,
        // a value like "proj&injected=true" would append extra query parameters.
        // validate_project_id must reject any character that is not a-z, A-Z, 0-9, -, or _.
        // The error must come from validation (before any HTTP call), not from a
        // network failure — so the error message must describe the invalid character.
        let result = fetch_pending_events("https://api.example.com", "proj&injected=true", "tok").await;
        assert!(result.is_err(), "project_id with & must be rejected before URL construction");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("invalid characters"),
            "error must say 'invalid characters' (from validate_project_id), not a network error. Got: {}", msg
        );
    }
}
