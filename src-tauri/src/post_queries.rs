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
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

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

    fn make_drafts_state(path: &str) -> AppState {
        AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    fn write_draft(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_drafts_sorts_failed_before_ready() {
        let dir = std::env::temp_dir().join("postlane_test_get_drafts_sort_status");
        write_draft(&dir, "r1", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);
        write_draft(&dir, "f1", r#"{"status":"failed","platforms":["x"],"created_at":"2026-04-19T00:00:00Z"}"#);

        let state = make_drafts_state(dir.to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[1].status, "ready");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_drafts_sorts_by_created_at_descending() {
        let dir = std::env::temp_dir().join("postlane_test_get_drafts_sort_ts");
        write_draft(&dir, "old", r#"{"status":"ready","platforms":["x"],"created_at":"2026-01-01T00:00:00Z"}"#);
        write_draft(&dir, "new", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);

        let state = make_drafts_state(dir.to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result[0].created_at.as_deref() > result[1].created_at.as_deref());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_drafts_none_created_at_sorts_before_timestamped() {
        let dir = std::env::temp_dir().join("postlane_test_get_drafts_none_ts");
        write_draft(&dir, "with-ts", r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-20T00:00:00Z"}"#);
        write_draft(&dir, "no-ts", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_drafts_state(dir.to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result[0].created_at.is_none());
        assert!(result[1].created_at.is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_drafts_two_none_created_at_stable() {
        let dir = std::env::temp_dir().join("postlane_test_get_drafts_two_none");
        write_draft(&dir, "a", r#"{"status":"ready","platforms":["x"]}"#);
        write_draft(&dir, "b", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_drafts_state(dir.to_str().unwrap());
        let result = get_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.created_at.is_none()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_post_content_rejects_invalid_platform() {
        let dir = std::env::temp_dir().join("postlane_test_invalid_platform");
        fs::create_dir_all(&dir).expect("create dir");
        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "twitter");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid platform"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_post_content_accepts_substack_notes() {
        let dir = std::env::temp_dir().join("postlane_test_substack_notes_platform");
        fs::create_dir_all(&dir).expect("create dir");
        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "substack_notes");
        let err = result.unwrap_err();
        assert!(!err.contains("Invalid platform"), "got: {}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_no_url_suppression_for_linkedin() {
        let dir = std::env::temp_dir().join("postlane_test_linkedin_url_passthrough");
        let post_dir = dir.join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create dir");

        let long_url = format!("https://example.com/{}", "a".repeat(30));
        let content = format!("Check this out {}", long_url);
        fs::write(post_dir.join("linkedin.md"), &content).expect("write linkedin.md");

        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "linkedin")
            .expect("linkedin is a valid platform and file exists");

        assert_eq!(result, content, "content must be returned verbatim");
        assert!(result.contains(&long_url), "full URL must be present");
        assert!(!result.contains(&"x".repeat(23)), "URL must not have been replaced");

        let _ = fs::remove_dir_all(&dir);
    }
}
