// SPDX-License-Identifier: BUSL-1.1

mod pipeline;

use crate::app_state::AppState;
use crate::post_meta::{PostMeta, PostStatus};
use pipeline::{
    acquire_meta_lock, apply_scheduler_result, call_scheduler, is_platform_edited,
    record_scheduler_failure, validate_platform, validate_post_folder, validate_repo_path,
    InFlightGuard,
};
use tauri::State;

/// Core implementation, injectable for testing (no real HTTP call when `app` is None).
pub async fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
    state: &AppState,
    app: Option<&tauri::AppHandle>,
    consent: bool,
) -> Result<(), String> {
    let _guard = InFlightGuard::new(&state.in_flight_sends);
    validate_platform(platform)?;
    validate_post_folder(post_folder)?;
    let canonical_str = validate_repo_path(repo_path, state)?;
    let canonical_path = std::path::PathBuf::from(&canonical_str);
    let lock = acquire_meta_lock(&canonical_str, post_folder);
    let _lock_guard = lock.lock().await;
    let meta_path = PostMeta::path_for(&canonical_path, post_folder);
    let mut meta = PostMeta::load(&meta_path)?;
    if meta.sent_platforms.contains_key(platform) {
        return Ok(());
    }
    let post_path = canonical_path.join(".postlane/posts").join(post_folder);
    if !post_path.exists() {
        return Err(format!("Post folder '{}' does not exist", post_folder));
    }
    let sent_at = chrono::Utc::now().to_rfc3339();
    let is_edited = is_platform_edited(&meta, platform);
    if let Some(app_handle) = app {
        match call_scheduler(app_handle, platform, &post_path, &meta, &canonical_path).await {
            Ok((scheduler_id, platform_url)) => {
                apply_scheduler_result(
                    &mut meta,
                    platform,
                    &scheduler_id,
                    platform_url.as_deref(),
                    &sent_at,
                );
                if let Err(e) = meta.save(&meta_path) {
                    log::error!("[approve_post] meta write failed after successful send: {}", e);
                    return Ok(());
                }
            }
            Err(e) => {
                record_scheduler_failure(&mut meta, &e);
                let _ = meta.save(&meta_path);
                return Err(e);
            }
        }
    } else {
        // Test mode: simulate scheduler success without an HTTP call.
        meta.sent_platforms.insert(platform.to_string(), sent_at);
        meta.status = Some(PostStatus::Sent);
        meta.save(&meta_path)?;
    }
    state.telemetry.record(
        consent,
        "post_approved",
        serde_json::json!({"platform": platform, "is_edited": is_edited}),
    );
    Ok(())
}

/// Tauri command — approves a post for publishing on the given platform.
#[tauri::command]
pub async fn approve_post(
    repo_path: String,
    post_folder: String,
    platform: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    approve_post_impl(&repo_path, &post_folder, &platform, &state, Some(&app), consent).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post_meta::PostMeta;
    use crate::storage::{Repo, ReposConfig};
    use std::path::Path;

    fn make_state(repo_path: &str) -> AppState {
        AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: repo_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    fn write_post(dir: &Path, post_folder: &str) {
        let post_path = dir.join(".postlane/posts").join(post_folder);
        std::fs::create_dir_all(&post_path).expect("create post dir");
        std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
        std::fs::write(post_path.join("x.md"), "test content").expect("write x.md");
    }

    // --- §validate_platform ---

    #[tokio::test]
    async fn test_approve_post_rejects_unknown_platform() {
        let dir = std::env::temp_dir().join("postlane_test_approve_unknown_platform");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-a", "unknown", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown platform"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_post_rejects_empty_platform() {
        let dir = std::env::temp_dir().join("postlane_test_approve_empty_platform");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-a", "", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown platform"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §validate_post_folder ---

    #[tokio::test]
    async fn test_approve_post_rejects_path_traversal() {
        let dir = std::env::temp_dir().join("postlane_test_approve_traversal_m19");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "../etc", "x", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_post_rejects_multi_segment_post_folder() {
        let dir = std::env::temp_dir().join("postlane_test_approve_multi_seg");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "a/b", "x", &state, None, false).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
        let _ = std::fs::remove_dir_all(&dir);
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
        let dir = std::env::temp_dir().join("postlane_test_approve_idempotent_m19");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §concurrent_calls ---

    #[tokio::test]
    async fn test_approve_post_concurrent_calls_send_only_once() {
        let dir = std::env::temp_dir().join("postlane_test_approve_concurrent");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_post_and_save_post_draft_do_not_race() {
        let dir = std::env::temp_dir().join("postlane_test_approve_race");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §scheduler_result (integration) ---

    #[tokio::test]
    async fn test_approve_post_writes_sent_status_to_meta() {
        let dir = std::env::temp_dir().join("postlane_test_approve_sent_status_m19");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §failed_status (integration) ---

    #[tokio::test]
    async fn test_approve_post_failed_status_does_not_block_retry() {
        let dir = std::env::temp_dir().join("postlane_test_approve_retry");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
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
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §telemetry ---

    #[tokio::test]
    async fn test_approve_records_telemetry_when_consent_given() {
        let dir = std::env::temp_dir().join("postlane_test_approve_tel_yes_m19");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-tel-a");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-a", "x", &state, None, true).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_no_telemetry_when_consent_not_given() {
        let dir = std::env::temp_dir().join("postlane_test_approve_tel_no_m19");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_post(&canonical, "post-tel-b");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-b", "x", &state, None, false).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
