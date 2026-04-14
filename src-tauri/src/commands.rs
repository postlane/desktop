// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::{PostMeta, SendResult};
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Get all draft posts (status === "ready" or "failed") across all active repos
/// This is the testable implementation
pub fn get_drafts_impl(state: &AppState) -> Result<Vec<PostMeta>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut all_drafts = Vec::new();

    for repo in &repos.repos {
        // Skip inactive repos
        if !repo.active {
            continue;
        }

        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");

        // Skip if posts directory doesn't exist
        if !posts_dir.exists() {
            continue;
        }

        // Read all post folders
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

            // Read and parse meta.json
            match fs::read_to_string(&meta_path) {
                Ok(content) => match serde_json::from_str::<PostMeta>(&content) {
                    Ok(meta) => {
                        // Only include ready or failed posts
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

    // Sort: failed posts first, then by created_at (most recent first)
    all_drafts.sort_by(|a, b| {
        // First, sort by status (failed before ready)
        match (&a.status[..], &b.status[..]) {
            ("failed", "ready") => std::cmp::Ordering::Less,
            ("ready", "failed") => std::cmp::Ordering::Greater,
            _ => {
                // Same status - sort by created_at (most recent first)
                match (&b.created_at, &a.created_at) {
                    (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        }
    });

    Ok(all_drafts)
}

/// Tauri command wrapper for get_drafts
#[tauri::command]
pub fn get_drafts(state: State<AppState>) -> Result<Vec<PostMeta>, String> {
    get_drafts_impl(&state)
}

/// Approve and send a post
/// This is the testable implementation
pub fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
) -> Result<SendResult, String> {
    // Step 1: Canonicalize repo_path
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    // Step 2: Validate repo_path is in repos.json
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = repos.repos.iter().any(|r| r.path == canonical_str);

    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    // Step 3: Validate post folder
    let post_path = canonical_path
        .join(".postlane/posts")
        .join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    // Read current meta.json
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    // Step 4: Call scheduling provider (stub for Milestone 3)
    // In Milestone 4, this will call the real provider
    // For now, simulate success
    let mut platform_results = std::collections::HashMap::new();
    for platform in &meta.platforms {
        platform_results.insert(platform.clone(), "success".to_string());
    }

    // Step 5: Update meta.json with results
    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    fs::write(&temp_path, serde_json::to_string_pretty(&meta).unwrap())
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
    })
}

/// Tauri command wrapper for approve_post
#[tauri::command]
pub fn approve_post(
    repo_path: String,
    post_folder: String,
    state: State<AppState>,
) -> Result<SendResult, String> {
    approve_post_impl(&repo_path, &post_folder, &state)
}

/// Dismiss a post
/// This is the testable implementation
pub fn dismiss_post_impl(
    repo_path: &str,
    post_folder: &str,
) -> Result<(), String> {
    let repo_pathbuf = PathBuf::from(repo_path);
    let post_path = repo_pathbuf.join(".postlane/posts").join(post_folder);
    let meta_path = post_path.join("meta.json");

    // Check meta.json exists
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    // Read current meta.json
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    // Update status to dismissed
    meta.status = "dismissed".to_string();

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    fs::write(&temp_path, serde_json::to_string_pretty(&meta).unwrap())
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(())
}

/// Tauri command wrapper for dismiss_post
#[tauri::command]
pub fn dismiss_post(
    repo_path: String,
    post_folder: String,
) -> Result<(), String> {
    dismiss_post_impl(&repo_path, &post_folder)
}
