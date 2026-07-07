// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::app_state::AppState;
use postlane_desktop_lib::post_approval::approve_post_impl;
use postlane_desktop_lib::post_dismiss::{delete_post_impl, dismiss_post_impl};
use postlane_desktop_lib::post_export::export_history_csv_impl;
use postlane_desktop_lib::post_queries::{get_drafts_impl, get_post_content_impl};
use postlane_desktop_lib::post_retry::retry_post_impl;
use postlane_desktop_lib::repo_mgmt::{add_repo_impl, check_repo_health_impl, remove_repo_impl, set_repo_active_impl};
use postlane_desktop_lib::storage::{Repo, ReposConfig};
use postlane_desktop_lib::types::PostMeta;
use std::fs;
use std::collections::HashMap;
use tempfile::TempDir;

/// Creates an AppState backed by an isolated temp file so tests never touch ~/.postlane/repos.json.
/// The returned TempDir must be kept in scope for the duration of the test — drop it at the end.
fn make_test_state(repos: ReposConfig) -> (AppState, TempDir) {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let state = AppState::new_with_path(repos, repos_path);
    (state, tmp)
}

/// Writes a post's meta.json under `{repo_path}/.postlane/posts/{folder}` with only the
/// fields tests commonly vary; every other PostMeta field is left at None.
fn write_post_with_status(
    repo_path: &std::path::Path,
    folder: &str,
    status: &str,
    platforms: Vec<String>,
    error: Option<String>,
    created_at: &str,
    sent_at: Option<String>,
) {
    let post_dir = repo_path.join(".postlane/posts").join(folder);
    fs::create_dir_all(&post_dir).unwrap();
    let meta = PostMeta {
        status: status.to_string(),
        platforms,
        schedule: None,
        trigger: None,
        scheduler_ids: None,
        platform_results: None,
        platform_urls: None,
        error,
        image_url: None,
        image_source: None,
        image_attribution: None,
        llm_model: None,
        created_at: Some(created_at.to_string()),
        sent_at,
        voice_guide_version: None,
        schedule_source: None,
        schedule_timezone: None,
    };
    fs::write(post_dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();
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

        write_post_with_status(&repo1_path, "post1", "ready", vec!["x".to_string()], None, "2024-01-01T00:00:00Z", None);
        write_post_with_status(
            &repo1_path, "post2", "failed", vec!["bluesky".to_string()],
            Some("Connection timeout".to_string()), "2024-01-02T00:00:00Z", None,
        );
        // post3 is "sent" and should be excluded from get_drafts_impl's results
        write_post_with_status(
            &repo1_path, "post3", "sent", vec!["x".to_string()], None,
            "2024-01-03T00:00:00Z", Some("2024-01-03T01:00:00Z".to_string()),
        );

        // Setup AppState
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo1_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            voice_guide_version: None,
            schedule_source: None,
            schedule_timezone: None,
        };
        fs::write(
            post_dir.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: false, // Inactive repo
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Try to approve post from unregistered repo
        let result = approve_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            "x",
            &state,
            None,
            false,
        ).await;

        // Assert: Should fail with path not registered error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not registered") || err.contains("403"));
    }

    #[tokio::test]
    async fn test_approve_post_fails_with_invalid_path() {
        // Setup: Create temp directory
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Try to approve post with non-existent post folder
        let result = approve_post_impl(
            repo_path.to_str().unwrap(),
            "nonexistent",
            "x",
            &state,
            None,
            false,
        ).await;

        // Assert: Should fail with validation error
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_approve_post_with_absent_meta_json_uses_default() {
        // PostMeta::load returns Ok(default) when meta.json is absent, so approve succeeds.
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();
        // Canonicalize so validate_repo_path's canonicalize matches the registered path.
        let canonical = fs::canonicalize(&repo_path).unwrap();
        let canonical_str = canonical.to_str().unwrap().to_string();

        let post_folder = canonical.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();
        fs::write(post_folder.join("x.md"), "short content").unwrap();

        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_str.clone(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        let result = approve_post_impl(
            &canonical_str,
            "post1",
            "x",
            &state,
            None,
            false,
        ).await;

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod dismiss_post_tests {
    use super::*;

    #[tokio::test]
    async fn test_dismiss_post_sets_status_to_dismissed() {
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
            voice_guide_version: None,
            schedule_source: None,
            schedule_timezone: None,
        };
        fs::write(
            post_folder.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        // Test: Dismiss the post
        let canonical_repo = fs::canonicalize(&repo_path).unwrap();
        let (state, _tmp_repos) = make_test_state(ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        });
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            false,
        ).await;

        // Assert: Should succeed
        assert!(result.is_ok());

        // Verify: meta.json status is now "dismissed"
        let updated_content = fs::read_to_string(post_folder.join("meta.json")).unwrap();
        let updated_meta: PostMeta = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_meta.status, "dismissed");
    }

    #[tokio::test]
    async fn test_dismiss_post_fails_with_missing_meta_json() {
        // Setup: Create repo with post folder but no meta.json
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

        // Test: Try to dismiss post without meta.json
        let (state, _tmp_repos) = make_test_state(ReposConfig { version: 1, workspaces: vec![], repos: vec![] });
        let result = dismiss_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
            false,
        ).await;

        // Assert: Should fail
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod delete_post_tests {
    use super::*;

    #[test]
    fn test_delete_post_removes_platform_md() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();
        fs::write(post_folder.join("x.md"), "Hello").unwrap();

        let canonical = fs::canonicalize(&repo_path).unwrap();
        let (state, _tmp_repos) = make_test_state(ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        });

        let result = delete_post_impl(repo_path.to_str().unwrap(), "post1", "x", &state);

        assert!(result.is_ok());
        assert!(!post_folder.join("x.md").exists(), "x.md must be deleted");
    }

    #[test]
    fn test_delete_post_rejects_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        fs::create_dir_all(&repo_path).unwrap();

        let (state, _tmp_repos) = make_test_state(ReposConfig { version: 1, workspaces: vec![], repos: vec![] });
        let result = delete_post_impl(repo_path.to_str().unwrap(), "../evil", "x", &state);

        assert!(result.is_err());
    }

    #[test]
    fn test_delete_post_errors_when_md_missing() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo1");
        let post_folder = repo_path.join(".postlane/posts/post1");
        fs::create_dir_all(&post_folder).unwrap();

        let canonical = fs::canonicalize(&repo_path).unwrap();
        let (state, _tmp_repos) = make_test_state(ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        });

        let result = delete_post_impl(repo_path.to_str().unwrap(), "post1", "x", &state);

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod retry_post_tests {
    use super::*;

    #[tokio::test]
    async fn test_retry_post_only_retries_failed_platforms() {
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
            voice_guide_version: None,
            schedule_source: None,
            schedule_timezone: None,
        };
        fs::write(
            post_folder.join("meta.json"),
            serde_json::to_string(&meta).unwrap(),
        )
        .unwrap();

        // Canonicalize the path for repos_config (matches what retry_post_impl does)
        let canonical_repo_path = fs::canonicalize(&repo_path).unwrap();

        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "repo1".to_string(),
                name: "Test Repo".to_string(),
                path: canonical_repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Retry the post
        let result = retry_post_impl(
            repo_path.to_str().unwrap(),
            "post1",
            &state,
        ).await;

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
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![
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
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: repo_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Deactivate repo
        let result = set_repo_active_impl("id1", false, &state);
        assert!(result.is_ok());

        // Verify: Repo is now inactive
        let repos = state.repos.lock().unwrap();
        assert!(!repos.repos[0].active);
    }

    #[test]
    fn test_set_repo_active_activates_repo() {
        // Setup: Create repo that's initially inactive
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: false,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Activate repo
        let result = set_repo_active_impl("id1", true, &state);
        assert!(result.is_ok());

        // Verify: Repo is now active
        let repos = state.repos.lock().unwrap();
        assert!(repos.repos[0].active);
    }

    #[test]
    fn test_set_repo_active_fails_with_invalid_id() {
        // Setup: Create state with one repo
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/tmp/repo1".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: canonical_path.to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Check repo health
        let result = check_repo_health_impl(&state);

        // Assert: Should succeed
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, "id1");
        assert!(statuses[0].reachable);
    }

    #[test]
    fn test_check_repo_health_reports_unreachable_repos() {
        // Setup: Create repo config pointing to non-existent path
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![
                Repo {
                    id: "id1".to_string(),
                    name: "Repo 1".to_string(),
                    path: "/nonexistent/path".to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        // Test: Check repo health
        let result = check_repo_health_impl(&state);

        // Assert: Should succeed but report unreachable
        assert!(result.is_ok());
        let statuses = result.unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].id, "id1");
        assert!(!statuses[0].reachable);
    }
}

#[cfg(test)]
mod export_history_csv_tests {
    use super::*;

    fn write_sent_post_meta(
        posts_dir: &std::path::Path,
        folder: &str,
        platforms: &[&str],
        llm_model: Option<&str>,
        created_at: &str,
        sent_at: &str,
    ) {
        let post_dir = posts_dir.join(folder);
        fs::create_dir_all(&post_dir).unwrap();
        let meta = serde_json::json!({
            "status": "sent",
            "platforms": platforms,
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": llm_model,
            "created_at": created_at,
            "sent_at": sent_at
        });
        fs::write(post_dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    }

    #[test]
    fn test_export_history_csv_generates_valid_csv_headers() {
        // Setup: Empty repos (zero posts case)
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
            version: 1, workspaces: vec![], repos: vec![],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

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
        let posts1_dir = repo1_path.join(".postlane/posts");
        fs::create_dir_all(&posts1_dir).unwrap();
        write_sent_post_meta(&posts1_dir, "post1a", &["x"], None, "2024-01-01T00:00:00Z", "2024-01-01T12:00:00Z");
        write_sent_post_meta(
            &posts1_dir, "post1b", &["x", "linkedin"], Some("claude-3-5-sonnet"),
            "2024-01-02T00:00:00Z", "2024-01-02T12:00:00Z",
        );

        // Repo 2 with 1 sent post
        let repo2_path = temp_dir.path().join("repo2");
        let posts2_dir = repo2_path.join(".postlane/posts");
        fs::create_dir_all(&posts2_dir).unwrap();
        write_sent_post_meta(&posts2_dir, "post2a", &["x"], None, "2024-01-03T00:00:00Z", "2024-01-03T12:00:00Z");

        // Canonicalize paths
        let canonical_repo1 = fs::canonicalize(&repo1_path).unwrap();
        let canonical_repo2 = fs::canonicalize(&repo2_path).unwrap();

        // Setup repos config
        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![
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
        let (state, _tmp_repos) = make_test_state(repos_config);

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

// ---------------------------------------------------------------------------
// Regression guard: mutations must write to state.repos_path, not ~/.postlane
// ---------------------------------------------------------------------------
// These tests guard against the data-corruption bug that has recurred multiple
// times: a mutating command writes test fixture data to ~/.postlane/repos.json,
// wiping the user's real repo registry and making all posts disappear from the
// queue. If any of these tests fail, a command is bypassing state.repos_path.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod test_isolation_guard {
    use super::*;

    fn guard_repo() -> Repo {
        Repo {
            id: "guard-r1".to_string(),
            name: "guard-repo".to_string(),
            path: "/tmp/guard-nonexistent".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    /// set_repo_active_impl must persist the change to state.repos_path (not ~/.postlane).
    #[test]
    fn test_set_repo_active_writes_to_repos_path() {
        let tmp = TempDir::new().unwrap();
        let repos_path = tmp.path().join("repos.json");
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![guard_repo()] },
            repos_path.clone(),
        );
        let result = set_repo_active_impl("guard-r1", false, &state);
        assert!(result.is_ok(), "set_repo_active_impl failed: {:?}", result);
        assert!(repos_path.exists(), "set_repo_active_impl did not write to state.repos_path");
        let written = fs::read_to_string(&repos_path).unwrap();
        assert!(
            written.contains("\"active\":false") || written.contains("\"active\": false"),
            "expected active=false in {repos_path:?}, got: {written}"
        );
    }

    /// remove_repo_impl must persist the removal to state.repos_path (not ~/.postlane).
    #[test]
    fn test_remove_repo_writes_to_repos_path() {
        let tmp = TempDir::new().unwrap();
        let repos_path = tmp.path().join("repos.json");
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![guard_repo()] },
            repos_path.clone(),
        );
        let result = remove_repo_impl("guard-r1", &state);
        assert!(result.is_ok(), "remove_repo_impl failed: {:?}", result);
        assert!(repos_path.exists(), "remove_repo_impl did not write to state.repos_path");
        let written = fs::read_to_string(&repos_path).unwrap();
        assert!(
            !written.contains("guard-r1"),
            "expected guard-r1 to be removed from {repos_path:?}, got: {written}"
        );
    }

    /// set_repo_active_impl and remove_repo_impl must never modify ~/.postlane/repos.json.
    /// If this test fails it means a command has hardcoded the home-dir path.
    #[test]
    fn test_mutations_do_not_touch_home_dir_repos_json() {
        let home_repos = std::env::var("HOME")
            .ok()
            .map(|h| std::path::PathBuf::from(h).join(".postlane/repos.json"));

        let mtime_before = home_repos
            .as_ref()
            .and_then(|p| fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        let (state, _tmp_repos) = make_test_state(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![guard_repo()] }
        );
        let _ = set_repo_active_impl("guard-r1", false, &state);
        let _ = remove_repo_impl("guard-r1", &state);

        let mtime_after = home_repos
            .as_ref()
            .and_then(|p| fs::metadata(p).ok())
            .and_then(|m| m.modified().ok());

        assert_eq!(
            mtime_before, mtime_after,
            "a mutating command modified ~/.postlane/repos.json — \
             use AppState::new_with_path in tests to isolate writes"
        );
    }

    /// Integration path: posts in a registered repo's .postlane/posts/ are returned
    /// by get_drafts_impl. This is the Rust-layer equivalent of "posts appear in queue."
    /// If this test fails, the queue will be empty even though post files exist on disk.
    #[test]
    fn test_get_drafts_loads_posts_from_registered_repos() {
        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("my-repo");
        let post_dir = repo_path.join(".postlane/posts/260527-test-post");
        fs::create_dir_all(&post_dir).unwrap();

        let meta = PostMeta {
            status: "ready".to_string(),
            platforms: vec!["bluesky".to_string()],
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
            created_at: Some("2026-05-27T00:00:00Z".to_string()),
            sent_at: None,
            voice_guide_version: None,
            schedule_source: None,
            schedule_timezone: None,
        };
        fs::write(post_dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();
        fs::write(post_dir.join("bluesky.md"), "Test post content").unwrap();

        let repos_config = ReposConfig {
            version: 1, workspaces: vec![], repos: vec![Repo {
                id: "test-repo-id".to_string(),
                name: "my-repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-05-27T00:00:00Z".to_string(),
            }],
        };
        let (state, _tmp_repos) = make_test_state(repos_config);

        let drafts = get_drafts_impl(&state).expect("get_drafts_impl failed");

        assert_eq!(drafts.len(), 1, "expected 1 draft post, got {}", drafts.len());
        assert_eq!(drafts[0].status, "ready");
        assert_eq!(
            drafts[0].created_at.as_deref(),
            Some("2026-05-27T00:00:00Z"),
            "post created_at did not match — wrong post loaded"
        );
    }
}
