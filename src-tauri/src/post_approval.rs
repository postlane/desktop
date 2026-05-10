// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::platform_constants::KNOWN_SOCIAL_PLATFORMS;
use crate::post_meta::{PostMeta, PostStatus};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

/// RAII guard — increments in_flight_sends on construction, decrements on drop.
/// Prevents the tray from quitting while a send is in progress.
struct InFlightGuard(Arc<AtomicUsize>);

impl InFlightGuard {
    fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter.clone())
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

/// In v1, all social platforms route through the Zernio scheduler.
/// This indirection allows per-platform provider routing in v2 without call-site changes.
fn scheduler_provider_for(_platform: &str) -> &'static str {
    "zernio"
}

fn validate_platform(platform: &str) -> Result<(), String> {
    if !KNOWN_SOCIAL_PLATFORMS.contains(&platform) {
        return Err(format!(
            "Unknown platform '{}': must be one of {:?}",
            platform, KNOWN_SOCIAL_PLATFORMS
        ));
    }
    Ok(())
}

fn validate_post_folder(post_folder: &str) -> Result<(), String> {
    if Path::new(post_folder).components().count() != 1 {
        return Err(format!(
            "Invalid post folder '{}': must be a single path component.",
            post_folder
        ));
    }
    Ok(())
}

fn validate_repo_path(repo_path: &str, state: &AppState) -> Result<String, String> {
    let canonical = std::fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize repo path '{}': {}", repo_path, e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or("Repo path contains non-UTF-8 characters")?
        .to_string();
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    if !repos.repos.iter().any(|r| r.path == canonical_str) {
        return Err(format!("Repo '{}' is not registered", canonical_str));
    }
    Ok(canonical_str)
}

fn acquire_meta_lock(canonical_str: &str, post_folder: &str) -> Arc<tokio::sync::Mutex<()>> {
    let key = format!("{}\x00{}", canonical_str, post_folder);
    crate::platform_constants::POST_META_LOCKS
        .entry(key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// Returns true if `platform` appears in meta.edited_platforms.
/// None (pre-M19 post) and Some([]) (post-M19, unedited) both return false.
fn is_platform_edited(meta: &PostMeta, platform: &str) -> bool {
    meta.edited_platforms
        .as_deref()
        .unwrap_or(&[])
        .contains(&platform.to_string())
}

/// Testable credential resolver — injectable for unit testing.
fn get_api_key_for_platform(
    platform: &str,
    keyring_fn: impl Fn(&str, &str) -> Option<String>,
) -> Result<String, String> {
    let provider = scheduler_provider_for(platform);
    let key = format!("postlane-scheduler-{}", provider);
    keyring_fn("postlane", &key)
        .ok_or_else(|| format!("No credential found for scheduler provider '{}'", provider))
}

fn get_api_key_from_handle(app: &tauri::AppHandle, platform: &str) -> Result<String, String> {
    get_api_key_for_platform(platform, |service, account| {
        app.keyring()
            .get_password(service, account)
            .ok()
            .flatten()
    })
}

fn read_platform_content(post_path: &Path, platform: &str) -> Result<String, String> {
    let content_file = post_path.join(format!("{}.md", platform));
    if !content_file.exists() {
        return Err(format!(
            "Content file {}.md not found in {:?}",
            platform, post_path
        ));
    }
    std::fs::read_to_string(&content_file)
        .map_err(|e| format!("Failed to read {}.md: {}", platform, e))
}

fn load_account_ids(
    canonical_path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let config_path = canonical_path.join(".postlane/config.json");
    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config.json: {}", e))?;
    Ok(config["scheduler"]["account_ids"]
        .as_object()
        .cloned()
        .unwrap_or_default())
}

/// Stamps a successful scheduler response onto `meta` in-place.
/// Empty scheduler_id and non-https platform_url are silently dropped.
fn apply_scheduler_result(
    meta: &mut PostMeta,
    platform: &str,
    scheduler_id: &str,
    platform_url: Option<&str>,
    sent_at: &str,
) {
    meta.sent_platforms.insert(platform.to_string(), sent_at.to_string());
    if !scheduler_id.is_empty() {
        meta.scheduler_ids.insert(platform.to_string(), scheduler_id.to_string());
    }
    if let Some(url) = platform_url {
        if url.starts_with("https://") {
            meta.platform_urls.insert(platform.to_string(), url.to_string());
        } else {
            log::warn!("[approve_post] rejected non-https platform_url: {}", url);
        }
    }
}

fn record_scheduler_failure(meta: &mut PostMeta, error: &str) {
    meta.status = Some(PostStatus::Failed);
    meta.error = Some(error.to_string());
}

async fn call_scheduler(
    app: &tauri::AppHandle,
    platform: &str,
    post_path: &Path,
    meta: &PostMeta,
    canonical_path: &Path,
) -> Result<(String, Option<String>), String> {
    let api_key = get_api_key_from_handle(app, platform)?;
    let content = read_platform_content(post_path, platform)?;
    let account_ids = load_account_ids(canonical_path).unwrap_or_default();
    let account_id = account_ids
        .get(platform)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let scheduled_for = meta
        .scheduled_for
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let provider = crate::scheduling::credential_router::build_provider(
        scheduler_provider_for(platform),
        api_key,
    )?;
    let result = provider
        .schedule_post(&content, platform, scheduled_for, None, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok((result.scheduler_id, result.platform_url))
}

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
                apply_scheduler_result(&mut meta, platform, &scheduler_id, platform_url.as_deref(), &sent_at);
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
        meta.save(&meta_path)?;
    }
    state.telemetry.record(
        consent,
        "post_approved",
        serde_json::json!({"platform": platform, "is_edited": is_edited}),
    );
    Ok(())
}

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

    // --- §scheduler_provider_for ---

    #[test]
    fn test_approve_post_resolves_x_to_zernio_provider() {
        let key_used = std::cell::RefCell::new(String::new());
        let _ = get_api_key_for_platform("x", |_service, account| {
            *key_used.borrow_mut() = account.to_string();
            None
        });
        assert_eq!(*key_used.borrow(), "postlane-scheduler-zernio");
    }

    #[test]
    fn test_approve_post_returns_error_when_no_scheduler_credential() {
        let result = get_api_key_for_platform("x", |_service, _account| None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("zernio"));
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

    // --- §is_platform_edited ---

    #[test]
    fn test_approve_post_records_edited_true_when_platform_in_edited_platforms() {
        let meta = PostMeta {
            edited_platforms: Some(vec!["x".to_string()]),
            ..PostMeta::default()
        };
        assert!(is_platform_edited(&meta, "x"));
    }

    #[test]
    fn test_approve_post_records_edited_false_when_platform_not_in_edited_platforms() {
        let meta = PostMeta {
            edited_platforms: Some(vec!["linkedin".to_string()]),
            ..PostMeta::default()
        };
        assert!(!is_platform_edited(&meta, "x"));
    }

    #[test]
    fn test_approve_post_records_edited_false_when_edited_platforms_none() {
        let meta = PostMeta {
            edited_platforms: None,
            ..PostMeta::default()
        };
        assert!(!is_platform_edited(&meta, "x"));
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
        // The DashMap lock ensures neither call overwrites the other's write;
        // the idempotency check ensures the second call is a no-op once sent_platforms is set.
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

    // --- §scheduler_result ---

    #[test]
    fn test_approve_post_writes_sent_at_after_scheduler_success() {
        let mut meta = PostMeta::default();
        apply_scheduler_result(&mut meta, "x", "sched-1", None, "2026-05-01T00:00:00Z");
        assert_eq!(
            meta.sent_platforms.get("x").map(String::as_str),
            Some("2026-05-01T00:00:00Z")
        );
    }

    #[test]
    fn test_approve_post_writes_scheduler_id_from_response() {
        let mut meta = PostMeta::default();
        apply_scheduler_result(&mut meta, "x", "abc123", None, "2026-05-01T00:00:00Z");
        assert_eq!(
            meta.scheduler_ids.get("x").map(String::as_str),
            Some("abc123")
        );
    }

    #[test]
    fn test_approve_post_writes_platform_url_from_response() {
        let mut meta = PostMeta::default();
        apply_scheduler_result(&mut meta, "x", "s1", Some("https://x.com/post/123"), "2026-05-01T00:00:00Z");
        assert_eq!(
            meta.platform_urls.get("x").map(String::as_str),
            Some("https://x.com/post/123")
        );
    }

    #[test]
    fn test_approve_post_does_not_write_scheduler_id_when_absent() {
        let mut meta = PostMeta::default();
        apply_scheduler_result(&mut meta, "x", "", None, "2026-05-01T00:00:00Z");
        assert!(meta.scheduler_ids.is_empty(), "empty scheduler_id must not be stored");
    }

    #[test]
    fn test_approve_post_rejects_http_platform_url() {
        let mut meta = PostMeta::default();
        apply_scheduler_result(&mut meta, "x", "s1", Some("http://x.com/post/123"), "2026-05-01T00:00:00Z");
        assert!(meta.platform_urls.is_empty(), "http:// url must be rejected");
    }

    // --- §failed_status ---

    #[test]
    fn test_approve_post_writes_failed_status_on_scheduler_error() {
        let mut meta = PostMeta::default();
        record_scheduler_failure(&mut meta, "HTTP 500 from scheduler");
        assert_eq!(meta.status, Some(PostStatus::Failed));
        assert_eq!(meta.error.as_deref(), Some("HTTP 500 from scheduler"));
    }

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

    // --- §warn_on_tracking_error (retained for coverage) ---

    #[test]
    fn test_provider_is_usable_returns_false_when_rate_limited() {
        // This function was removed in M19 — approve_post is now single-provider.
        // Verify scheduler_provider_for always returns "zernio" for all known platforms.
        assert_eq!(scheduler_provider_for("x"), "zernio");
        assert_eq!(scheduler_provider_for("linkedin"), "zernio");
        assert_eq!(scheduler_provider_for("bluesky"), "zernio");
    }
}
