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

pub fn dismiss_post_impl(repo_path: &str, post_folder: &str) -> Result<(), String> {
    let post_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder);
    let meta_path = post_path.join("meta.json");

    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    meta.status = "dismissed".to_string();

    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn dismiss_post(repo_path: String, post_folder: String) -> Result<(), String> {
    dismiss_post_impl(&repo_path, &post_folder)
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
    const VALID_PLATFORMS: &[&str] = &["x", "bluesky", "mastodon"];
    if !VALID_PLATFORMS.contains(&platform) {
        return Err(format!(
            "Invalid platform: '{}'. Must be one of: x, bluesky, mastodon",
            platform
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

    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

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

    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
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
    let tmp_path = postlane_dir.join("pending-redraft.json.tmp");

    let queued_at = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "post_folder": post_folder,
        "instruction": instruction,
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
    fn test_queue_redraft_overwrites_existing_pending() {
        let dir = std::env::temp_dir().join("postlane_test_queue_redraft_overwrites");
        fs::create_dir_all(&dir).expect("create test dir");
        let repos = make_repos_canonical(&[&dir]);

        // First write
        queue_redraft_impl(dir.to_str().unwrap(), "20260101-v100-changelog", "first instruction", &repos)
            .expect("first write should succeed");

        // Second write — should overwrite, not append
        queue_redraft_impl(dir.to_str().unwrap(), "20260201-v110-changelog", "second instruction", &repos)
            .expect("second write should succeed");

        let pending_path = dir.join(".postlane/pending-redraft.json");
        let content = std::fs::read_to_string(&pending_path).expect("read pending-redraft.json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

        assert_eq!(parsed["post_folder"].as_str(), Some("20260201-v110-changelog"));
        assert_eq!(parsed["instruction"].as_str(), Some("second instruction"));

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
    fn test_cancel_redraft_rejects_unregistered_repo() {
        let dir = std::env::temp_dir().join("postlane_test_cancel_redraft_unregistered");
        let registered = std::env::temp_dir().join("postlane_test_cancel_registered_only");
        fs::create_dir_all(&registered).expect("create registered dir");
        // dir intentionally not created — canonicalize will fail
        let repos = make_repos_canonical(&[&registered]);

        let result = cancel_redraft_impl(dir.to_str().unwrap(), &repos);
        assert!(result.is_err(), "expected Err for unregistered repo");
    }
}
