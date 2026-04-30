// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::post_io::{collect_posts_from_repos, sort_by_status_priority_then_timestamp};
use crate::types::Post;
use std::fs;
use tauri::State;

pub type DraftPost = Post;

/// Parse a single post directory into a `Post`, returning `None` if the
/// post should be skipped (missing/invalid meta, wrong status, etc.).
fn parse_draft_post(
    post_path: &std::path::Path,
    repo_id: &str,
    repo_name: &str,
    repo_path: &str,
) -> Option<Post> {
    if !post_path.is_dir() {
        return None;
    }
    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&meta_path).ok()?;
    let meta: serde_json::Value = serde_json::from_str(&content).ok()?;

    let status = match meta.get("status").and_then(|s| s.as_str()) {
        Some(s @ "ready") | Some(s @ "failed") => s.to_string(),
        _ => return None,
    };

    let post_folder = post_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    let platforms: Vec<String> = meta
        .get("platforms")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let platform_results = meta.get("platform_results").and_then(|v| {
        v.as_object().map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
    });

    Some(Post {
        repo_id: repo_id.to_string(),
        repo_name: repo_name.to_string(),
        repo_path: repo_path.to_string(),
        post_folder,
        status,
        platforms,
        schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
        trigger: meta.get("trigger").and_then(|v| v.as_str()).map(String::from),
        platform_results,
        error: meta.get("error").and_then(|v| v.as_str()).map(String::from),
        image_url: meta.get("image_url").and_then(|v| v.as_str()).map(String::from),
        llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
        created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
        scheduler_ids: None,
        platform_urls: None,
        provider: None,
        sent_at: None,
    })
}

fn sort_drafts(drafts: &mut [Post]) {
    sort_by_status_priority_then_timestamp(drafts, "failed", "ready", |d| &d.status, |d| d.created_at.as_deref());
}

pub fn get_all_drafts_impl(state: &AppState) -> Result<Vec<Post>, String> {
    let repos = state.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))?;
    let mut drafts = collect_posts_from_repos(&repos.repos, true, parse_draft_post);
    sort_drafts(&mut drafts);
    Ok(drafts)
}

#[tauri::command]
pub fn get_all_drafts(state: State<'_, AppState>) -> Result<Vec<Post>, String> {
    get_all_drafts_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_state(repos: Vec<Repo>) -> AppState {
        AppState::new(ReposConfig { version: 1, repos })
    }

    fn write_meta(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_all_drafts_empty() {
        let state = make_state(vec![]);
        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_drafts_only_ready_and_failed_included() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_filter_dq");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"],"created_at":"2024-06-01T10:00:00Z"}"#);
        write_meta(&dir, "p2", r#"{"status":"sent","platforms":["x"],"created_at":"2024-06-02T10:00:00Z"}"#);
        write_meta(&dir, "p3", r#"{"status":"failed","platforms":["bluesky"],"created_at":"2024-06-03T10:00:00Z","error":"timeout"}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 2, "sent post should be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_failed_before_ready() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_sort_dq");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"],"created_at":"2024-06-03T00:00:00Z"}"#);
        write_meta(&dir, "p2", r#"{"status":"failed","platforms":["x"],"created_at":"2024-06-01T00:00:00Z"}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[1].status, "ready");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_inactive_repo_excluded() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_inactive_dq");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: false, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert!(result.is_empty(), "inactive repo should be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_enriches_with_repo_context() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_context_dq");
        write_meta(&dir, "my-post", r#"{"status":"ready","platforms":["x","bluesky"],"trigger":"Launched v2","created_at":"2024-06-01T00:00:00Z"}"#);

        let state = make_state(vec![Repo {
            id: "abc-123".to_string(), name: "My App".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].repo_id, "abc-123");
        assert_eq!(result[0].repo_name, "My App");
        assert_eq!(result[0].post_folder, "my-post");
        assert_eq!(result[0].trigger.as_deref(), Some("Launched v2"));
        assert_eq!(result[0].platforms, vec!["x", "bluesky"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_sorts_same_status_by_created_at_descending() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_sort_ts");
        write_meta(&dir, "old", r#"{"status":"ready","platforms":["x"],"created_at":"2026-01-01T00:00:00Z"}"#);
        write_meta(&dir, "new", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);

        let state = make_state(vec![Repo { id: "r1".to_string(), name: "Repo".to_string(), path: dir.to_str().unwrap().to_string(), active: true, added_at: "2024-01-01T00:00:00Z".to_string() }]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result[0].post_folder, "new");
        assert_eq!(result[1].post_folder, "old");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_none_created_at_sorts_before_timestamped() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_none_ts");
        write_meta(&dir, "with-ts", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);
        write_meta(&dir, "no-ts", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_state(vec![Repo { id: "r1".to_string(), name: "Repo".to_string(), path: dir.to_str().unwrap().to_string(), active: true, added_at: "2024-01-01T00:00:00Z".to_string() }]);
        let result = get_all_drafts_impl(&state).expect("ok");
        // None created_at sorts first (treated as newer/pending)
        assert!(result[0].created_at.is_none());
        assert!(result[1].created_at.is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_two_none_created_at_are_stable() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_two_none");
        write_meta(&dir, "a", r#"{"status":"ready","platforms":["x"]}"#);
        write_meta(&dir, "b", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_state(vec![Repo { id: "r1".to_string(), name: "Repo".to_string(), path: dir.to_str().unwrap().to_string(), active: true, added_at: "2024-01-01T00:00:00Z".to_string() }]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_optional_fields_error_and_image() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_opt_fields");
        write_meta(&dir, "p1", r#"{
            "status":"failed","platforms":["x"],
            "error":"Provider timed out",
            "image_url":"https://example.com/img.png",
            "llm_model":"claude-3-5-sonnet",
            "platform_results":{"x":"failed"}
        }"#);

        let state = make_state(vec![Repo { id: "r1".to_string(), name: "Repo".to_string(), path: dir.to_str().unwrap().to_string(), active: true, added_at: "2024-01-01T00:00:00Z".to_string() }]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].error.as_deref(), Some("Provider timed out"));
        assert_eq!(result[0].image_url.as_deref(), Some("https://example.com/img.png"));
        assert_eq!(result[0].llm_model.as_deref(), Some("claude-3-5-sonnet"));
        assert_eq!(result[0].platform_results.as_ref().unwrap().get("x").map(String::as_str), Some("failed"));
        let _ = fs::remove_dir_all(&dir);
    }
}
