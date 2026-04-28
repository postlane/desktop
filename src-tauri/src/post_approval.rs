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
/// On `ProviderError::RateLimit`, marks the provider as temporarily exhausted and tries the next.
async fn send_platform_with_fallback(
    app_handle: &tauri::AppHandle,
    fallback_order: &[String],
    repo_id: &str,
    post_path: &std::path::Path,
    platform: &str,
    meta: &PostMeta,
    account_ids: &serde_json::Map<String, serde_json::Value>,
) -> Result<(String, crate::providers::scheduling::PostScheduleResult), String> {
    use crate::providers::scheduling::ProviderError;
    use chrono::Datelike;

    let content = read_platform_content(post_path, platform)?;
    let scheduled_for = parse_schedule(meta.schedule.as_deref());
    let account_id = account_ids
        .get(platform)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let usage_path = crate::scheduling::usage_tracker::default_store_path()?;
    let now = chrono::Utc::now();
    let (month, year) = (now.month(), now.year() as u32);
    let mut rate_limited = std::collections::HashSet::new();

    for provider_name in fallback_order {
        if rate_limited.contains(provider_name) {
            continue;
        }
        if crate::scheduling::usage_tracker::is_at_limit_at(
            provider_name,
            &usage_path,
            month,
            year,
        ) {
            continue;
        }
        let api_key = match get_keyring_cred(app_handle, provider_name, repo_id) {
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

    let mut platform_results = std::collections::HashMap::new();
    let mut scheduler_ids = std::collections::HashMap::new();
    let mut platform_urls = std::collections::HashMap::new();
    let mut fallback_used: Option<String> = None;

    for platform in &meta.platforms {
        match send_platform_with_fallback(
            app_handle,
            &fallback_order,
            &repo_id,
            post_path,
            platform,
            meta,
            &account_ids,
        )
        .await
        {
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

/// Writes updated meta atomically.
fn write_sent_meta(meta_path: &std::path::Path, meta: &PostMeta) -> Result<(), String> {
    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;
    Ok(())
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

    meta.status = "sent".to_string();
    meta.platform_results = Some(results.platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    if !results.scheduler_ids.is_empty() {
        meta.scheduler_ids = Some(results.scheduler_ids);
    }
    if !results.platform_urls.is_empty() {
        meta.platform_urls = Some(results.platform_urls);
    }

    write_sent_meta(&meta_path, &meta)?;
    state
        .telemetry
        .record(consent, "post_approved", serde_json::json!({"platforms": meta.platforms}));
    Ok(SendResult {
        success: true,
        platform_results: Some(results.platform_results),
        error: None,
        fallback_provider: results.fallback_provider,
    })
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
}
