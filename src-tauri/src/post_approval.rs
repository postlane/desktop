// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::scheduler_credentials::get_credential_keyring_key;
use crate::types::{PostMeta, SendResult};
use std::fs;
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

struct PlatformSendResults {
    platform_results: std::collections::HashMap<String, String>,
    scheduler_ids: std::collections::HashMap<String, String>,
    platform_urls: std::collections::HashMap<String, String>,
    fallback_provider: Option<String>,
}

/// Bundles the per-approval context passed into each platform send attempt.
struct SendContext<'a> {
    app_handle: &'a tauri::AppHandle,
    fallback_order: &'a [String],
    repo_id: &'a str,
    post_path: &'a std::path::Path,
    account_ids: &'a serde_json::Map<String, serde_json::Value>,
}

/// Stamps send results onto `meta` in-place: platform_results, sent_at, scheduler_ids, platform_urls.
fn apply_send_results_to_meta(meta: &mut PostMeta, results: &PlatformSendResults, sent_at: &str) {
    meta.platform_results = Some(results.platform_results.clone());
    meta.sent_at = Some(sent_at.to_string());
    if !results.scheduler_ids.is_empty() {
        meta.scheduler_ids = Some(results.scheduler_ids.clone());
    }
    if !results.platform_urls.is_empty() {
        meta.platform_urls = Some(results.platform_urls.clone());
    }
}

/// Build the `SendResult` returned to the caller after an approval attempt.
fn build_approval_result(results: &PlatformSendResults, partial_failure: bool) -> SendResult {
    if partial_failure {
        return SendResult {
            success: false,
            platform_results: Some(results.platform_results.clone()),
            error: Some(
                "One or more platforms failed. Retry to attempt the failed platforms.".to_string(),
            ),
            fallback_provider: results.fallback_provider.clone(),
        };
    }
    SendResult {
        success: true,
        platform_results: Some(results.platform_results.clone()),
        error: None,
        fallback_provider: results.fallback_provider.clone(),
    }
}

/// Returns true if `provider_name` should be attempted in the fallback chain.
/// Returns false if it is rate-limited this request or has exhausted its monthly quota.
fn provider_is_usable(
    provider_name: &str,
    usage_path: &std::path::Path,
    month: u32,
    year: u32,
    rate_limited: &std::collections::HashSet<String>,
) -> bool {
    if rate_limited.contains(provider_name) {
        return false;
    }
    !crate::scheduling::usage_tracker::is_at_limit_at(provider_name, usage_path, month, year)
}

/// Returns true if any platform result indicates a send failure.
fn any_platform_failed(results: &std::collections::HashMap<String, String>) -> bool {
    results.values().any(|v| v.starts_with("error:"))
}

/// Rejects post folder names that contain path separators or traversal sequences.
fn validate_post_folder(post_folder: &str) -> Result<(), String> {
    if post_folder.is_empty()
        || post_folder.contains('/')
        || post_folder.contains('\\')
        || post_folder.contains("..")
    {
        return Err(format!(
            "Invalid post folder name '{}': must not be empty or contain path separators.",
            post_folder
        ));
    }
    Ok(())
}

/// Returns the scheduler account_ids map from `.postlane/config.json`.
fn load_account_ids(
    canonical_path: &std::path::Path,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let config_path = canonical_path.join(".postlane/config.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;
    Ok(config["scheduler"]["account_ids"]
        .as_object()
        .cloned()
        .unwrap_or_default())
}

/// Resolves the registered repo_id from AppState for the given canonical path.
fn resolve_repo_id(state: &AppState, canonical_path: &std::path::Path) -> Result<String, String> {
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    repos
        .repos
        .iter()
        .find(|r| r.path == canonical_str)
        .ok_or_else(|| "Repository not found in state".to_string())
        .map(|r| r.id.clone())
}

fn read_platform_content(post_path: &std::path::Path, platform: &str) -> Result<String, String> {
    let content_file = post_path.join(format!("{}.md", platform));
    if !content_file.exists() {
        return Err(format!("Content file {}.md not found", platform));
    }
    fs::read_to_string(&content_file)
        .map_err(|e| format!("Failed to read {}.md: {}", platform, e))
}

fn parse_schedule(schedule: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(schedule?)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

fn get_keyring_cred(app: &tauri::AppHandle, provider: &str, repo_id: &str) -> Option<String> {
    let keys = get_credential_keyring_key(provider, Some(repo_id));
    for key in keys {
        if let Ok(Some(cred)) = app.keyring().get_password("postlane", &key) {
            return Some(cred);
        }
    }
    None
}

/// Tries to send one platform's content through the fallback provider chain.
/// Returns `(provider_name, PostScheduleResult)` on success.
/// On `ProviderError::RateLimit`, records the provider in `rate_limited` and tries the next.
/// `rate_limited` is shared across all platforms in one approval call so 429s are not retried
/// for subsequent platforms in the same request.
async fn send_platform_with_fallback(
    ctx: &SendContext<'_>,
    platform: &str,
    meta: &PostMeta,
    rate_limited: &mut std::collections::HashSet<String>,
) -> Result<(String, crate::providers::scheduling::PostScheduleResult), String> {
    use crate::providers::scheduling::ProviderError;
    use chrono::Datelike;

    let content = read_platform_content(ctx.post_path, platform)?;
    let scheduled_for = parse_schedule(meta.schedule.as_deref());
    let account_id = ctx.account_ids
        .get(platform)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let usage_path = crate::scheduling::usage_tracker::default_store_path()?;
    let now = chrono::Utc::now();
    let (month, year) = (now.month(), now.year() as u32);

    for provider_name in ctx.fallback_order {
        if !provider_is_usable(provider_name, &usage_path, month, year, rate_limited) {
            continue;
        }
        let api_key = match get_keyring_cred(ctx.app_handle, provider_name, ctx.repo_id) {
            Some(k) => k,
            None => continue,
        };
        let provider =
            match crate::scheduling::credential_router::build_provider(provider_name, api_key) {
                Ok(p) => p,
                Err(_) => continue,
            };
        match provider
            .schedule_post(
                &content,
                platform,
                scheduled_for,
                meta.image_url.as_deref(),
                account_id,
            )
            .await
        {
            Ok(result) => return Ok((provider_name.clone(), result)),
            Err(ProviderError::RateLimit(_)) => {
                rate_limited.insert(provider_name.clone());
                continue;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err(
        "All configured schedulers have reached their limits or have no credentials. \
         Check Settings \u{2192} Scheduler to add capacity or upgrade a provider."
            .to_string(),
    )
}

/// Sends each platform's content via the fallback-aware scheduler chain.
async fn send_via_provider(
    app_handle: &tauri::AppHandle,
    canonical_path: &std::path::Path,
    post_path: &std::path::Path,
    meta: &PostMeta,
    state: &AppState,
) -> Result<PlatformSendResults, String> {
    let account_ids = load_account_ids(canonical_path)?;
    let config_path = canonical_path.join(".postlane/config.json");
    let repo_id = resolve_repo_id(state, canonical_path)?;
    let fallback_order =
        crate::scheduling::credential_router::read_fallback_order(&config_path);
    let primary = fallback_order.first().cloned();
    let ctx = SendContext {
        app_handle,
        fallback_order: &fallback_order,
        repo_id: &repo_id,
        post_path,
        account_ids: &account_ids,
    };

    let mut platform_results = std::collections::HashMap::new();
    let mut scheduler_ids = std::collections::HashMap::new();
    let mut platform_urls = std::collections::HashMap::new();
    let mut fallback_used: Option<String> = None;
    let mut rate_limited = std::collections::HashSet::new();

    for platform in &meta.platforms {
        match send_platform_with_fallback(&ctx, platform, meta, &mut rate_limited).await {
            Ok((provider_name, post_result)) => {
                if Some(&provider_name) != primary.as_ref() {
                    fallback_used = Some(provider_name.clone());
                }
                crate::scheduling::usage_tracker::record_post(&provider_name).ok();
                platform_results.insert(platform.clone(), "success".to_string());
                scheduler_ids.insert(platform.clone(), post_result.scheduler_id);
                if let Some(url) = post_result.platform_url {
                    platform_urls.insert(platform.clone(), url);
                }
            }
            Err(e) => {
                platform_results.insert(platform.clone(), format!("error: {}", e));
            }
        }
    }

    Ok(PlatformSendResults {
        platform_results,
        scheduler_ids,
        platform_urls,
        fallback_provider: fallback_used,
    })
}

/// Verifies repo is registered, loads and returns (canonical_path, post_path, meta_path, meta).
fn load_approved_meta(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf, PostMeta), String> {
    validate_post_folder(post_folder)?;
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = {
        let repos = state.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos.repos.iter().any(|r| r.path == canonical_str)
    };
    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }
    let post_path = canonical_path.join(".postlane/posts").join(post_folder);
    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }
    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err(format!("meta.json not found at {}", meta_path.display()));
    }
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;
    let meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;
    Ok((canonical_path, post_path, meta_path, meta))
}

/// Writes updated meta atomically using the project's atomic_write utility.
fn write_sent_meta(meta_path: &std::path::Path, meta: &PostMeta) -> Result<(), String> {
    let json_content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    crate::init::atomic_write(meta_path, json_content.as_bytes())
        .map_err(|e| format!("Failed to write meta.json: {}", e))
}

pub async fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
    app: Option<&tauri::AppHandle>,
    consent: bool,
) -> Result<SendResult, String> {
    let (canonical_path, post_path, meta_path, mut meta) =
        load_approved_meta(repo_path, post_folder, state)?;

    // Idempotency guard: if already sent, return success without re-sending.
    if meta.status == "sent" {
        return Ok(SendResult {
            success: true,
            platform_results: meta.platform_results,
            error: None,
            fallback_provider: None,
        });
    }

    let results = if let Some(app_handle) = app {
        send_via_provider(app_handle, &canonical_path, &post_path, &meta, state).await?
    } else {
        let platform_results = meta
            .platforms
            .iter()
            .map(|p| (p.clone(), "success".to_string()))
            .collect();
        PlatformSendResults {
            platform_results,
            scheduler_ids: std::collections::HashMap::new(),
            platform_urls: std::collections::HashMap::new(),
            fallback_provider: None,
        }
    };

    let partial_failure = any_platform_failed(&results.platform_results);
    meta.status = if partial_failure { "failed" } else { "sent" }.to_string();
    apply_send_results_to_meta(&mut meta, &results, &chrono::Utc::now().to_rfc3339());
    write_sent_meta(&meta_path, &meta)?;

    if !partial_failure {
        state
            .telemetry
            .record(consent, "post_approved", serde_json::json!({"platforms": meta.platforms}));
    }
    Ok(build_approval_result(&results, partial_failure))
}

#[tauri::command]
pub async fn approve_post(
    repo_path: String,
    post_folder: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SendResult, String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    approve_post_impl(&repo_path, &post_folder, &state, Some(&app), consent).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};

    // --- §fix: validate_post_folder ---

    #[tokio::test]
    async fn test_rejects_path_traversal_in_post_folder() {
        let dir = std::env::temp_dir().join("postlane_test_traversal");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "../../../etc/passwd", &state, None, false).await;
        assert!(result.is_err(), "path traversal must be rejected");
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"), "error must name the problem");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_rejects_slash_in_post_folder() {
        let dir = std::env::temp_dir().join("postlane_test_slash_folder");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "valid/../../escape", &state, None, false).await;
        assert!(result.is_err(), "slash in folder must be rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_rejects_empty_post_folder() {
        let dir = std::env::temp_dir().join("postlane_test_empty_folder");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "", &state, None, false).await;
        assert!(result.is_err(), "empty folder must be rejected");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- §fix: any_platform_failed ---

    #[test]
    fn test_any_platform_failed_true_when_error_present() {
        let mut results = std::collections::HashMap::new();
        results.insert("x".to_string(), "success".to_string());
        results.insert("linkedin".to_string(), "error: rate limited".to_string());
        assert!(any_platform_failed(&results));
    }

    #[test]
    fn test_any_platform_failed_false_when_all_succeed() {
        let mut results = std::collections::HashMap::new();
        results.insert("x".to_string(), "success".to_string());
        results.insert("linkedin".to_string(), "success".to_string());
        assert!(!any_platform_failed(&results));
    }

    #[test]
    fn test_any_platform_failed_false_when_empty() {
        let results: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        assert!(!any_platform_failed(&results));
    }

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

    fn write_meta(dir: &std::path::Path, post_folder: &str, status: &str) {
        let post_path = dir.join(".postlane/posts").join(post_folder);
        std::fs::create_dir_all(&post_path).expect("create post dir");
        let meta = serde_json::json!({
            "status": status,
            "platforms": ["x"],
            "post_folder": post_folder,
        });
        std::fs::write(
            post_path.join("meta.json"),
            serde_json::to_string_pretty(&meta).expect("serialize"),
        )
        .expect("write meta.json");
        std::fs::write(post_path.join("x.md"), "test content").expect("write x.md");
    }

    #[tokio::test]
    async fn test_approve_already_sent_returns_success_without_scheduler() {
        let canonical = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join("postlane_test_approve_idempotent");
        std::fs::create_dir_all(&canonical).expect("create dir");

        write_meta(&canonical, "post-001", "sent");

        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);

        let result = approve_post_impl(&canonical_str, "post-001", &state, None, false).await;
        assert!(result.is_ok(), "already-sent post should return Ok: {:?}", result);
        assert!(result.unwrap().success, "success must be true");

        let _ = std::fs::remove_dir_all(&canonical);
    }

    #[tokio::test]
    async fn test_approve_ready_post_without_scheduler_succeeds() {
        let dir = std::env::temp_dir().join("postlane_test_approve_ready");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();

        write_meta(&canonical, "post-002", "ready");
        let state = make_state(&canonical_str);

        let result = approve_post_impl(&canonical_str, "post-002", &state, None, false).await;
        assert!(result.is_ok(), "ready post should be approved: {:?}", result);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_missing_meta_error_includes_path() {
        let dir = std::env::temp_dir().join("postlane_test_approve_no_meta");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();

        let post_path = canonical.join(".postlane/posts/post-003");
        std::fs::create_dir_all(&post_path).expect("create post dir");

        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-003", &state, None, false).await;
        assert!(result.is_err(), "missing meta.json should return Err");
        let err = result.unwrap_err();
        assert!(err.contains("meta.json"), "error must mention meta.json, got: {}", err);
        assert!(err.contains("post-003"), "error must include post folder path, got: {}", err);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_records_telemetry_when_consent_given() {
        let dir = std::env::temp_dir().join("postlane_test_approve_telemetry_yes");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_meta(&canonical, "post-tel-a", "ready");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-a", &state, None, true).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_no_telemetry_when_consent_not_given() {
        let dir = std::env::temp_dir().join("postlane_test_approve_telemetry_no");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_meta(&canonical, "post-tel-b", "ready");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-tel-b", &state, None, false).await;
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_approve_returns_no_fallback_without_provider() {
        let dir = std::env::temp_dir().join("postlane_test_approve_no_fallback");
        std::fs::create_dir_all(&dir).expect("create dir");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        write_meta(&canonical, "post-fb-none", "ready");
        let state = make_state(&canonical_str);
        let result = approve_post_impl(&canonical_str, "post-fb-none", &state, None, false).await;
        assert!(result.is_ok(), "{:?}", result);
        assert!(
            result.unwrap().fallback_provider.is_none(),
            "no fallback used when app is None"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- apply_send_results_to_meta ---

    fn empty_meta() -> PostMeta {
        PostMeta {
            status: "ready".into(),
            platforms: vec!["x".into()],
            platform_results: None,
            sent_at: None,
            schedule: None,
            scheduler_ids: None,
            platform_urls: None,
            error: None,
            image_url: None,
            image_source: None,
            image_attribution: None,
            trigger: None,
            llm_model: None,
            created_at: None,
        }
    }

    fn send_results(success: bool) -> PlatformSendResults {
        let mut platform_results = std::collections::HashMap::new();
        platform_results.insert("x".to_string(), if success { "success".into() } else { "error: failed".into() });
        PlatformSendResults {
            platform_results,
            scheduler_ids: [("x".into(), "sched-1".into())].into_iter().collect(),
            platform_urls: [("x".into(), "https://example.com/1".into())].into_iter().collect(),
            fallback_provider: None,
        }
    }

    #[test]
    fn apply_send_results_to_meta_sets_platform_results_and_sent_at() {
        let mut meta = empty_meta();
        let results = send_results(true);
        apply_send_results_to_meta(&mut meta, &results, "2026-01-01T00:00:00Z");
        assert_eq!(meta.platform_results.unwrap().get("x").map(String::as_str), Some("success"));
        assert_eq!(meta.sent_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn apply_send_results_to_meta_populates_scheduler_ids_and_urls() {
        let mut meta = empty_meta();
        apply_send_results_to_meta(&mut meta, &send_results(true), "2026-01-01T00:00:00Z");
        assert_eq!(meta.scheduler_ids.unwrap().get("x").map(String::as_str), Some("sched-1"));
        assert_eq!(meta.platform_urls.unwrap().get("x").map(String::as_str), Some("https://example.com/1"));
    }

    // --- build_approval_result ---

    #[test]
    fn build_approval_result_returns_success_when_no_failure() {
        let r = build_approval_result(&send_results(true), false);
        assert!(r.success);
        assert!(r.error.is_none());
    }

    #[test]
    fn build_approval_result_returns_failure_on_partial_failure() {
        let r = build_approval_result(&send_results(false), true);
        assert!(!r.success);
        assert!(r.error.is_some());
    }

    // --- provider_is_usable ---

    #[test]
    fn provider_is_usable_returns_false_when_rate_limited() {
        let mut rl = std::collections::HashSet::new();
        rl.insert("zernio".to_string());
        let fake_path = std::path::Path::new("/nonexistent");
        assert!(!provider_is_usable("zernio", fake_path, 1, 2026, &rl));
    }
}
