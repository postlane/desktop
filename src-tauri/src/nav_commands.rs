// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{AppState, AppStateFile, read_app_state, write_app_state};
use serde::Serialize;

/// Payload emitted on the "meta-changed" Tauri event
#[derive(Serialize, Clone, Debug)]
pub struct MetaChangedPayload {
    pub repo_id: String,
    pub post_folder: String,
}
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Repo with runtime-computed fields for the nav component
#[derive(Serialize, Clone, Debug)]
pub struct RepoWithStatus {
    pub id: String,
    pub name: String,
    pub path: String,
    pub active: bool,
    pub added_at: String,
    /// Whether the repo path currently exists on disk
    pub path_exists: bool,
    /// Count of posts with status "ready"
    pub ready_count: u32,
    /// Count of posts with status "failed"
    pub failed_count: u32,
    /// ISO 8601 timestamp of the most recent post created_at, or None
    pub last_post_at: Option<String>,
}

/// Testable implementation: builds RepoWithStatus for all repos in AppState
pub fn get_repos_impl(state: &AppState) -> Result<Vec<RepoWithStatus>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let result = repos
        .repos
        .iter()
        .map(|repo| {
            let path_exists = std::path::Path::new(&repo.path).exists();
            let (ready_count, failed_count, last_post_at) =
                scan_post_statuses(&repo.path);
            RepoWithStatus {
                id: repo.id.clone(),
                name: repo.name.clone(),
                path: repo.path.clone(),
                active: repo.active,
                added_at: repo.added_at.clone(),
                path_exists,
                ready_count,
                failed_count,
                last_post_at,
            }
        })
        .collect();

    Ok(result)
}

/// Scans a repo's posts directory and returns (ready_count, failed_count, last_post_at).
/// Returns (0, 0, None) if the posts directory does not exist or cannot be read.
fn scan_post_statuses(repo_path: &str) -> (u32, u32, Option<String>) {
    let posts_dir = PathBuf::from(repo_path).join(".postlane/posts");
    if !posts_dir.exists() {
        return (0, 0, None);
    }

    let entries = match fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(_) => return (0, 0, None),
    };

    let mut ready_count: u32 = 0;
    let mut failed_count: u32 = 0;
    let mut latest_ts: Option<String> = None;

    for entry in entries.flatten() {
        let meta_path = entry.path().join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match meta.get("status").and_then(|s| s.as_str()) {
            Some("ready") => ready_count += 1,
            Some("failed") => failed_count += 1,
            _ => {}
        }

        if let Some(ts) = meta.get("created_at").and_then(|v| v.as_str()) {
            latest_ts = Some(match &latest_ts {
                None => ts.to_string(),
                Some(prev) => {
                    if ts > prev.as_str() {
                        ts.to_string()
                    } else {
                        prev.clone()
                    }
                }
            });
        }
    }

    (ready_count, failed_count, latest_ts)
}

#[tauri::command]
pub fn get_repos(state: State<'_, AppState>) -> Result<Vec<RepoWithStatus>, String> {
    get_repos_impl(&state)
}

#[tauri::command]
pub fn read_app_state_command() -> AppStateFile {
    read_app_state()
}

#[tauri::command]
pub fn save_app_state_command(state: AppStateFile) -> Result<(), String> {
    write_app_state(&state)
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

    #[test]
    fn test_get_repos_empty() {
        let state = make_state(vec![]);
        let result = get_repos_impl(&state).expect("should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_repos_nonexistent_path_marked_missing() {
        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: "/nonexistent/path/that/cannot/exist".to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repos_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 1);
        assert!(!result[0].path_exists);
        assert_eq!(result[0].ready_count, 0);
        assert_eq!(result[0].failed_count, 0);
        assert!(result[0].last_post_at.is_none());
    }

    #[test]
    fn test_get_repos_counts_ready_and_failed_posts() {
        let dir = std::env::temp_dir().join("postlane_test_get_repos_counts");
        let posts_dir = dir.join(".postlane/posts");

        // post 1: ready
        let p1 = posts_dir.join("post-001");
        fs::create_dir_all(&p1).expect("create post dir");
        fs::write(
            p1.join("meta.json"),
            r#"{"status":"ready","created_at":"2024-06-01T10:00:00Z"}"#,
        )
        .expect("write meta");

        // post 2: failed
        let p2 = posts_dir.join("post-002");
        fs::create_dir_all(&p2).expect("create post dir");
        fs::write(
            p2.join("meta.json"),
            r#"{"status":"failed","created_at":"2024-06-02T10:00:00Z"}"#,
        )
        .expect("write meta");

        // post 3: sent (should not count)
        let p3 = posts_dir.join("post-003");
        fs::create_dir_all(&p3).expect("create post dir");
        fs::write(
            p3.join("meta.json"),
            r#"{"status":"sent","created_at":"2024-06-03T10:00:00Z"}"#,
        )
        .expect("write meta");

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repos_impl(&state).expect("should succeed");
        assert_eq!(result[0].ready_count, 1);
        assert_eq!(result[0].failed_count, 1);
        // latest created_at across all three posts
        assert_eq!(
            result[0].last_post_at.as_deref(),
            Some("2024-06-03T10:00:00Z")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_no_posts_dir() {
        let (ready, failed, ts) = scan_post_statuses("/nonexistent/path");
        assert_eq!(ready, 0);
        assert_eq!(failed, 0);
        assert!(ts.is_none());
    }

    #[test]
    fn test_scan_malformed_meta_skipped() {
        let dir = std::env::temp_dir().join("postlane_test_scan_malformed");
        let posts_dir = dir.join(".postlane/posts/post-bad");
        fs::create_dir_all(&posts_dir).expect("create dir");
        fs::write(posts_dir.join("meta.json"), "{ not valid json }").expect("write");

        let (ready, failed, ts) =
            scan_post_statuses(dir.to_str().unwrap());
        assert_eq!(ready, 0);
        assert_eq!(failed, 0);
        assert!(ts.is_none());

        let _ = fs::remove_dir_all(&dir);
    }
}
