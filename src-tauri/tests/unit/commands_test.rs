// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::app_state::AppState;
use postlane_desktop_lib::commands::{add_repo_impl, approve_post_impl, check_repo_health_impl, dismiss_post_impl, export_history_csv_impl, get_drafts_impl, remove_repo_impl, retry_post_impl, set_repo_active_impl};
use postlane_desktop_lib::storage::{Repo, ReposConfig};
use postlane_desktop_lib::types::PostMeta;
use std::fs;
use std::collections::HashMap;
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

#[cfg(test)]
mod approve_post_tests {
    use super::*;

    #[test]
    fn test_approve_post_fails_with_unregistered_repo() {
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
        );

        // Assert: Should fail with path not registered error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not registered") || err.contains("403"));
    }

    #[test]
    fn test_approve_post_fails_with_invalid_path() {
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
        );

        // Assert: Should fail with validation error
        assert!(result.is_err());
    }

    #[test]
    fn test_approve_post_fails_without_meta_json() {
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
        );

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
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
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
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
        );

        // Assert: Should fail
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
