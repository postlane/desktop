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
        ayrshare::AyrshareProvider, buffer::BufferProvider, zernio::ZernioProvider,
    };

    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found".to_string());
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
        _ => return Err(format!("Unknown provider: {}", provider_name)),
    };

    let mut scheduler = state.scheduler.lock().await;
    *scheduler = Some(provider);

    Ok(())
}

pub async fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<SendResult, String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
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
        return Err("meta.json not found in post folder".to_string());
    }

    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    let mut platform_results = std::collections::HashMap::new();
    let mut scheduler_ids = std::collections::HashMap::new();
    let mut platform_urls: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if let Some(app_handle) = app {
        let repo_id = {
            let repos = state
                .repos
                .lock()
                .map_err(|e| format!("Failed to lock repos: {}", e))?;
            let repo = repos
                .repos
                .iter()
                .find(|r| r.path == canonical_str)
                .ok_or("Repository not found in state")?;
            repo.id.clone()
        };

        get_or_init_provider(app_handle, repo_path, &repo_id, state).await?;

        let config_path = canonical_path.join(".postlane/config.json");
        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.json: {}", e))?;

        let config: serde_json::Value = serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?;

        let account_ids = config["scheduler"]["account_ids"]
            .as_object()
            .cloned()
            .unwrap_or_default();

        for platform in &meta.platforms {
            let content_file = post_path.join(format!("{}.md", platform));
            let content = if content_file.exists() {
                fs::read_to_string(&content_file)
                    .map_err(|e| format!("Failed to read {}.md: {}", platform, e))?
            } else {
                return Err(format!("Content file {}.md not found", platform));
            };

            let scheduled_for = meta.schedule.as_ref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

            let account_id = account_ids
                .get(platform)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());

            let result = {
                let scheduler = state.scheduler.lock().await;
                let provider = scheduler.as_ref().ok_or("Provider not initialized")?;
                provider
                    .schedule_post(&content, platform, scheduled_for, meta.image_url.as_deref(), account_id)
                    .await
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
    } else {
        for platform in &meta.platforms {
            platform_results.insert(platform.clone(), "success".to_string());
        }
    }

    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    if !scheduler_ids.is_empty() {
        meta.scheduler_ids = Some(scheduler_ids);
    }
    if !platform_urls.is_empty() {
        meta.platform_urls = Some(platform_urls);
    }

    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
    })
}

#[tauri::command]
pub async fn approve_post(
    repo_path: String,
    post_folder: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SendResult, String> {
    approve_post_impl(&repo_path, &post_folder, &state, Some(&app)).await
}
