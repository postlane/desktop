// SPDX-License-Identifier: BUSL-1.1
// Tests for post_approval/mod.rs — extracted to keep the file under 400 lines.

use super::*;
use crate::post_meta::{PostMeta, PostStatus};
use crate::storage::Repo;
use std::path::Path;


    fn make_state(repo_path: &str) -> AppState {
        crate::test_fixtures::make_state(vec![Repo {
            id: "r1".to_string(),
            name: "test".to_string(),
            path: repo_path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }])
    }

    fn write_post(dir: &Path, post_folder: &str) {
        let post_path = dir.join(".postlane/posts").join(post_folder);
        std::fs::create_dir_all(&post_path).expect("create post dir");
        std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
        std::fs::write(post_path.join("x.md"), "test content").expect("write x.md");
    }

    fn write_post_with_content(dir: &Path, post_folder: &str, platform: &str, content: &str) {
        let post_path = dir.join(".postlane/posts").join(post_folder);
        std::fs::create_dir_all(&post_path).expect("create post dir");
        std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
        std::fs::write(post_path.join(format!("{}.md", platform)), content).expect("write platform file");
    }

    // --- §validate_char_limit (integration) ---

    #[tokio::test]
    async fn test_approve_post_rejects_over_limit_bluesky_post() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post_with_content(&canonical, "post-over-limit", "bluesky", &"a".repeat(301));
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-over-limit", "bluesky", &state, None, false).await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("301"), "error must mention actual count: {}", msg);
        assert!(msg.contains("300"), "error must mention the limit: {}", msg);
    }

    #[tokio::test]
    async fn test_approve_post_accepts_post_at_exact_limit() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post_with_content(&canonical, "post-at-limit", "bluesky", &"a".repeat(300));
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-at-limit", "bluesky", &state, None, false).await;
        assert!(result.is_ok(), "post at exact limit must be accepted: {:?}", result);
    }

    // --- §validate_platform ---

    #[tokio::test]
    async fn test_approve_post_rejects_unknown_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-a", "unknown", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown platform"));
    }

    #[tokio::test]
    async fn test_approve_post_rejects_empty_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-a", "", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown platform"));
    }

    // --- §validate_post_folder ---

    #[tokio::test]
    async fn test_approve_post_rejects_path_traversal() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "../etc", "x", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    #[tokio::test]
    async fn test_approve_post_rejects_multi_segment_post_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "a/b", "x", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    // --- §validate_repo_path ---

    #[tokio::test]
    async fn test_approve_post_rejects_repo_path_not_in_repos() {
        let state = make_state("/nonexistent/path/that/is/not/registered");
        let result = approve_post_impl("/tmp", "post-a", "x", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }

    // --- §idempotency ---

    #[tokio::test]
    async fn test_approve_post_is_idempotent_when_already_sent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-idem");
        // Pre-populate sent_platforms so post appears already sent
        let meta_path = PostMeta::path_for(&canonical, "post-idem");
        let mut meta = PostMeta::default();
        meta.sent_platforms.insert("x".to_string(), "2026-05-01T00:00:00Z".to_string());
        meta.save(&meta_path).expect("save pre-sent meta");
        let state = make_state(&canonical_str);
        // Second call must return Ok without error
        let result = approve_post_impl(&canonical_str, "post-idem", "x", &state, None, false).await;
        assert!(result.is_ok(), "idempotent call must succeed: {:?}", result);
        // sent_platforms must still have exactly one entry
        let loaded = PostMeta::load(&meta_path).unwrap();
        assert_eq!(loaded.sent_platforms.len(), 1);
    }

    // --- §concurrent_calls ---

    #[tokio::test]
    async fn test_approve_post_concurrent_calls_send_only_once() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-concurrent");
        let state = make_state(&canonical_str);
        // Two sequential calls simulate concurrent access after the DashMap lock serializes.
        let r1 = approve_post_impl(&canonical_str, "post-concurrent", "x", &state, None, false).await;
        let r2 = approve_post_impl(&canonical_str, "post-concurrent", "x", &state, None, false).await;
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        let meta_path = PostMeta::path_for(&canonical, "post-concurrent");
        let meta = PostMeta::load(&meta_path).unwrap();
        assert_eq!(meta.sent_platforms.len(), 1, "exactly one sent_at entry");
    }

    #[tokio::test]
    async fn test_approve_post_and_save_post_draft_do_not_race() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-race");
        let meta_path = PostMeta::path_for(&canonical, "post-race");
        // Simulate save_post_draft acquiring the lock and writing edited_platforms
        {
            let lock = acquire_meta_lock(&canonical_str, "post-race");
            let _guard = lock.lock().await;
            let mut meta = PostMeta::load(&meta_path).unwrap();
            meta.edited_platforms = Some(vec!["x".to_string()]);
            meta.edited_at = Some("2026-05-01T00:00:00Z".to_string());
            meta.save(&meta_path).unwrap();
        }
        // approve_post must acquire the lock and write sent_platforms without
        // overwriting edited_platforms (PostMeta::load reads the full current state)
        let state = make_state(&canonical_str);
        approve_post_impl(&canonical_str, "post-race", "x", &state, None, false)
            .await
            .expect("approve must succeed");
        let final_meta = PostMeta::load(&meta_path).unwrap();
        assert!(final_meta.sent_platforms.contains_key("x"), "sent_platforms must be set");
        assert_eq!(
            final_meta.edited_platforms,
            Some(vec!["x".to_string()]),
            "edited_platforms must be preserved"
        );
    }

    // --- §scheduler_result (integration) ---

    #[tokio::test]
    async fn test_approve_post_writes_sent_status_to_meta() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "my-post");
        let state = make_state(&canonical_str);
        approve_post_impl(&canonical_str, "my-post", "x", &state, None, false)
            .await
            .expect("should succeed");
        let meta_path = PostMeta::path_for(&canonical, "my-post");
        let meta = PostMeta::load(&meta_path).expect("load");
        assert_eq!(
            meta.status,
            Some(PostStatus::Sent),
            "approve_post must write status=sent so engagement_sync can pick up the post"
        );
    }

    // --- §failed_status (integration) ---

    #[tokio::test]
    async fn test_approve_post_failed_status_does_not_block_retry() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-retry");
        // Pre-write meta with status=Failed (simulates a prior failed attempt)
        let meta_path = PostMeta::path_for(&canonical, "post-retry");
        let mut meta = PostMeta::default();
        meta.status = Some(PostStatus::Failed);
        meta.error = Some("prior failure".to_string());
        meta.save(&meta_path).expect("save failed meta");
        // Retry must proceed (idempotency is on sent_platforms, not status)
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-retry", "x", &state, None, false).await;
        assert!(result.is_ok(), "retry after failure must succeed: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).unwrap();
        assert!(final_meta.sent_platforms.contains_key("x"));
    }

    // --- §image download trigger (21.8.8) ---

    #[tokio::test]
    async fn test_approve_post_writes_download_triggered_at_when_location_set() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-dl");
        let meta_path = PostMeta::path_for(&canonical, "post-dl");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://api.unsplash.com/photos/abc/download".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-dl", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must succeed: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load after approval");
        assert!(
            final_meta.image_download_triggered_at.is_some(),
            "image_download_triggered_at must be written after approval"
        );
    }

    #[tokio::test]
    async fn test_approve_post_skips_download_when_triggered_at_already_set() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-dl-skip");
        let meta_path = PostMeta::path_for(&canonical, "post-dl-skip");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://api.unsplash.com/photos/abc/download".to_string());
        meta.image_download_triggered_at = Some("2026-05-01T09:00:00Z".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-dl-skip", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must succeed: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load after approval");
        assert_eq!(
            final_meta.image_download_triggered_at.as_deref(),
            Some("2026-05-01T09:00:00Z"),
            "original triggered_at must be unchanged (21.8.25)"
        );
    }

    #[tokio::test]
    async fn test_approve_post_legacy_image_url_no_download_trigger() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-legacy");
        let meta_path = PostMeta::path_for(&canonical, "post-legacy");
        std::fs::write(
            &meta_path,
            r#"{"image_url":"https://images.unsplash.com/photo-old"}"#,
        )
        .expect("write legacy meta");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-legacy", "x", &state, None, false).await;
        assert!(result.is_ok(), "legacy post must approve without error: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load after approval");
        assert!(
            final_meta.image_download_triggered_at.is_none(),
            "no download trigger for legacy post without image_download_location"
        );
    }

    // --- §download_location SSRF validation (21.8.22) ---

    #[tokio::test]
    async fn test_approve_post_download_location_private_ip_does_not_trigger() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-ssrf-ip");
        let meta_path = PostMeta::path_for(&canonical, "post-ssrf-ip");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://192.168.1.1/download".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-ssrf-ip", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must not be blocked: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load");
        assert!(final_meta.image_download_triggered_at.is_none(), "private IP must be rejected");
    }

    #[tokio::test]
    async fn test_approve_post_download_location_localhost_does_not_trigger() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-ssrf-localhost");
        let meta_path = PostMeta::path_for(&canonical, "post-ssrf-localhost");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://localhost/download".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-ssrf-localhost", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must not be blocked: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load");
        assert!(final_meta.image_download_triggered_at.is_none(), "localhost must be rejected");
    }

    #[tokio::test]
    async fn test_approve_post_download_location_loopback_ip_does_not_trigger() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-ssrf-loopback");
        let meta_path = PostMeta::path_for(&canonical, "post-ssrf-loopback");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://127.0.0.1/download".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-ssrf-loopback", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must not be blocked: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load");
        assert!(final_meta.image_download_triggered_at.is_none(), "loopback IP must be rejected");
    }

    #[tokio::test]
    async fn test_approve_post_invalid_download_location_does_not_block_approval() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-bad-dl");
        let meta_path = PostMeta::path_for(&canonical, "post-bad-dl");
        let mut meta = PostMeta::load(&meta_path).expect("load");
        meta.image_download_location = Some("https://evil.example.com/download".to_string());
        meta.save(&meta_path).expect("save");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-bad-dl", "x", &state, None, false).await;
        assert!(result.is_ok(), "approval must still succeed with invalid download_location: {:?}", result);
        let final_meta = PostMeta::load(&meta_path).expect("load after approval");
        assert!(
            final_meta.image_download_triggered_at.is_none(),
            "triggered_at must not be written when location failed validation"
        );
    }

    // --- §meta_write_failure ---
    //
    // ARCHITECTURE NOTE — app=Some path is NOT covered by unit tests.
    //
    // The production path (app=Some, line ~92–113 in approve_post_impl) calls
    // call_scheduler() then writes meta.json.  If the meta write fails it now
    // returns Err (fixed: was Ok, which caused duplicate sends on retry).
    //
    // WHY THIS PATH CANNOT BE UNIT-TESTED:
    //   `call_scheduler` requires a real `tauri::AppHandle` to resolve the
    //   managed scheduler state.  AppHandle cannot be constructed outside of a
    //   Tauri runtime, so there is no safe way to inject it in a `#[tokio::test]`.
    //   Attempting to construct one via internal Tauri APIs would couple the tests
    //   to private Tauri internals and break on any Tauri upgrade.
    //
    // REGRESSION RISK:
    //   If the `return Err(...)` on line ~107 is reverted to `log::error!` + `Ok(())`
    //   (the original bug), the frontend will silently succeed and the post will remain
    //   in the queue.  The user can then click Approve again and send a duplicate post.
    //   This is a data-integrity bug that can cause real-world duplicate publishing.
    //
    // HOW TO TEST IT:
    //   Use WebDriver / Tauri integration tests (`tauri::test`) to boot a real app
    //   instance, write a post, make the posts directory read-only, call approve_post,
    //   and assert the Tauri command returns an Err variant (surfaced as a rejected
    //   IPC promise on the JS side).  See: https://docs.rs/tauri/latest/tauri/test/
    //
    // The test below covers only the app=None (test-mode) path, which exercises the
    // same Err contract via the `?` operator on meta.save().

    #[tokio::test]
    async fn test_approve_post_returns_err_when_meta_write_fails() {
        // Verifies the contract: a disk-write failure after scheduler success must
        // return Err so the frontend can surface it. Returning Ok() would leave the
        // queue showing the post unsent, causing a duplicate send on retry.
        // This test covers the app=None (test-mode) path only; see the architecture
        // note above explaining why the app=Some production path cannot be unit-tested.
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-ro-fail");

        let post_path = canonical.join(".postlane/posts/post-ro-fail");
        let ro = std::fs::Permissions::from_mode(0o444);
        std::fs::set_permissions(&post_path, ro).expect("set read-only");

        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-ro-fail", "x", &state, None, false).await;

        let rw = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&post_path, rw);

        assert!(result.is_err(), "meta write failure must return Err, not Ok — returning Ok causes duplicate sends");
    }

    // --- §telemetry ---

    #[tokio::test]
    async fn test_approve_records_telemetry_when_consent_given() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-tel-a");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-a", "x", &state, None, true).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 1);
    }

    #[tokio::test]
    async fn test_approve_no_telemetry_when_consent_not_given() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-tel-b");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-b", "x", &state, None, false).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 0);
    }

    // ── 22.2.7 workspace path resolution ─────────────────────────────────────

    fn make_workspace_state(workspace_path: &str, child_path: &str, posts_dir: &str) -> AppState {
        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

        let ws_path = std::path::Path::new(workspace_path);
        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "r1".to_string(),
                name: "frontend".to_string(),
                path: child_path.to_string(),
                posts_dir: posts_dir.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&ws_path.join("repos.json"), &ws_repos).expect("write ws repos");

        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: "ws-1".to_string(),
                name: "myorg".to_string(),
                workspace_path: workspace_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!(
            "repos_approve_ws_{}.json", std::process::id()
        ));
        AppState::new_with_path(config, repos_path)
    }

    fn write_workspace_post(workspace_path: &Path, posts_dir: &str, post_folder: &str) {
        let post_path = workspace_path.join("posts").join(posts_dir).join(post_folder);
        std::fs::create_dir_all(&post_path).expect("create workspace post dir");
        std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
        std::fs::write(post_path.join("x.md"), "workspace post content").expect("write x.md");
    }

    /// 22.2.12 — approve_post_impl resolves post path correctly from workspace layout.
    #[tokio::test]
    async fn test_approve_post_resolves_workspace_layout_path() {
        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("frontend");
        std::fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

        let state = make_workspace_state(
            canonical_ws.to_str().unwrap(),
            canonical_child.to_str().unwrap(),
            "frontend",
        );
        write_workspace_post(&canonical_ws, "frontend", "my-post");

        let result = approve_post_impl(
            canonical_child.to_str().unwrap(),
            "my-post",
            "x",
            &state,
            None,
            false,
        ).await;
        assert!(result.is_ok(), "workspace approve must succeed: {:?}", result);
    }

    /// 22.2.13 — approve_post_impl resolves post path correctly from legacy per-repo layout.
    #[tokio::test]
    async fn test_approve_post_resolves_legacy_layout_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "legacy-post");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "legacy-post", "x", &state, None, false).await;
        assert!(result.is_ok(), "legacy approve must succeed: {:?}", result);
    }

    /// 22.2.18 — sent.jsonl history entry written after successful workspace approval.
    #[tokio::test]
    async fn test_approve_post_writes_sent_jsonl_for_workspace() {
        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("frontend");
        std::fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

        let state = make_workspace_state(
            canonical_ws.to_str().unwrap(),
            canonical_child.to_str().unwrap(),
            "frontend",
        );
        write_workspace_post(&canonical_ws, "frontend", "history-post");

        let result = approve_post_impl(
            canonical_child.to_str().unwrap(),
            "history-post",
            "x",
            &state,
            None,
            false,
        ).await;
        assert!(result.is_ok(), "workspace approve must succeed: {:?}", result);

        let jsonl_path = canonical_ws.join("history").join("frontend").join("sent.jsonl");
        assert!(jsonl_path.exists(), "sent.jsonl must be written after workspace approval");
        let content = std::fs::read_to_string(&jsonl_path).expect("read sent.jsonl");
        let entry: serde_json::Value = serde_json::from_str(content.trim()).expect("valid JSON");
        assert_eq!(entry["platform"].as_str(), Some("x"));
        assert_eq!(entry["post_folder"].as_str(), Some("history-post"));
    }

    /// History entry scheduler_id must come from meta.scheduler_ids, not meta.sent_platforms.
    /// meta.sent_platforms stores the sent_at timestamp — writing that as scheduler_id is wrong.
    #[tokio::test]
    async fn test_approve_post_workspace_history_uses_scheduler_id_not_timestamp() {
        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("frontend");
        std::fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

        let state = make_workspace_state(
            canonical_ws.to_str().unwrap(),
            canonical_child.to_str().unwrap(),
            "frontend",
        );

        // Write post with a pre-existing scheduler_id (simulates re-approve after prior schedule call)
        let post_dir = canonical_ws.join("posts").join("frontend").join("sched-id-post");
        std::fs::create_dir_all(&post_dir).expect("create post dir");
        std::fs::write(
            post_dir.join("meta.json"),
            r#"{"scheduler_ids":{"x":"real-uuid-abc123"},"platforms":["x"]}"#,
        ).expect("write meta");
        std::fs::write(post_dir.join("x.md"), "content").expect("write x.md");

        let result = approve_post_impl(
            canonical_child.to_str().unwrap(),
            "sched-id-post",
            "x",
            &state,
            None,
            false,
        ).await;
        assert!(result.is_ok(), "approve must succeed: {:?}", result);

        let jsonl_path = canonical_ws.join("history").join("frontend").join("sent.jsonl");
        let content = std::fs::read_to_string(&jsonl_path).expect("read sent.jsonl");
        let entry: serde_json::Value = serde_json::from_str(content.trim()).expect("valid JSON");
        assert_eq!(
            entry["scheduler_id"].as_str(),
            Some("real-uuid-abc123"),
            "history scheduler_id must be the real UUID from meta.scheduler_ids, not a timestamp: got '{}'",
            entry["scheduler_id"]
        );
    }

    /// 22.10.5 — after workspace approval, meta.json is written to the workspace posts path,
    /// not to the child repo's own .postlane/posts/ directory.
    #[tokio::test]
    async fn test_approve_post_writes_meta_json_to_workspace_path_not_child_repo() {
        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("my-repo");
        std::fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

        let state = make_workspace_state(
            canonical_ws.to_str().unwrap(),
            canonical_child.to_str().unwrap(),
            "my-repo",
        );
        write_workspace_post(&canonical_ws, "my-repo", "test-post");

        let result = approve_post_impl(
            canonical_child.to_str().unwrap(),
            "test-post",
            "x",
            &state,
            None,
            false,
        ).await;
        assert!(result.is_ok(), "workspace approve must succeed: {:?}", result);

        // meta.json must be updated at the workspace posts path
        let ws_meta = canonical_ws.join("posts").join("my-repo").join("test-post").join("meta.json");
        assert!(ws_meta.exists(), "meta.json must exist at workspace posts path: {}", ws_meta.display());
        let meta: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&ws_meta).expect("read meta.json")
        ).expect("parse meta.json");
        assert!(
            meta.get("sent_platforms").is_some(),
            "meta.json at workspace path must have sent_platforms written by approve pipeline",
        );

        // meta.json must NOT be written to the child repo's own .postlane/posts/ directory
        let legacy_meta = canonical_child.join(".postlane").join("posts").join("test-post").join("meta.json");
        assert!(
            !legacy_meta.exists(),
            "meta.json must NOT be written to child repo legacy path: {}",
            legacy_meta.display(),
        );
    }

    // --- §license_check (FIN-C1) ---

    /// approve_post must return Err when the license has been marked expired by the
    /// revalidation loop. A cancelled subscriber must not be able to approve posts
    /// after the 24-hour loop fires.
    #[tokio::test]
    async fn test_approve_post_blocked_when_license_expired() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-expired-license");
        let state = make_state(&canonical_str);
        state.license_expired.store(true, std::sync::atomic::Ordering::Relaxed);

        let result = approve_post_impl(&canonical_str, "post-expired-license", "x", &state, None, false).await;

        assert!(result.is_err(), "approval must be blocked when license is expired");
        let msg = result.unwrap_err();
        assert!(
            msg.to_lowercase().contains("license"),
            "error must mention license, got: {}",
            msg
        );
    }

    /// approve_post must proceed normally when the license has not been marked expired.
    /// This is the default state — all existing tests implicitly rely on this.
    #[tokio::test]
    async fn test_approve_post_proceeds_when_license_not_expired() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-valid-license");
        let state = make_state(&canonical_str);
        // license_expired defaults to false — no action needed

        let result = approve_post_impl(&canonical_str, "post-valid-license", "x", &state, None, false).await;

        assert!(result.is_ok(), "approval must proceed when license is not expired: {:?}", result);
    }

    // --- §post_folder_missing ---

    /// When the repo is registered but the post folder does not exist on disk, approve_post
    /// must return Err. PostMeta::load returns default for a missing file, so we reach
    /// the `if !post_path.exists()` guard (line 82 of approve_post_impl).
    #[tokio::test]
    async fn test_approve_post_returns_err_when_post_folder_does_not_exist() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        // Repo is registered but no post folder is created
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "no-such-post", "x", &state, None, false).await;
        assert!(result.is_err(), "must fail when post folder does not exist");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("does not exist"),
            "error must describe the missing folder, got: {}", msg
        );
    }

    #[test]
    fn test_cancel_post_impl_returns_not_implemented_error() {
        let result = cancel_post_impl();
        assert!(result.is_err(), "cancel must return Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not yet available"),
            "error message must be user-facing (contain 'not yet available'), got: {}",
            msg
        );
    }

    #[test]
    fn test_cancel_post_impl_error_is_user_facing() {
        let result = cancel_post_impl();
        assert!(result.is_err());
        let msg = result.unwrap_err();
        // Must NOT contain internal milestone/roadmap language
        assert!(!msg.contains("Milestone"), "error message must not leak internal milestone: {}", msg);
        assert!(!msg.contains("M4"), "error message must not leak internal milestone: {}", msg);
        assert!(!msg.contains("deferred"), "error message must not leak internal milestone: {}", msg);
        // Must be user-facing — contain actionable guidance
        assert!(msg.contains("not yet available") || msg.contains("delete"),
                "error message must be user-facing, got: {}", msg);
    }
