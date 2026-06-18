// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::app_state::AppState;
use postlane_desktop_lib::post_editor::{update_post_content_impl, update_post_image_impl, is_direct_image_url};
use postlane_desktop_lib::storage::{Repo, ReposConfig};
use postlane_desktop_lib::types::PostMeta;
use std::fs;
use tempfile::TempDir;

fn make_state(repos: Vec<Repo>) -> AppState {
    AppState::new_with_path(
        ReposConfig { version: 1, workspaces: vec![], repos },
        std::env::temp_dir().join(format!("pl_test_state_{}.json", std::process::id())),
    )
}

fn make_repo(id: &str, path: &str) -> Repo {
    Repo {
        id: id.to_string(),
        name: id.to_string(),
        path: path.to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn make_post_dir(temp_dir: &TempDir, post_folder: &str) -> std::path::PathBuf {
    let post_path = temp_dir.path().join(".postlane/posts").join(post_folder);
    fs::create_dir_all(&post_path).unwrap();
    fs::write(post_path.join("x.md"), "original x content").unwrap();
    fs::write(post_path.join("bluesky.md"), "original bluesky content").unwrap();
    fs::write(post_path.join("mastodon.md"), "original mastodon content").unwrap();
    temp_dir.path().to_path_buf()
}

#[cfg(test)]
mod update_post_content_tests {
    use super::*;

    #[test]
    fn test_writes_new_content_to_platform_file() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_post_dir(&temp_dir, "post-001");
        let canonical = fs::canonicalize(&repo_path).unwrap();
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        let result = update_post_content_impl(
            canonical.to_str().unwrap(),
            "post-001",
            "x",
            "Updated X post content.",
            &state,
        );

        assert!(result.is_ok());
        let written = fs::read_to_string(
            repo_path.join(".postlane/posts/post-001/x.md"),
        )
        .unwrap();
        assert_eq!(written, "Updated X post content.");
    }

    #[test]
    fn test_all_three_platforms_accepted() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_post_dir(&temp_dir, "post-002");
        let canonical = fs::canonicalize(&repo_path).unwrap();
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        for platform in &["x", "bluesky", "mastodon"] {
            let result = update_post_content_impl(
                canonical.to_str().unwrap(),
                "post-002",
                platform,
                "test content",
                &state,
            );
            assert!(result.is_ok(), "platform '{}' should be accepted", platform);
        }
    }

    #[test]
    fn test_rejects_unknown_platform() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_post_dir(&temp_dir, "post-003");
        // State irrelevant — platform check fires before repo check.
        let state = make_state(vec![]);

        let result = update_post_content_impl(
            repo_path.to_str().unwrap(),
            "post-003",
            "instagram",
            "content",
            &state,
        );

        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_lowercase().contains("invalid platform"),
            "error should mention invalid platform"
        );
    }

    #[test]
    fn test_rejects_post_folder_with_double_dot() {
        let temp_dir = TempDir::new().unwrap();
        // State irrelevant — folder check fires before repo check.
        let state = make_state(vec![]);

        let result = update_post_content_impl(
            temp_dir.path().to_str().unwrap(),
            "../escape-attempt",
            "x",
            "malicious",
            &state,
        );

        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_lowercase().contains("invalid post folder"),
            "error should mention invalid post folder"
        );
    }

    #[test]
    fn test_rejects_post_folder_with_forward_slash() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(vec![]);

        let result = update_post_content_impl(
            temp_dir.path().to_str().unwrap(),
            "nested/folder",
            "x",
            "content",
            &state,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    #[test]
    fn test_no_tmp_file_remains_after_successful_write() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_post_dir(&temp_dir, "post-004");
        let canonical = fs::canonicalize(&repo_path).unwrap();
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        update_post_content_impl(
            canonical.to_str().unwrap(),
            "post-004",
            "x",
            "new content",
            &state,
        )
        .unwrap();

        let tmp = repo_path.join(".postlane/posts/post-004/x.md.tmp");
        assert!(!tmp.exists(), "tmp file must not remain after atomic write");
    }

    #[test]
    fn test_returns_error_when_post_folder_does_not_exist() {
        let temp_dir = TempDir::new().unwrap();
        let canonical = fs::canonicalize(temp_dir.path()).unwrap();
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        let result = update_post_content_impl(
            canonical.to_str().unwrap(),
            "nonexistent-post",
            "x",
            "content",
            &state,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_rejects_unregistered_repo_path() {
        let temp_dir = TempDir::new().unwrap();
        let post_path = temp_dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_path).unwrap();
        let state = make_state(vec![]); // no repos registered

        let result = update_post_content_impl(
            temp_dir.path().to_str().unwrap(),
            "my-post",
            "x",
            "content",
            &state,
        );

        assert!(result.is_err(), "unregistered repo path must be rejected");
    }
}

// ---------------------------------------------------------------------------
// update_post_image tests
// ---------------------------------------------------------------------------

fn make_meta(temp_dir: &TempDir, post_folder: &str, image_url: Option<&str>) -> std::path::PathBuf {
    let post_path = temp_dir.path().join(".postlane/posts").join(post_folder);
    fs::create_dir_all(&post_path).unwrap();
    let meta = PostMeta {
        status: "ready".to_string(),
        platforms: vec!["x".to_string()],
        schedule: None,
        trigger: None,
        scheduler_ids: None,
        platform_results: None,
        platform_urls: None,
        error: None,
        image_url: image_url.map(|s| s.to_string()),
        image_source: None,
        image_attribution: None,
        llm_model: None,
        created_at: Some("2026-04-17T00:00:00Z".to_string()),
        sent_at: None,
        voice_guide_version: None,
        schedule_source: None,
        schedule_timezone: None,
    };
    fs::write(
        post_path.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    temp_dir.path().to_path_buf()
}

#[cfg(test)]
mod update_post_image_tests {
    use super::*;

    #[test]
    fn test_sets_image_url_in_meta_json() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_meta(&temp_dir, "post-img-01", None);

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            repo_path.to_str().unwrap(),
            "post-img-01",
            Some("https://example.com/image.png"),
            &state,
        );

        assert!(result.is_ok());
        let content = fs::read_to_string(
            repo_path.join(".postlane/posts/post-img-01/meta.json"),
        ).unwrap();
        let meta: PostMeta = serde_json::from_str(&content).unwrap();
        assert_eq!(meta.image_url.as_deref(), Some("https://example.com/image.png"));
    }

    #[test]
    fn test_clears_image_url_when_none() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_meta(&temp_dir, "post-img-02", Some("https://old.com/img.png"));

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            repo_path.to_str().unwrap(),
            "post-img-02",
            None,
            &state,
        );

        assert!(result.is_ok());
        let content = fs::read_to_string(
            repo_path.join(".postlane/posts/post-img-02/meta.json"),
        ).unwrap();
        let meta: PostMeta = serde_json::from_str(&content).unwrap();
        assert!(meta.image_url.is_none());
    }

    #[test]
    fn test_rejects_http_url() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_meta(&temp_dir, "post-img-03", None);

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            repo_path.to_str().unwrap(),
            "post-img-03",
            Some("http://example.com/image.png"),
            &state,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("https"));
    }

    #[test]
    fn test_rejects_non_url_string() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_meta(&temp_dir, "post-img-04", None);

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            repo_path.to_str().unwrap(),
            "post-img-04",
            Some("not-a-url"),
            &state,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_rejects_post_folder_with_path_traversal() {
        let temp_dir = TempDir::new().unwrap();

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            temp_dir.path().to_str().unwrap(),
            "../outside",
            Some("https://example.com/image.png"),
            &state,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    #[test]
    fn test_no_tmp_file_remains_after_write() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = make_meta(&temp_dir, "post-img-05", None);

        let state = make_state(vec![]);
        update_post_image_impl(
            repo_path.to_str().unwrap(),
            "post-img-05",
            Some("https://example.com/image.png"),
            &state,
        ).unwrap();

        let tmp = repo_path.join(".postlane/posts/post-img-05/meta.json.tmp");
        assert!(!tmp.exists(), "tmp file must not remain after atomic write");
    }

    #[test]
    fn test_returns_error_when_post_folder_missing() {
        let temp_dir = TempDir::new().unwrap();

        let state = make_state(vec![]);
        let result = update_post_image_impl(
            temp_dir.path().to_str().unwrap(),
            "nonexistent",
            Some("https://example.com/image.png"),
            &state,
        );

        assert!(result.is_err());
    }
}

// ---------------------------------------------------------------------------
// is_direct_image_url tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod is_direct_image_url_tests {
    use super::*;

    #[test]
    fn test_recognises_common_image_extensions() {
        assert!(is_direct_image_url("https://example.com/photo.jpg"));
        assert!(is_direct_image_url("https://example.com/photo.jpeg"));
        assert!(is_direct_image_url("https://example.com/photo.png"));
        assert!(is_direct_image_url("https://example.com/photo.webp"));
        assert!(is_direct_image_url("https://example.com/photo.gif"));
    }

    #[test]
    fn test_recognises_known_image_cdn_hostnames() {
        assert!(is_direct_image_url("https://images.unsplash.com/photo-abc123?w=1200"));
    }

    #[test]
    fn test_rejects_page_urls() {
        assert!(!is_direct_image_url("https://unsplash.com/photos/neon-signage-xv7-GlvBLFw"));
        assert!(!is_direct_image_url("https://example.com/blog/my-post"));
        assert!(!is_direct_image_url("https://example.com"));
    }

    #[test]
    fn test_extension_check_is_case_insensitive() {
        assert!(is_direct_image_url("https://example.com/photo.JPG"));
        assert!(is_direct_image_url("https://example.com/photo.PNG"));
    }
}
