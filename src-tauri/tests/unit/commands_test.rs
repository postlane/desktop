// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::app_state::AppState;
use postlane_desktop_lib::commands::get_drafts_impl;
use postlane_desktop_lib::storage::{Repo, ReposConfig};
use postlane_desktop_lib::types::PostMeta;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod get_drafts_tests {
    use super::*;

    #[test]
    fn test_get_drafts_returns_ready_and_failed_posts() {
        // Setup: Create temp directory with repos
        let temp_dir = TempDir::new().unwrap();
        let repo1_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo1_path).unwrap();
        fs::create_dir_all(repo1_path.join(".postlane/posts")).unwrap();

        // Create a ready post
        let ready_post_dir = repo1_path.join(".postlane/posts/post1");
        fs::create_dir_all(&ready_post_dir).unwrap();
        let ready_meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            sent_at: None,
        };
        fs::write(
            ready_post_dir.join("meta.json"),
            serde_json::to_string(&ready_meta).unwrap(),
        )
        .unwrap();

        // Create a failed post
        let failed_post_dir = repo1_path.join(".postlane/posts/post2");
        fs::create_dir_all(&failed_post_dir).unwrap();
        let failed_meta = PostMeta {
            status: "failed".to_string(),
            platforms: vec!["bluesky".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            error: Some("Connection timeout".to_string()),
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-02T00:00:00Z".to_string()),
            sent_at: None,
        };
        fs::write(
            failed_post_dir.join("meta.json"),
            serde_json::to_string(&failed_meta).unwrap(),
        )
        .unwrap();

        // Create a sent post (should be excluded)
        let sent_post_dir = repo1_path.join(".postlane/posts/post3");
        fs::create_dir_all(&sent_post_dir).unwrap();
        let sent_meta = PostMeta {
            status: "sent".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-03T00:00:00Z".to_string()),
            sent_at: Some("2024-01-03T01:00:00Z".to_string()),
        };
        fs::write(
            sent_post_dir.join("meta.json"),
            serde_json::to_string(&sent_meta).unwrap(),
        )
        .unwrap();

        // Setup AppState
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo1_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let state = AppState::new(repos_config);

        // Test: Call get_drafts_impl
        let result = get_drafts_impl(&state);

        // Assert: Should return 2 posts (ready and failed), sorted with failed first
        assert!(result.is_ok());
        let drafts = result.unwrap();
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].status, "failed");
        assert_eq!(drafts[1].status, "ready");
    }

    #[test]
    fn test_get_drafts_excludes_inactive_repos() {
        // Setup: Create temp directory with inactive repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();
        fs::create_dir_all(repo_path.join(".postlane/posts")).unwrap();

        let post_dir = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_dir).unwrap();
        let meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            sent_at: None,
        };
        fs::write(
            post_dir.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: false, // Inactive repo
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let state = AppState::new(repos_config);

        // Test: Call get_drafts_impl
        let result = get_drafts_impl(&state);

        // Assert: Should return empty (inactive repo excluded)
        assert!(result.is_ok());
        let drafts = result.unwrap();
        assert_eq!(drafts.len(), 0);
    }

    #[test]
    fn test_get_drafts_handles_missing_posts_directory() {
        // Setup: Repo without .postlane/posts directory
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let state = AppState::new(repos_config);

        // Test: Call get_drafts_impl
        let result = get_drafts_impl(&state);

        // Assert: Should return empty list (not error)
        assert!(result.is_ok());
        let drafts = result.unwrap();
        assert_eq!(drafts.len(), 0);
    }
}
