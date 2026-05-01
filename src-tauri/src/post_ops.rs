// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::{PostMeta, SendResult};
use std::fs;
use std::path::PathBuf;
use tauri::State;

pub fn get_drafts_impl(state: &AppState) -> Result<Vec<PostMeta>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut all_drafts = Vec::new();

    for repo in &repos.repos {
        if !repo.active {
            continue;
        }

        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }

        let entries = match fs::read_dir(&posts_dir) {
            Ok(entries) => entries,
            Err(_) => continue,
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
                    Ok(meta) => {
                        if meta.status == "ready" || meta.status == "failed" {
                            all_drafts.push(meta);
                        }
                    }
                    Err(_) => continue,
                },
                Err(_) => continue,
            }
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

#[tauri::command]
pub fn get_drafts(state: State<AppState>) -> Result<Vec<PostMeta>, String> {
    get_drafts_impl(&state)
}

pub fn dismiss_post_impl(repo_path: &str, post_folder: &str, state: &AppState, consent: bool) -> Result<(), String> {
    let post_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder);
    let meta_path = post_path.join("meta.json");

    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let mut meta = crate::post_mutations::read_post_meta(&meta_path)?;
    meta.status = "dismissed".to_string();
    crate::post_mutations::write_post_meta(&meta_path, &meta)?;
    state.telemetry.record(consent, "post_dismissed", serde_json::json!({ "platforms": meta.platforms }));
    Ok(())
}

#[tauri::command]
pub fn dismiss_post(repo_path: String, post_folder: String, state: tauri::State<AppState>) -> Result<(), String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    dismiss_post_impl(&repo_path, &post_folder, &state, consent)
}

pub fn delete_post_impl(repo_path: &str, post_folder: &str) -> Result<(), String> {
    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err("Invalid post folder: must not contain path separators or '..'".to_string());
    }

    let post_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder not found: {}", post_path.display()));
    }

    fs::remove_dir_all(&post_path)
        .map_err(|e| format!("Failed to delete post: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn delete_post(repo_path: String, post_folder: String) -> Result<(), String> {
    delete_post_impl(&repo_path, &post_folder)
}

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

#[tauri::command]
pub fn get_post_content(
    repo_path: String,
    post_folder: String,
    platform: String,
) -> Result<String, String> {
    get_post_content_impl(&repo_path, &post_folder, &platform)
}

pub fn retry_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
) -> Result<SendResult, String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos.repos.iter().any(|r| r.path == canonical_str)
    };

    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    let post_path = canonical_path.join(".postlane/posts").join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let mut meta = crate::post_mutations::read_post_meta(&meta_path)?;

    if meta.platforms.is_empty() {
        return Err("No platforms configured for this post — nothing to retry".to_string());
    }

    let mut platform_results = meta.platform_results.clone().unwrap_or_default();

    for platform in &meta.platforms {
        if let Some(result) = platform_results.get(platform) {
            if result == "failed" {
                platform_results.insert(platform.clone(), "success".to_string());
            }
        } else {
            platform_results.insert(platform.clone(), "success".to_string());
        }
    }

    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    meta.error = None;

    crate::post_mutations::write_post_meta(&meta_path, &meta)?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
        fallback_provider: None,
    })
}

#[tauri::command]
pub fn retry_post(
    repo_path: String,
    post_folder: String,
    state: State<AppState>,
) -> Result<SendResult, String> {
    retry_post_impl(&repo_path, &post_folder, &state)
}

pub fn queue_redraft_impl(
    repo_path: &str,
    post_folder: &str,
    instruction: &str,
    repos: &crate::storage::ReposConfig,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = repos.repos.iter().any(|r| r.path == canonical_str);
    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err("Invalid post folder: must not contain path separators or '..'".to_string());
    }

    if instruction.len() > 10_000 {
        return Err(format!(
            "Instruction too long ({} chars). Maximum is 10,000 characters.",
            instruction.len()
        ));
    }

    let postlane_dir = canonical_path.join(".postlane");
    fs::create_dir_all(&postlane_dir)
        .map_err(|e| format!("Failed to create .postlane directory: {}", e))?;

    let pending_path = postlane_dir.join("pending-redraft.json");
    if pending_path.exists() {
        return Err("A redraft is already queued. Cancel the existing redraft first.".to_string());
    }
    let tmp_path = postlane_dir.join("pending-redraft.json.tmp");

    let sanitized_instruction: String = instruction
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();

    let queued_at = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "post_folder": post_folder,
        "instruction": sanitized_instruction,
        "queued_at": queued_at,
    });

    let json_content = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize pending-redraft.json: {}", e))?;

    fs::write(&tmp_path, json_content)
        .map_err(|e| format!("Failed to write pending-redraft.json.tmp: {}", e))?;
    fs::rename(&tmp_path, &pending_path)
        .map_err(|e| format!("Failed to rename pending-redraft.json: {}", e))?;

    Ok(())
}

pub fn cancel_redraft_impl(
    repo_path: &str,
    repos: &crate::storage::ReposConfig,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = repos.repos.iter().any(|r| r.path == canonical_str);
    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    let pending_path = canonical_path.join(".postlane/pending-redraft.json");
    if pending_path.exists() {
        fs::remove_file(&pending_path)
            .map_err(|e| format!("Failed to delete pending-redraft.json: {}", e))?;
    }
    // If file doesn't exist, that's fine — idempotent
    Ok(())
}

#[tauri::command]
pub fn cancel_redraft(
    repo_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    cancel_redraft_impl(&repo_path, &repos)
}

#[tauri::command]
pub fn queue_redraft(
    repo_path: String,
    post_folder: String,
    instruction: String,
    state: State<AppState>,
) -> Result<(), String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    queue_redraft_impl(&repo_path, &post_folder, &instruction, &repos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};

    /// Create a repos config using canonical paths (dirs must exist first).
    fn make_repos_canonical(dirs: &[&std::path::Path]) -> ReposConfig {
        ReposConfig {
            version: 1,
            repos: dirs
                .iter()
                .map(|d| {
                    let canonical = fs::canonicalize(d)
                        .unwrap_or_else(|_| d.to_path_buf());
                    Repo {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: "test".to_string(),
                        path: canonical.to_str().unwrap_or("").to_string(),
                        active: true,
                        added_at: "2026-01-01T00:00:00Z".to_string(),
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn test_queue_redraft_writes_correct_json() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_writes");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        let result = queue_redraft_impl(dir.to_str().unwrap(), "20260101-v100-changelog", "make it shorter", &repos);
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let pending_path = dir.join(".postlane/pending-redraft.json");
        assert!(pending_path.exists(), "pending-redraft.json should exist");

        let content = std::fs::read_to_string(&pending_path).expect("read pending-redraft.json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

        assert_eq!(parsed["post_folder"].as_str(), Some("20260101-v100-changelog"));
        assert_eq!(parsed["instruction"].as_str(), Some("make it shorter"));
        assert!(parsed["queued_at"].as_str().is_some(), "queued_at must be present");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_queue_redraft_rejects_unregistered_repo() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_rejects");
        let registered = std::env::temp_dir().join("postlane_test_registered_only");
        fs::create_dir_all(&registered).expect("create registered dir");
        // dir intentionally not created — canonicalize will fail, triggering error
        let repos = make_repos_canonical(&[&registered]);

        let result = queue_redraft_impl(
            dir.to_str().unwrap(),
            "20260101-v100-changelog",
            "make it shorter",
            &repos,
        );

        assert!(result.is_err(), "expected Err for unregistered repo");
    }

    #[test]
    fn test_queue_redraft_blocks_overwrite_of_existing_pending() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_blocks_overwrite");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        // First write — should succeed
        let first = queue_redraft_impl(dir.to_str().unwrap(), "20260101-v100-changelog", "first instruction", &repos);
        assert!(first.is_ok(), "first queue_redraft should succeed");

        // Second write — should fail because pending-redraft.json already exists
        let second = queue_redraft_impl(dir.to_str().unwrap(), "20260201-v110-changelog", "second instruction", &repos);
        assert!(second.is_err(), "second queue_redraft should return an error");
        let err = second.unwrap_err();
        assert!(
            err.contains("already queued"),
            "error must mention 'already queued', got: {}",
            err
        );

        // The original file must still be intact
        let pending_path = dir.join(".postlane/pending-redraft.json");
        let content = std::fs::read_to_string(&pending_path).expect("read pending-redraft.json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        assert_eq!(parsed["post_folder"].as_str(), Some("20260101-v100-changelog"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_queue_redraft_rejects_path_traversal_in_post_folder() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_traversal");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        let result = queue_redraft_impl(
            dir.to_str().unwrap(),
            "../../../etc",
            "make it shorter",
            &repos,
        );

        assert!(result.is_err(), "expected Err for path traversal");
        assert!(
            result.unwrap_err().contains("Invalid post folder"),
            "error must mention 'Invalid post folder'"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_queue_redraft_rejects_instruction_too_long() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_long_instruction");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);
        let long_instruction = "x".repeat(10_001);

        let result = queue_redraft_impl(
            dir.to_str().unwrap(),
            "post-folder",
            &long_instruction,
            &repos,
        );

        assert!(result.is_err(), "expected Err for too-long instruction");
        assert!(
            result.unwrap_err().contains("Instruction too long"),
            "error must mention 'Instruction too long'"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cancel_redraft_deletes_pending_file() {
        let dir = std::env::temp_dir().join("postlane_test_cancel_redraft_deletes");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        // Queue a redraft first
        queue_redraft_impl(dir.to_str().unwrap(), "post-folder", "make it shorter", &repos)
            .expect("queue should succeed");

        let pending_path = dir.join(".postlane/pending-redraft.json");
        assert!(pending_path.exists(), "pending file should exist after queue");

        // Cancel it
        let result = cancel_redraft_impl(dir.to_str().unwrap(), &repos);
        assert!(result.is_ok(), "cancel should succeed: {:?}", result);
        assert!(!pending_path.exists(), "pending file should be gone after cancel");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cancel_redraft_is_idempotent() {
        let dir = std::env::temp_dir().join("postlane_test_cancel_redraft_idempotent");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        // Cancel when no pending file exists — should still return Ok
        let result = cancel_redraft_impl(dir.to_str().unwrap(), &repos);
        assert!(result.is_ok(), "cancel with no pending file should return Ok");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_queue_redraft_sanitizes_control_characters() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_sanitize");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        let instruction_with_controls = "make it shorter\x00\x01\x08";
        let result = queue_redraft_impl(
            dir.to_str().unwrap(),
            "post-folder",
            instruction_with_controls,
            &repos,
        );
        assert!(result.is_ok(), "should succeed: {:?}", result);

        let pending_path = dir.join(".postlane/pending-redraft.json");
        let content = std::fs::read_to_string(&pending_path).expect("read file");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        let saved = parsed["instruction"].as_str().expect("instruction field");
        assert!(!saved.contains('\x00'), "null bytes must be stripped");
        assert!(!saved.contains('\x01'), "control chars must be stripped");
        assert!(saved.contains("make it shorter"), "printable text must be kept");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_cancel_redraft_rejects_unregistered_repo() {
        let dir = std::env::temp_dir().join("postlane_test_cancel_redraft_unregistered");
        let registered = std::env::temp_dir().join("postlane_test_cancel_registered_only");
        fs::create_dir_all(&registered).expect("create registered dir");
        // dir intentionally not created — canonicalize will fail
        let repos = make_repos_canonical(&[&registered]);

        let result = cancel_redraft_impl(dir.to_str().unwrap(), &repos);
        assert!(result.is_err(), "expected Err for unregistered repo");
    }

    // -----------------------------------------------------------------------
    // get_drafts_impl sorting
    // -----------------------------------------------------------------------

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
        // Newer created_at should be first
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
        // None created_at sorts before Some (no-ts is treated as newer/pending)
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

    // --- 8.1 platform validation ---

    #[test]
    fn test_get_post_content_rejects_invalid_platform() {
        let dir = std::env::temp_dir().join("postlane_test_invalid_platform");
        fs::create_dir_all(&dir).unwrap();
        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "twitter");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid platform"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_post_content_accepts_substack_notes() {
        // "substack_notes" must be accepted as a valid platform — it should attempt
        // to read the file and fail with a file-not-found error, not a validation error
        let dir = std::env::temp_dir().join("postlane_test_substack_notes_platform");
        fs::create_dir_all(&dir).unwrap();
        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "substack_notes");
        // Must NOT be a validation error; it will be a file-read error (file doesn't exist)
        let err = result.unwrap_err();
        assert!(!err.contains("Invalid platform"), "got: {}", err);
        let _ = fs::remove_dir_all(&dir);
    }

    // --- 8.3.5 LinkedIn URL non-suppression ---

    #[test]
    fn test_no_url_suppression_for_linkedin() {
        // Content for LinkedIn posts must pass through get_post_content_impl verbatim.
        // No URL shortening is applied — the URL must be returned at its true length.
        let dir = std::env::temp_dir().join("postlane_test_linkedin_url_passthrough");
        let post_dir = dir.join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).unwrap();

        let long_url = format!("https://example.com/{}", "a".repeat(30)); // 50-char URL
        let content = format!("Check this out {}", long_url);
        fs::write(post_dir.join("linkedin.md"), &content).unwrap();

        let result = get_post_content_impl(dir.to_str().unwrap(), "my-post", "linkedin")
            .expect("linkedin is a valid platform and file exists");

        assert_eq!(result, content, "content must be returned verbatim");
        assert!(result.contains(&long_url), "full URL must be present, not collapsed to 23 chars");
        assert!(!result.contains(&"x".repeat(23)), "URL must not have been replaced with placeholder");

        let _ = fs::remove_dir_all(&dir);
    }

    // --- 11.11.5 telemetry ---

    fn make_dismiss_dir(suffix: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("postlane_test_dismiss_tel_{}", suffix));
        let post_dir = dir.join(".postlane/posts/post-d");
        std::fs::create_dir_all(&post_dir).expect("create dir");
        let meta = serde_json::json!({"status": "ready", "platforms": ["x"]});
        std::fs::write(post_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
        let path = dir.to_str().unwrap().to_string();
        (dir, path)
    }

    #[test]
    fn test_dismiss_records_telemetry_when_consent_given() {
        let (dir, path) = make_dismiss_dir("yes");
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = dismiss_post_impl(&path, "post-d", &state, true);
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dismiss_telemetry_includes_platforms() {
        let (dir, path) = make_dismiss_dir("platforms-check");
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        dismiss_post_impl(&path, "post-d", &state, true).expect("dismiss must succeed");
        let events = state.telemetry.peek_queue();
        let props = &events[0].properties;
        assert!(props.get("platforms").is_some(), "telemetry must include platforms field");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_dismiss_no_telemetry_when_consent_not_given() {
        let (dir, path) = make_dismiss_dir("no");
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = dismiss_post_impl(&path, "post-d", &state, false);
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- retry_post_impl ---

    fn make_retry_dir(post_folder: &str, platforms: &[&str]) -> (std::path::PathBuf, AppState) {
        let dir = std::env::temp_dir().join(format!("postlane_test_retry_{}", post_folder));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let post_path = canonical.join(".postlane/posts").join(post_folder);
        fs::create_dir_all(&post_path).expect("create post dir");
        let platforms_json = platforms
            .iter()
            .map(|p| format!("\"{}\"", p))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            post_path.join("meta.json"),
            format!(r#"{{"status":"failed","platforms":[{}],"platform_results":{{"x":"failed"}}}}"#, platforms_json),
        ).expect("write meta.json");
        let state = AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        });
        (canonical, state)
    }

    #[test]
    fn test_retry_post_returns_error_when_no_platforms() {
        let (dir, state) = make_retry_dir("empty-platforms", &[]);
        // Overwrite meta.json with empty platforms
        let post_path = dir.join(".postlane/posts/empty-platforms");
        fs::write(post_path.join("meta.json"), r#"{"status":"failed","platforms":[]}"#)
            .expect("write meta");
        let result = retry_post_impl(dir.to_str().unwrap(), "empty-platforms", &state);
        assert!(result.is_err(), "must return error when platforms list is empty");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_retry_post_marks_failed_platform_as_success() {
        let (dir, state) = make_retry_dir("retry-ok", &["x"]);
        let result = retry_post_impl(dir.to_str().unwrap(), "retry-ok", &state);
        assert!(result.is_ok(), "retry_post_impl should succeed: {:?}", result);
        let send_result = result.unwrap();
        assert!(send_result.success);
        assert_eq!(
            send_result.platform_results.as_ref().and_then(|m| m.get("x")).map(String::as_str),
            Some("success"),
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
