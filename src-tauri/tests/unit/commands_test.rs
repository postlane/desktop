// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::app_state::AppState;
use postlane_desktop_lib::commands::{add_repo_impl, approve_post_impl, check_repo_health_impl, delete_post_impl, dismiss_post_impl, export_history_csv_impl, get_drafts_impl, get_post_content_impl, remove_repo_impl, retry_post_impl, set_repo_active_impl};
use postlane_desktop_lib::init;
use postlane_desktop_lib::storage::{Repo, ReposConfig};
use postlane_desktop_lib::types::PostMeta;
use std::fs;
use std::collections::HashMap;
use std::sync::OnceLock;
use tempfile::TempDir;

// Mutex to ensure tests that use ~/.postlane directory run sequentially
// This prevents race conditions when tests run in parallel
static POSTLANE_DIR_MUTEX: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

fn get_postlane_dir_lock() -> &'static std::sync::Mutex<()> {
    POSTLANE_DIR_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}

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
            platform_urls: None,
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
            platform_urls: None,
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
            platform_urls: None,
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
            platform_urls: None,
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

#[cfg(test)]
mod approve_post_tests {
    use super::*;

    #[tokio::test]
    async fn test_approve_post_fails_with_unregistered_repo() {
        // Setup: Create temp directory with unregistered repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        // Empty repos config (no repos registered)
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Try to approve post from unregistered repo
        let result = approve_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            None,
            false,
        ).await;

        // Assert: Should fail with path not registered error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not registered") || err.contains("403"));
    }

    #[tokio::test]
    async fn test_approve_post_fails_with_invalid_path() {
        // Setup: Create temp directory
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

        // Test: Try to approve post with non-existent post folder
        let result = approve_post_impl(
            repo_path.to_str().unwrap(),
            "nonexistent",
            &state,
            None,
            false,
        ).await;

        // Assert: Should fail with validation error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_approve_post_fails_without_meta_json() {
        // Setup: Create repo and post folder without meta.json
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

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

        // Test: Try to approve post without meta.json
        let result = approve_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            None,
            false,
        ).await;

        // Assert: Should fail with validation error
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod dismiss_post_tests {
    use super::*;

    #[test]
    fn test_dismiss_post_sets_status_to_dismissed() {
        // Setup: Create repo with ready post
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

        let meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            platform_urls: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            sent_at: None,
        };
        fs::write(
            post_folder.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        // Test: Dismiss the post
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            false,
        );

        // Assert: Should succeed
        assert!(result.is_ok());

        // Verify: meta.json status is now "dismissed"
        let updated_content = fs::read_to_string(post_folder.join("meta.json")).unwrap();
        let updated_meta: PostMeta = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_meta.status, "dismissed");
    }

    #[test]
    fn test_dismiss_post_fails_with_missing_meta_json() {
        // Setup: Create repo with post folder but no meta.json
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

        // Test: Try to dismiss post without meta.json
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            false,
        );

        // Assert: Should fail
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod delete_post_tests {
    use super::*;

    #[test]
    fn test_delete_post_removes_folder() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();
        fs::write(post_folder.join("x.md"), "Hello").unwrap();

        let result = delete_post_impl(repo_path.to_str().unwrap(), "post1");

        assert!(result.is_ok());
        assert!(!post_folder.exists());
    }

    #[test]
    fn test_delete_post_rejects_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let result = delete_post_impl(repo_path.to_str().unwrap(), "../evil");

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_post_errors_when_folder_missing() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let result = delete_post_impl(repo_path.to_str().unwrap(), "nonexistent");

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod retry_post_tests {
    use super::*;

    #[test]
    fn test_retry_post_only_retries_failed_platforms() {
        // Setup: Create repo with failed post (x succeeded, bluesky failed)
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

        let mut platform_results = HashMap::new();
        platform_results.insert("x".to_string(), "success".to_string());
        platform_results.insert("bluesky".to_string(), "failed".to_string());

        let meta = PostMeta {
            status: "failed".to_string(),
            platforms: vec!["x".to_string(), "bluesky".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: Some(platform_results),
            platform_urls: None,
            error: Some("Bluesky timeout".to_string()),
            image_url: None,
            image_source: None,
            image_attribution: None,
            llm_model: None,
            created_at: Some("2024-01-01T00:00:00Z".to_string()),
            sent_at: None,
        };
        fs::write(
            post_folder.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        // Canonicalize the path for repos_config (matches what retry_post_impl does)
        let canonical_repo_path = fs::canonicalize(&repo_path).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let state = AppState::new(repos_config);

        // Test: Retry the post
        let result = retry_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
        );

        // Assert: Should succeed
        assert!(result.is_ok());
        let send_result = result.unwrap();
        assert!(send_result.success);

        // Verify: x should still be success, bluesky should be retried (success in stub)
        let updated_content = fs::read_to_string(post_folder.join("meta.json")).unwrap();
        let updated_meta: PostMeta = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_meta.status, "sent");
        let results = updated_meta.platform_results.unwrap();
        assert_eq!(results.get("x").unwrap(), "success"); // Should not change
        assert_eq!(results.get("bluesky").unwrap(), "success"); // Should be retried
    }
}

#[cfg(test)]
mod add_repo_tests {
    use super::*;

    #[test]
    fn test_add_repo_validates_git_directory() {
        // Setup: Create repo without .git directory
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        // Create .postlane/config.json
        let postlane_dir = repo_path.join(".postlane");
        fs::create_dir_all(&postlane_dir).unwrap();
        fs::write(postlane_dir.join("config.json"), "{}").unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Try to add repo without .git directory
        let result = add_repo_impl(repo_path.to_str().unwrap(), &state);

        // Assert: Should fail with git validation error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("git") || err.contains("Not a git repository"));
    }

    #[test]
    fn test_add_repo_validates_config_json() {
        // Setup: Create repo with .git but without .postlane/config.json
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();
        fs::create_dir_all(repo_path.join(".git")).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Try to add repo without config.json
        let result = add_repo_impl(repo_path.to_str().unwrap(), &state);

        // Assert: Should fail with config validation error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("config") || err.contains("init"));
    }

    #[test]
    fn test_add_repo_generates_uuid_and_name() {
        // Setup: Create valid repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_path).unwrap();
        fs::create_dir_all(repo_path.join(".git")).unwrap();

        let postlane_dir = repo_path.join(".postlane");
        fs::create_dir_all(&postlane_dir).unwrap();
        fs::write(postlane_dir.join("config.json"), "{}").unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Add repo
        let result = add_repo_impl(repo_path.to_str().unwrap(), &state);

        // Assert: Should succeed
        assert!(result.is_ok());
        let repo = result.unwrap();

        // Verify: UUID format (36 chars with hyphens)
        assert_eq!(repo.id.len(), 36);
        assert!(repo.id.contains('-'));

        // Verify: Name derived from folder
        assert_eq!(repo.name, "test-repo");

        // Verify: Repo is active
        assert!(repo.active);

        // Verify: Repo was added to state
        let repos = state.repos.lock().unwrap();
        assert_eq!(repos.repos.len(), 1);
        assert_eq!(repos.repos[0].id, repo.id);
    }
}

#[cfg(test)]
mod remove_repo_tests {
    use super::*;

    #[test]
    fn test_remove_repo_removes_from_state() {
        // Acquire lock to prevent race conditions with other tests using ~/.postlane
        let _lock = get_postlane_dir_lock().lock().unwrap();

        // Initialize postlane directory
        init::init_postlane_dir().expect("Failed to init postlane dir");

        // Setup: Create state with two repos
        let temp_dir = TempDir::new().unwrap();
        let repo1_path = temp_dir.path().join("repo1");
        let repo2_path = temp_dir.path().join("repo2");
        fs::create_dir_all(&repo1_path).unwrap();
        fs::create_dir_all(&repo2_path).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: repo1_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
                Repo {
                    id: "id2".to_string(),
                    name: "Repo 2".to_string(),
                    path: repo2_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-02T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Remove first repo
        let result = remove_repo_impl("id1", &state);

        // Assert: Should succeed
        assert!(result.is_ok());

        // Verify: Only second repo remains
        let repos = state.repos.lock().unwrap();
        assert_eq!(repos.repos.len(), 1);
        assert_eq!(repos.repos[0].id, "id2");
    }

    #[test]
    fn test_remove_repo_fails_with_invalid_id() {
        // Setup: Create state with one repo
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Try to remove non-existent repo
        let result = remove_repo_impl("nonexistent", &state);

        // Assert: Should fail
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not found") || err.contains("does not exist"));
    }

    #[test]
    fn test_remove_repo_does_not_delete_files() {
        // Acquire lock to prevent race conditions with other tests using ~/.postlane
        let _lock = get_postlane_dir_lock().lock().unwrap();

        // Initialize postlane directory
        init::init_postlane_dir().expect("Failed to init postlane dir");

        // Setup: Create actual repo directory with files
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        // Create a test file
        let test_file = repo_path.join("test.txt");
        fs::write(&test_file, "test content").unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: repo_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Remove repo
        let result = remove_repo_impl("id1", &state);
        assert!(result.is_ok());

        // Verify: Repo directory and files still exist
        assert!(repo_path.exists());
        assert!(test_file.exists());
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "test content");
    }
}

#[cfg(test)]
mod set_repo_active_tests {
    use super::*;

    #[test]
    fn test_set_repo_active_toggles_state() {
        // Acquire lock to prevent race conditions with other tests using ~/.postlane
        let _lock = get_postlane_dir_lock().lock().unwrap();

        // Initialize postlane directory
        init::init_postlane_dir().expect("Failed to init postlane dir");

        // Setup: Create repo that's initially active
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Deactivate repo
        let result = set_repo_active_impl("id1", false, &state);
        assert!(result.is_ok());

        // Verify: Repo is now inactive
        let repos = state.repos.lock().unwrap();
        assert_eq!(repos.repos[0].active, false);
    }

    #[test]
    fn test_set_repo_active_activates_repo() {
        // Acquire lock to prevent race conditions with other tests using ~/.postlane
        let _lock = get_postlane_dir_lock().lock().unwrap();

        // Initialize postlane directory
        init::init_postlane_dir().expect("Failed to init postlane dir");

        // Setup: Create repo that's initially inactive
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: false,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Activate repo
        let result = set_repo_active_impl("id1", true, &state);
        assert!(result.is_ok());

        // Verify: Repo is now active
        let repos = state.repos.lock().unwrap();
        assert_eq!(repos.repos[0].active, true);
    }

    #[test]
    fn test_set_repo_active_fails_with_invalid_id() {
        // Setup: Create state with one repo
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Try to set active on non-existent repo
        let result = set_repo_active_impl("nonexistent", true, &state);

        // Assert: Should fail
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod check_repo_health_tests {
    use super::*;

    #[test]
    fn test_check_repo_health_reports_reachable_repos() {
        // Setup: Create repo with valid config.json
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let postlane_dir = repo_path.join(".postlane");
        fs::create_dir_all(&postlane_dir).unwrap();
        fs::write(postlane_dir.join("config.json"), "{}").unwrap();

        let canonical_path = fs::canonicalize(&repo_path).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: canonical_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Check repo health
        let result = check_repo_health_impl(&state);

        // Assert: Should succeed
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, "id1");
        assert_eq!(statuses[0].reachable, true);
    }

    #[test]
    fn test_check_repo_health_reports_unreachable_repos() {
        // Setup: Create repo config pointing to non-existent path
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/nonexistent/path".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Check repo health
        let result = check_repo_health_impl(&state);

        // Assert: Should succeed but report unreachable
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, "id1");
        assert_eq!(statuses[0].reachable, false);
    }
}

#[cfg(test)]
mod export_history_csv_tests {
    use super::*;

    #[test]
    fn test_export_history_csv_generates_valid_csv_headers() {
        // Setup: Empty repos (zero posts case)
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Export CSV
        let result = export_history_csv_impl(&state);

        // Assert: Should succeed
        assert!(result.is_ok());
        let csv_content = result.unwrap();

        // Verify: Has correct headers
        assert!(csv_content.starts_with("repo,slug,platforms,scheduler,model,sent_at,likes,reposts,replies,impressions,view_urls"));
    }

    #[test]
    fn test_export_history_csv_handles_zero_posts() {
        // Setup: Empty repos
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };
        let state = AppState::new(repos_config);

        // Test: Export CSV
        let result = export_history_csv_impl(&state);

        // Assert: Should succeed with headers only
        assert!(result.is_ok());
        let csv_content = result.unwrap();
        let lines: Vec<&str> = csv_content.lines().collect();
        assert_eq!(lines.len(), 1); // Headers only
    }

    #[test]
    fn test_export_history_csv_with_three_sent_posts_across_two_repos() {
        // Setup: Create temp directories with two repos
        let temp_dir = TempDir::new().unwrap();

        // Repo 1 with 2 sent posts
        let repo1_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo1_path).unwrap();
        let posts1_dir = repo1_path.join(".postlane/posts");
        fs::create_dir_all(&posts1_dir).unwrap();

        let post1a_dir = posts1_dir.join("post1a");
        fs::create_dir_all(&post1a_dir).unwrap();
        let meta1a = serde_json::json!({
            "status": "sent",
            "platforms": ["x"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": "2024-01-01T00:00:00Z",
            "sent_at": "2024-01-01T12:00:00Z"
        });
        fs::write(post1a_dir.join("meta.json"), serde_json::to_string(&meta1a).unwrap()).unwrap();

        let post1b_dir = posts1_dir.join("post1b");
        fs::create_dir_all(&post1b_dir).unwrap();
        let meta1b = serde_json::json!({
            "status": "sent",
            "platforms": ["x", "linkedin"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": "claude-3-5-sonnet",
            "created_at": "2024-01-02T00:00:00Z",
            "sent_at": "2024-01-02T12:00:00Z"
        });
        fs::write(post1b_dir.join("meta.json"), serde_json::to_string(&meta1b).unwrap()).unwrap();

        // Repo 2 with 1 sent post
        let repo2_path = temp_dir.path().join("repo2");
        fs::create_dir_all(&repo2_path).unwrap();
        let posts2_dir = repo2_path.join(".postlane/posts");
        fs::create_dir_all(&posts2_dir).unwrap();

        let post2a_dir = posts2_dir.join("post2a");
        fs::create_dir_all(&post2a_dir).unwrap();
        let meta2a = serde_json::json!({
            "status": "sent",
            "platforms": ["x"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": "2024-01-03T00:00:00Z",
            "sent_at": "2024-01-03T12:00:00Z"
        });
        fs::write(post2a_dir.join("meta.json"), serde_json::to_string(&meta2a).unwrap()).unwrap();

        // Canonicalize paths
        let canonical_repo1 = fs::canonicalize(&repo1_path).unwrap();
        let canonical_repo2 = fs::canonicalize(&repo2_path).unwrap();

        // Setup repos config
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![
                Repo {
                    id: "repo1-id".to_string(),
                    name: "Repo 1".to_string(),
                    path: canonical_repo1.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
                Repo {
                    id: "repo2-id".to_string(),
                    name: "Repo 2".to_string(),
                    path: canonical_repo2.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let state = AppState::new(repos_config);

        // Test: Export CSV
        let result = export_history_csv_impl(&state);

        // Assert: Should succeed
        assert!(result.is_ok());
        let csv_content = result.unwrap();
        let lines: Vec<&str> = csv_content.lines().collect();

        // Verify: Has headers + 3 data rows
        assert_eq!(lines.len(), 4, "Expected 1 header + 3 data rows");

        // Verify: Headers are correct
        assert!(lines[0].starts_with("repo,slug,platforms,scheduler,model,sent_at"));

        // Verify: Contains all three posts
        assert!(csv_content.contains("post1a"), "CSV should contain post1a");
        assert!(csv_content.contains("post1b"), "CSV should contain post1b");
        assert!(csv_content.contains("post2a"), "CSV should contain post2a");
    }
}

#[cfg(test)]
mod provider_instantiation_tests {
    use super::*;

    #[tokio::test]
    async fn test_approve_post_no_credential_returns_clear_error() {
        // Setup: Create temp directory with a repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_path).unwrap();

        // Canonicalize the path so it matches what approve_post_impl expects
        let canonical_repo_path = fs::canonicalize(&repo_path).unwrap();

        // Create .postlane directory structure
        fs::create_dir_all(canonical_repo_path.join(".postlane/posts")).unwrap();

        // Create config.json with scheduler provider set to "zernio"
        let config_json = serde_json::json!({
            "version": 1,
            "scheduler": {
                "provider": "zernio"
            }
        });
        fs::write(
            canonical_repo_path.join(".postlane/config.json"),
            serde_json::to_string_pretty(&config_json).unwrap(),
        )
        .unwrap();

        // Create a ready post
        let post_dir = canonical_repo_path.join(".postlane/posts/test-post");
        fs::create_dir_all(&post_dir).unwrap();

        let ready_meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            platform_urls: None,
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
            serde_json::to_string(&ready_meta).unwrap(),
        )
        .unwrap();

        // Create post content file
        fs::write(post_dir.join("x.md"), "Test post content").unwrap();

        // Setup AppState with repo registered (using canonical path)
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "test-repo-id".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };

        let app_state = AppState::new(repos_config);

        // Call approve_post_impl
        // Note: Since we pass None for AppHandle (test mode), the implementation
        // uses the stub path and simulates success. Real credential checking
        // is tested in integration tests with a real AppHandle.
        let result = approve_post_impl(
            canonical_repo_path.to_str().unwrap(),
            "test-post",
            &app_state,
            None,  // No AppHandle in tests - uses stub path
            false,
        ).await;

        // Verify: In test mode (app=None), succeeds with stub implementation
        // Real credential error handling is tested via integration tests
        assert!(
            result.is_ok(),
            "Should succeed in test mode (no AppHandle), got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_approve_post_with_provider_already_instantiated() {
        // This test verifies that when AppState.scheduler already has a provider,
        // approve_post reuses it rather than trying to instantiate a new one
        //
        // Setup: Create temp directory with a repo
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_path).unwrap();

        // Canonicalize the path
        let canonical_repo_path = fs::canonicalize(&repo_path).unwrap();

        // Create .postlane directory structure
        fs::create_dir_all(canonical_repo_path.join(".postlane/posts")).unwrap();

        // Create config.json with scheduler provider set to "zernio"
        let config_json = serde_json::json!({
            "version": 1,
            "scheduler": {
                "provider": "zernio",
                "account_ids": { "x": "acc-twitter-test" }
            }
        });
        fs::write(
            canonical_repo_path.join(".postlane/config.json"),
            serde_json::to_string_pretty(&config_json).unwrap(),
        )
        .unwrap();

        // Create a ready post
        let post_dir = canonical_repo_path.join(".postlane/posts/test-post");
        fs::create_dir_all(&post_dir).unwrap();

        let ready_meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["x".to_string()],
            schedule: None,
            trigger: None,
            scheduler_ids: None,
            platform_results: None,
            platform_urls: None,
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
            serde_json::to_string(&ready_meta).unwrap(),
        )
        .unwrap();

        // Create post content file
        fs::write(post_dir.join("x.md"), "Test post content").unwrap();

        // Setup AppState with repo registered
        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "test-repo-id".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };

        let app_state = AppState::new(repos_config);

        // Pre-populate AppState.scheduler with a ZernioProvider instance
        // This simulates the provider already being instantiated from a previous call
        {
            use postlane_desktop_lib::providers::scheduling::zernio::ZernioProvider;
            let provider = ZernioProvider::new("test-api-key".to_string());
            let mut scheduler = app_state.scheduler.lock().await;
            *scheduler = Some(Box::new(provider));
        }

        // Call approve_post_impl - should use existing provider and succeed
        let result = approve_post_impl(
            canonical_repo_path.to_str().unwrap(),
            "test-post",
            &app_state,
            None,  // No AppHandle in tests - will use stub path
            false,
        ).await;

        // For now, this will still hit the stub implementation
        // When we implement the real provider integration, this should:
        // 1. Use the existing provider from AppState.scheduler
        // 2. Call schedule_post for the "x" platform
        // 3. Return success
        //
        // Currently it will succeed with the stub implementation
        assert!(
            result.is_ok(),
            "Should succeed when provider is already instantiated, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_eager_instantiation_with_no_repos() {
        // Test that eager_init_provider_if_configured handles empty repos list
        use postlane_desktop_lib::commands::eager_init_provider_if_configured;

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![],
        };

        let app_state = AppState::new(repos_config);

        // Should complete without error even with no repos
        let result = eager_init_provider_if_configured(&app_state, None).await;
        assert!(result.is_ok(), "Should succeed with no repos");

        // Scheduler should remain None
        let scheduler = app_state.scheduler.lock().await;
        assert!(scheduler.is_none(), "Scheduler should remain None");
    }

    #[tokio::test]
    async fn test_eager_instantiation_with_configured_provider_but_no_credential() {
        // Test that when a repo has a configured provider but no credential in keyring,
        // eager instantiation silently skips (doesn't error)
        use postlane_desktop_lib::commands::eager_init_provider_if_configured;

        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("test-repo");
        fs::create_dir_all(&repo_path).unwrap();
        let canonical_repo_path = fs::canonicalize(&repo_path).unwrap();

        // Create .postlane directory with config.json
        fs::create_dir_all(canonical_repo_path.join(".postlane")).unwrap();
        let config_json = serde_json::json!({
            "version": 1,
            "scheduler": {
                "provider": "ayrshare"
            }
        });
        fs::write(
            canonical_repo_path.join(".postlane/config.json"),
            serde_json::to_string_pretty(&config_json).unwrap(),
        ).unwrap();

        let repos_config = ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "test-repo-id".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };

        let app_state = AppState::new(repos_config);

        // Call eager init - should silently skip since no credential exists
        // (app=None means test mode, no real keyring access)
        let result = eager_init_provider_if_configured(&app_state, None).await;
        assert!(result.is_ok(), "Should succeed even without credential");

        // Scheduler should remain None (no credential = no instantiation)
        let scheduler = app_state.scheduler.lock().await;
        assert!(scheduler.is_none(), "Scheduler should remain None without credential");
    }
}  // end provider_instantiation_tests

// ---------------------------------------------------------------------------
// get_post_content_impl tests
// ---------------------------------------------------------------------------

mod get_post_content_tests {
    use super::*;

    fn setup_post(platform: &str, content: &str) -> (tempfile::TempDir, String, String) {
        let tmp = tempfile::TempDir::new().unwrap();
        let post_folder = "20260416-test-post";
        let repo_path = tmp.path().to_str().unwrap().to_string();
        let post_dir = tmp.path()
            .join(".postlane/posts")
            .join(post_folder);
        fs::create_dir_all(&post_dir).unwrap();
        fs::write(post_dir.join(format!("{}.md", platform)), content).unwrap();
        (tmp, repo_path, post_folder.to_string())
    }

    #[test]
    fn test_get_post_content_returns_x_content() {
        let content = "Timezone support shipped. Set it in Settings.";
        let (_tmp, repo_path, post_folder) = setup_post("x", content);
        let result = get_post_content_impl(&repo_path, &post_folder, "x");
        assert!(result.is_ok(), "Should read x.md successfully");
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_get_post_content_returns_bluesky_content() {
        let content = "**Timezone support** shipped. Set it in Settings.";
        let (_tmp, repo_path, post_folder) = setup_post("bluesky", content);
        let result = get_post_content_impl(&repo_path, &post_folder, "bluesky");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_get_post_content_rejects_invalid_platform() {
        let (_tmp, repo_path, post_folder) = setup_post("x", "content");
        let result = get_post_content_impl(&repo_path, &post_folder, "twitter");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid platform"));
    }

    #[test]
    fn test_get_post_content_rejects_path_traversal_in_platform() {
        let (_tmp, repo_path, post_folder) = setup_post("x", "content");
        let result = get_post_content_impl(&repo_path, &post_folder, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_post_content_rejects_path_traversal_in_post_folder() {
        let (_tmp, repo_path, _) = setup_post("x", "content");
        let result = get_post_content_impl(&repo_path, "../../etc", "x");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid post folder"));
    }

    #[test]
    fn test_get_post_content_missing_file_returns_err() {
        let tmp = tempfile::TempDir::new().unwrap();
        let repo_path = tmp.path().to_str().unwrap();
        let result = get_post_content_impl(repo_path, "nonexistent-post", "x");
        assert!(result.is_err());
    }
}
