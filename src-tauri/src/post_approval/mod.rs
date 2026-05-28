// SPDX-License-Identifier: BUSL-1.1

mod pipeline;

use crate::app_state::AppState;
use crate::post_meta::{PostMeta, PostStatus};
use pipeline::{
    acquire_meta_lock, apply_scheduler_result, call_scheduler, is_platform_edited,
    record_scheduler_failure, validate_char_limit, validate_platform, validate_post_folder,
    validate_repo_path, InFlightGuard,
};
use tauri::State;

/// Fires the Unsplash download trigger if applicable, then writes `image_download_triggered_at`.
/// Only fires when `image_download_location` is set, `image_download_triggered_at` is null,
/// and `download_location` starts with `https://api.unsplash.com/`.
/// Skips silently (with a warning log) if the URL fails validation — approval must not be blocked.
fn fire_unsplash_download_trigger(
    meta: &mut PostMeta,
    meta_path: &std::path::Path,
    app: Option<&tauri::AppHandle>,
) {
    let Some(ref location) = meta.image_download_location.clone() else {
        return;
    };
    if meta.image_download_triggered_at.is_some() {
        return;
    }
    // SSRF defence: domain whitelist is the primary guard here.
    // Unlike OG image fetches (which use is_private_url for DNS-resolution-time
    // validation), the download_location is validated against a specific Unsplash
    // hostname prefix. Any private IP literal (127.x, 192.168.x, etc.) cannot pass
    // this check. DNS rebinding against api.unsplash.com is outside the threat model.
    if !location.starts_with("https://api.unsplash.com/") {
        log::warn!(
            "[approve_post] image_download_location rejected (must start with https://api.unsplash.com/): {}",
            location
        );
        return;
    }
    meta.image_download_triggered_at = Some(chrono::Utc::now().to_rfc3339());
    if let Err(e) = meta.save(meta_path) {
        log::error!("[approve_post] failed to write image_download_triggered_at: {}", e);
    }
    if app.is_some() {
        let loc = location.clone();
        tauri::async_runtime::spawn(async move {
            let client = reqwest::Client::builder()
                .user_agent(concat!("Postlane/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default();
            if let Err(e) = client.get(&loc).send().await {
                log::warn!("[approve_post] Unsplash download trigger failed (non-fatal): {}", e);
            }
        });
    }
}

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
    validate_char_limit(platform, &post_path)?;
    // 21.8.8: fire Unsplash download trigger if not yet triggered.
    // Validates download_location starts with https://api.unsplash.com/ before any network call.
    // meta.json is a local writable file — always validate the URL (SSRF prevention).
    fire_unsplash_download_trigger(&mut meta, &meta_path, app);
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
                log::info!("[approve_post] sent '{}' to '{}' via scheduler", post_folder, platform);
                if let Err(e) = meta.save(&meta_path) {
                    log::error!("[approve_post] meta write failed after successful send: {}", e);
                    // Return Err so the frontend surfaces this to the user.
                    // Returning Ok() would leave the queue showing the post as unsent,
                    // causing a duplicate send when the user clicks Approve again.
                    return Err(format!(
                        "Post was sent but the local record could not be saved. \
                         Do not approve again — your post has already been submitted. \
                         Error: {}",
                        e
                    ));
                }
            }
            Err(e) => {
                // No retry: a single attempt failure writes status=Failed and surfaces
                // the error to the user. Retrying automatically risks duplicate sends
                // if the scheduler accepted the request but returned a transient error.
                // The user can re-approve from the queue once the issue is resolved.
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

    #[tokio::test]
    async fn test_approve_post_returns_err_when_meta_write_fails() {
        // Verifies the contract: a disk-write failure after scheduler success must
        // return Err so the frontend can surface it. Returning Ok() would leave the
        // queue showing the post unsent, causing a duplicate send on retry.
        // (The production app=Some path was fixed from Ok to Err; this test covers
        // the test-mode path which uses ? and was already correct.)
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
}
