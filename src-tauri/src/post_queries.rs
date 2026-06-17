// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::PostMeta;
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Scans a single repo's `.postlane/posts` directory and appends matching drafts.
fn collect_repo_drafts(repo_path: &str, out: &mut Vec<PostMeta>) {
    let posts_dir = PathBuf::from(repo_path).join(".postlane/posts");
    if !posts_dir.exists() {
        return;
    }
    let entries = match fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(e) => {
            log::error!("Failed to read posts dir {}: {}", posts_dir.display(), e);
            return;
        }
    };
    for entry in entries.flatten() {
        let post_folder = entry.path();
        if !post_folder.is_dir() {
            continue;
        }
        let meta_path = post_folder.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        match fs::read_to_string(&meta_path) {
            Ok(content) => match serde_json::from_str::<PostMeta>(&content) {
                Ok(meta) if meta.status == "ready" || meta.status == "failed" => out.push(meta),
                Ok(_) => {}
                Err(e) => log::warn!("Skipping malformed meta.json at {}: {}", meta_path.display(), e),
            },
            Err(e) => log::warn!("Failed to read {}: {}", meta_path.display(), e),
        }
    }
}

/// Returns all drafts with status `"ready"` or `"failed"` across all active repos.
/// Results are sorted: `"failed"` before `"ready"`, then by `created_at` descending.
pub fn get_drafts_impl(state: &AppState) -> Result<Vec<PostMeta>, String> {
    let repos = state.lock_repos()?;

    let mut all_drafts = Vec::new();
    for repo in &repos.repos {
        if repo.active {
            collect_repo_drafts(&repo.path, &mut all_drafts);
        }
    }

    all_drafts.sort_by(|a, b| {
        match (&a.status[..], &b.status[..]) {
            ("failed", "ready") => std::cmp::Ordering::Less,
            ("ready", "failed") => std::cmp::Ordering::Greater,
            _ => match (&b.created_at, &a.created_at) {
                (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });

    Ok(all_drafts)
}

/// Tauri command — returns all drafts across active repos.
#[tauri::command]
pub fn get_drafts(state: State<AppState>) -> Result<Vec<PostMeta>, String> {
    get_drafts_impl(&state)
}

/// Reads and returns the raw markdown content of a single platform file within a post folder.
/// Validates `platform` against the known allow-list before accessing the filesystem.
pub fn get_post_content_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
) -> Result<String, String> {
    const VALID_PLATFORMS: &[&str] = &[
        "x", "bluesky", "mastodon",
        "linkedin", "substack_notes", "substack", "product_hunt", "show_hn", "changelog",
    ];
    if !VALID_PLATFORMS.contains(&platform) {
        return Err(format!(
            "Invalid platform: '{}'. Must be one of: {}",
            platform,
            VALID_PLATFORMS.join(", ")
        ));
    }

    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err("Invalid post folder: must not contain path separators or '..'".to_string());
    }

    let file_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder)
        .join(format!("{}.md", platform));

    std::fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read post content at {}: {}", file_path.display(), e))
}

/// Tauri command — reads the markdown content for a specific platform within a post folder.
#[tauri::command]
pub fn get_post_content(
    repo_path: String,
    post_folder: String,
    platform: String,
) -> Result<String, String> {
    get_post_content_impl(&repo_path, &post_folder, &platform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_drafts_state(path: &str) -> (AppState, tempfile::TempDir) {
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(
            ReposConfig {
                version: 1, workspaces: vec![], repos: vec![Repo {
                    id: "r1".to_string(),
                    name: "test".to_string(),
                    path: path.to_string(),
                    active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                }],
            },
            _tmp_repos.path().join("repos.json"),
        );
        (state, _tmp_repos)
    }

    fn write_draft(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_drafts_sorts_failed_before_ready() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_draft(dir.path(), "r1", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);
        write_draft(dir.path(), "f1", r#"{"status":"failed","platforms":["x"],"created_at":"2026-04-19T00:00:00Z"}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[1].status, "ready");
    }

    #[test]
    fn test_get_drafts_sorts_by_created_at_descending() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_draft(dir.path(), "old", r#"{"status":"ready","platforms":["x"],"created_at":"2026-01-01T00:00:00Z"}"#);
        write_draft(dir.path(), "new", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result[0].created_at.as_deref() > result[1].created_at.as_deref());
    }

    #[test]
    fn test_get_drafts_none_created_at_sorts_before_timestamped() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_draft(dir.path(), "with-ts", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);
        write_draft(dir.path(), "no-ts", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result[0].created_at.is_none());
        assert!(result[1].created_at.is_some());
    }

    #[test]
    fn test_get_drafts_two_none_created_at_stable() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_draft(dir.path(), "a", r#"{"status":"ready","platforms":["x"]}"#);
        write_draft(dir.path(), "b", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.created_at.is_none()));
    }

    #[test]
    fn test_get_post_content_rejects_invalid_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let result = get_post_content_impl(dir.path().to_str().unwrap(), "my-post", "twitter");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid platform"));
    }

    #[test]
    fn test_get_post_content_accepts_substack_notes() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let result = get_post_content_impl(dir.path().to_str().unwrap(), "my-post", "substack_notes");
        let err = result.unwrap_err();
        assert!(!err.contains("Invalid platform"), "got: {}", err);
    }

    #[test]
    fn test_no_url_suppression_for_linkedin() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create dir");

        let long_url = format!("https://example.com/{}", "a".repeat(30));
        let content = format!("Check this out {}", long_url);
        fs::write(post_dir.join("linkedin.md"), &content).expect("write linkedin.md");

        let result = get_post_content_impl(dir.path().to_str().unwrap(), "my-post", "linkedin")
            .expect("linkedin is a valid platform and file exists");

        assert_eq!(result, content, "content must be returned verbatim");
        assert!(result.contains(&long_url), "full URL must be present");
        assert!(!result.contains(&"x".repeat(23)), "URL must not have been replaced");
    }

    #[test]
    fn test_get_drafts_skips_non_dir_entries() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join(".postlane/posts");
        fs::create_dir_all(&posts_dir).expect("create posts dir");
        // Write a plain file (not a directory) inside posts/
        fs::write(posts_dir.join("not-a-dir.json"), "noise").expect("write file");
        // Also write a valid draft so we know scanning continues
        write_draft(dir.path(), "real-post", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "non-dir entry must be skipped");
    }

    #[test]
    fn test_get_drafts_skips_post_folder_without_meta_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join(".postlane/posts");
        // Create a directory with no meta.json
        fs::create_dir_all(posts_dir.join("no-meta")).expect("create no-meta dir");
        // Valid draft to confirm scanning continues
        write_draft(dir.path(), "real-post", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "folder without meta.json must be skipped");
    }

    #[test]
    fn test_get_drafts_skips_malformed_meta_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Write malformed JSON
        let bad_dir = dir.path().join(".postlane/posts/bad-meta");
        fs::create_dir_all(&bad_dir).expect("create dir");
        fs::write(bad_dir.join("meta.json"), "this is not json").expect("write bad meta");
        // Valid draft alongside
        write_draft(dir.path(), "good-post", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "malformed meta.json must be skipped");
    }

    #[test]
    fn test_get_drafts_skips_post_with_other_status() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Post with status "sent" should be excluded
        write_draft(dir.path(), "sent-post", r#"{"status":"sent","platforms":["x"]}"#);
        write_draft(dir.path(), "ready-post", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "only ready/failed statuses are returned");
        assert_eq!(result[0].status, "ready");
    }

    #[test]
    fn test_get_drafts_empty_when_no_posts_dir() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // No .postlane/posts directory at all
        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "missing posts dir must yield empty result");
    }

    #[test]
    fn test_get_post_content_rejects_path_traversal_in_post_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let result = get_post_content_impl(dir.path().to_str().unwrap(), "../etc", "x");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("path separators") || msg.contains("'..'"), "got: {}", msg);
    }

    #[test]
    fn test_sort_none_some_both_arms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Two posts: one with created_at, two without (exercises both None/Some arms)
        write_draft(dir.path(), "p1", r#"{"status":"ready","platforms":["x"],"created_at":"2026-01-01T00:00:00Z"}"#);
        write_draft(dir.path(), "p2", r#"{"status":"ready","platforms":["x"]}"#);
        write_draft(dir.path(), "p3", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        // The two None entries sort before the Some entry
        assert_eq!(result.len(), 3);
        // At least both None entries come before or after Some entry, not mixed
        let none_count = result.iter().filter(|p| p.created_at.is_none()).count();
        let some_count = result.iter().filter(|p| p.created_at.is_some()).count();
        assert_eq!(none_count, 2);
        assert_eq!(some_count, 1);
    }

    /// When the posts directory exists as a file rather than a directory,
    /// fs::read_dir fails and collect_repo_drafts returns without adding anything
    /// (lines 17-19 in post_queries.rs).
    #[test]
    fn test_get_drafts_returns_empty_when_posts_dir_is_a_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let dot_postlane = dir.path().join(".postlane");
        fs::create_dir_all(&dot_postlane).expect("create .postlane dir");
        // Place a plain file where .postlane/posts would be → read_dir returns Err
        fs::write(dot_postlane.join("posts"), b"not a directory").expect("write posts as file");

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "posts path as file must yield empty result");
    }

    /// When meta.json exists as a directory rather than a file, fs::read_to_string
    /// fails and collect_repo_drafts logs a warning and skips that entry
    /// (line 37 in post_queries.rs).
    #[test]
    fn test_get_drafts_skips_meta_json_when_it_is_a_directory() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // meta.json as a directory → fs::read_to_string returns Err
        let meta_as_dir = dir.path().join(".postlane/posts/bad-post/meta.json");
        fs::create_dir_all(&meta_as_dir).expect("create meta.json as directory");
        // Also a valid post to confirm scanning continues past the broken entry
        write_draft(dir.path(), "good-post", r#"{"status":"ready","platforms":["x"]}"#);

        let (state, _tmp_repos) = make_drafts_state(dir.path().to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "unreadable meta.json must be skipped");
        assert_eq!(result[0].status, "ready");
    }
}
