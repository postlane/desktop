// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::PostMeta;
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
