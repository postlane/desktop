// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::scheduler_credentials::get_credential_keyring_key;
use crate::types::{PostMeta, SendResult};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

pub async fn eager_init_provider_if_configured(
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<(), String> {
    let app = match app {
        Some(a) => a,
        None => return Ok(()),
    };

    let repo_info: Option<(String, String, String)> = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;

        repos.repos.iter().filter(|r| r.active).find_map(|repo| {
            let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
            if !config_path.exists() {
                return None;
            }

            let config_content = fs::read_to_string(&config_path).ok()?;
            let config: serde_json::Value = serde_json::from_str(&config_content).ok()?;
            let provider_name = config["scheduler"]["provider"].as_str()?;

            Some((repo.path.clone(), repo.id.clone(), provider_name.to_string()))
        })
    };

    let (repo_path, repo_id, provider_name) = match repo_info {
        Some(info) => info,
        None => return Ok(()),
    };

    let keyring_keys = get_credential_keyring_key(&provider_name, Some(&repo_id));

    let mut api_key: Option<String> = None;
    for key in keyring_keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => {
                api_key = Some(credential);
                break;
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    if api_key.is_none() {
        return Ok(());
    }

    get_or_init_provider(app, &repo_path, &repo_id, state).await
}

pub(crate) async fn get_or_init_provider(
    app: &tauri::AppHandle,
    repo_path: &str,
    repo_id: &str,
    state: &AppState,
) -> Result<(), String> {
    use crate::providers::scheduling::{
        ayrshare::AyrshareProvider, buffer::BufferProvider, outstand::OutstandProvider,
        publer::PublerProvider, substack_notes::SubstackNotesProvider, webhook::WebhookProvider,
        zernio::ZernioProvider,
    };

    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let config: serde_json::Value = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let provider_name = config["scheduler"]["provider"]
        .as_str()
        .ok_or("scheduler.provider not found in config.json")?;

    {
        let scheduler = state.scheduler.lock().await;
        if let Some(existing) = scheduler.as_ref() {
            if existing.name() == provider_name {
                return Ok(());
            }
        }
    }

    let keyring_keys = get_credential_keyring_key(provider_name, Some(repo_id));

    let mut api_key: Option<String> = None;
    for key in keyring_keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => {
                api_key = Some(credential);
                break;
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    let api_key = api_key.ok_or_else(|| {
        format!(
            "No {} API key configured. Add it in Settings → Scheduler.",
            provider_name
        )
    })?;

    let provider: Box<dyn crate::providers::scheduling::SchedulingProvider> = match provider_name {
        "zernio" => Box::new(ZernioProvider::new(api_key)),
        "buffer" => Box::new(BufferProvider::new(api_key)),
        "ayrshare" => Box::new(AyrshareProvider::new(api_key)),
        "publer" => Box::new(PublerProvider::new(api_key)),
        "outstand" => Box::new(OutstandProvider::new(api_key)),
        "substack_notes" => Box::new(SubstackNotesProvider::new(api_key)),
        "webhook" => Box::new(WebhookProvider::new(api_key)),
        _ => return Err(format!("Unknown provider: {}", provider_name)),
    };

    let mut scheduler = state.scheduler.lock().await;
    *scheduler = Some(provider);

    Ok(())
}

struct PlatformSendResults {
    platform_results: std::collections::HashMap<String, String>,
    scheduler_ids: std::collections::HashMap<String, String>,
    platform_urls: std::collections::HashMap<String, String>,
}

/// Reads the repo_id and loads `account_ids` from `.postlane/config.json`.
async fn load_provider_config(
    app_handle: &tauri::AppHandle,
    repo_path: &str,
    canonical_path: &std::path::Path,
    state: &AppState,
    canonical_str: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let repo_id = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos
            .repos
            .iter()
            .find(|r| r.path == canonical_str)
            .ok_or("Repository not found in state")?
            .id
            .clone()
    };

    get_or_init_provider(app_handle, repo_path, &repo_id, state).await?;

    let config_path = canonical_path.join(".postlane/config.json");
    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    Ok(config["scheduler"]["account_ids"]
        .as_object()
        .cloned()
        .unwrap_or_default())
}

/// Sends each platform's content via the scheduler provider.
/// Returns per-platform results, scheduler IDs, and platform URLs.
async fn send_via_provider(
    app_handle: &tauri::AppHandle,
    repo_path: &str,
    canonical_path: &std::path::Path,
    post_path: &std::path::Path,
    meta: &PostMeta,
    state: &AppState,
    canonical_str: &str,
) -> Result<PlatformSendResults, String> {
    let account_ids = load_provider_config(app_handle, repo_path, canonical_path, state, canonical_str).await?;

    let mut platform_results = std::collections::HashMap::new();
    let mut scheduler_ids = std::collections::HashMap::new();
    let mut platform_urls: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for platform in &meta.platforms {
        let content_file = post_path.join(format!("{}.md", platform));
        if !content_file.exists() {
            return Err(format!("Content file {}.md not found", platform));
        }
        let content = fs::read_to_string(&content_file)
            .map_err(|e| format!("Failed to read {}.md: {}", platform, e))?;

        let scheduled_for = meta.schedule.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&chrono::Utc))
        });
        let account_id = account_ids.get(platform).and_then(|v| v.as_str()).filter(|s| !s.is_empty());

        let result = {
            let scheduler = state.scheduler.lock().await;
            let provider = scheduler.as_ref().ok_or("Provider not initialized")?;
            provider.schedule_post(&content, platform, scheduled_for, meta.image_url.as_deref(), account_id).await
        };

        match result {
            Ok(post_result) => {
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

    Ok(PlatformSendResults { platform_results, scheduler_ids, platform_urls })
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
        return Ok(SendResult { success: true, platform_results: meta.platform_results, error: None });
    }

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let results = if let Some(app_handle) = app {
        send_via_provider(app_handle, repo_path, &canonical_path, &post_path, &meta, state, canonical_str).await?
    } else {
        let platform_results = meta.platforms.iter()
            .map(|p| (p.clone(), "success".to_string()))
            .collect();
        PlatformSendResults {
            platform_results,
            scheduler_ids: std::collections::HashMap::new(),
            platform_urls: std::collections::HashMap::new(),
        }
    };

    meta.status = "sent".to_string();
    meta.platform_results = Some(results.platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    if !results.scheduler_ids.is_empty() { meta.scheduler_ids = Some(results.scheduler_ids); }
    if !results.platform_urls.is_empty() { meta.platform_urls = Some(results.platform_urls); }

    write_sent_meta(&meta_path, &meta)?;
    state.telemetry.record(consent, "post_approved", serde_json::json!({"platforms": meta.platforms}));
    Ok(SendResult { success: true, platform_results: Some(results.platform_results), error: None })
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
        ).expect("write meta.json");
        // also write x.md so content exists
        std::fs::write(post_path.join("x.md"), "test content").expect("write x.md");
    }

    #[tokio::test]
    async fn test_approve_already_sent_returns_success_without_scheduler() {
        let canonical = std::fs::canonicalize(
            std::env::temp_dir()
        ).unwrap().join("postlane_test_approve_idempotent");
        std::fs::create_dir_all(&canonical).expect("create dir");

        write_meta(&canonical, "post-001", "sent");

        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_state(&canonical_str);

        // First call: already "sent" — should return Ok immediately
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

        // Create post folder but NOT meta.json
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
}
