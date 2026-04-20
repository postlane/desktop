// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use std::fs;
use std::path::PathBuf;
use tauri::State;

pub fn get_repo_config_impl(
    repo_id: &str,
    state: &AppState,
) -> Result<(String, String), String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not in registered repos", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let provider_name = config["scheduler"]["provider"]
        .as_str()
        .ok_or("scheduler.provider not set in config.json")?
        .to_string();

    Ok((repo.path.clone(), provider_name))
}

pub fn save_account_id_impl(
    config_path: &std::path::Path,
    platform: &str,
    account_id: &str,
) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    if !config["scheduler"].is_object() {
        return Err("config.json is missing the 'scheduler' block".to_string());
    }

    if !config["scheduler"]["account_ids"].is_object() {
        config["scheduler"]["account_ids"] = serde_json::json!({});
    }

    config["scheduler"]["account_ids"][platform] = serde_json::json!(account_id);

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.json: {}", e))?;

    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, &serialized)
        .map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&tmp_path, config_path)
        .map_err(|e| format!("Failed to rename temp config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn list_profiles_for_repo(
    repo_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<crate::providers::scheduling::SchedulerProfile>, String> {
    use crate::providers::scheduling::{ProviderError, SchedulingProvider};
    use crate::providers::scheduling::ayrshare::AyrshareProvider;
    use crate::providers::scheduling::buffer::BufferProvider;
    use crate::providers::scheduling::zernio::ZernioProvider;
    use crate::scheduler_credentials::get_credential_keyring_key;
    use tauri_plugin_keyring::KeyringExt;

    let (_repo_path, provider_name) = get_repo_config_impl(&repo_id, &state)?;

    let keyring_keys = get_credential_keyring_key(&provider_name, Some(&repo_id));
    let mut api_key: Option<String> = None;
    for key in &keyring_keys {
        if let Ok(Some(k)) = app.keyring().get_password("postlane", key) {
            api_key = Some(k);
            break;
        }
    }
    let api_key = api_key.ok_or_else(|| {
        format!("No {} API key configured. Add it in Settings → Scheduler.", provider_name)
    })?;

    let provider: Box<dyn SchedulingProvider> = match provider_name.as_str() {
        "zernio" => Box::new(ZernioProvider::new(api_key)),
        "buffer" => Box::new(BufferProvider::new(api_key)),
        "ayrshare" => Box::new(AyrshareProvider::new(api_key)),
        other => return Err(format!("Unknown scheduler provider: {}", other)),
    };

    provider.list_profiles().await.map_err(|e: ProviderError| e.to_string())
}

#[tauri::command]
pub fn save_account_id(
    repo_id: String,
    platform: String,
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not in registered repos", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    save_account_id_impl(&config_path, &platform, &account_id)
}

#[tauri::command]
pub fn get_account_ids(
    repo_id: String,
    state: State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    if !config_path.exists() {
        return Ok(std::collections::HashMap::new());
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let account_ids = match config["scheduler"]["account_ids"].as_object() {
        Some(obj) => obj
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        None => std::collections::HashMap::new(),
    };

    Ok(account_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;
    use std::path::PathBuf;

    fn make_state(repos: Vec<Repo>) -> AppState {
        AppState::new(ReposConfig { version: 1, repos })
    }

    fn write_config(dir: &std::path::Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let config_path = config_dir.join("config.json");
        fs::write(&config_path, json).expect("write config.json");
        config_path
    }

    #[test]
    fn test_save_account_id_writes_x_account_id() {
        let dir = std::env::temp_dir().join("postlane_test_save_account_id_x_ac");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": { "provider": "zernio", "account_ids": {} }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-twitter-123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-twitter-123"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_account_id_preserves_other_platforms() {
        let dir = std::env::temp_dir().join("postlane_test_save_account_id_preserve_ac");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": {
                "provider": "zernio",
                "account_ids": { "x": "acc-twitter-existing" }
            }
        }"#);

        save_account_id_impl(&config_path, "bluesky", "acc-bluesky-456").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-twitter-existing"));
        assert_eq!(config["scheduler"]["account_ids"]["bluesky"].as_str(), Some("acc-bluesky-456"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_account_id_creates_account_ids_block_if_missing() {
        let dir = std::env::temp_dir().join("postlane_test_save_account_id_create_block_ac");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "scheduler": { "provider": "zernio" }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-new").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-new"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_account_id_preserves_other_config_fields() {
        let dir = std::env::temp_dir().join("postlane_test_save_account_id_fields_ac");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "base_url": "https://postlane.dev",
            "repo_type": "saas-product",
            "scheduler": { "provider": "zernio", "account_ids": {} },
            "llm": { "provider": "anthropic", "model": "claude-sonnet-4-6" }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-x").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["version"].as_i64(), Some(1));
        assert_eq!(config["base_url"].as_str(), Some("https://postlane.dev"));
        assert_eq!(config["repo_type"].as_str(), Some("saas-product"));
        assert_eq!(config["scheduler"]["provider"].as_str(), Some("zernio"));
        assert_eq!(config["llm"]["model"].as_str(), Some("claude-sonnet-4-6"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_account_id_errors_when_config_missing() {
        let result = save_account_id_impl(
            std::path::Path::new("/nonexistent/path/.postlane/config.json"),
            "x",
            "acc-123",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_save_account_id_errors_when_no_scheduler_block() {
        let dir = std::env::temp_dir().join("postlane_test_save_account_id_no_scheduler_ac");
        let config_path = write_config(&dir, r#"{ "version": 1, "platforms": ["x"] }"#);

        let result = save_account_id_impl(&config_path, "x", "acc-123");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheduler"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_account_id_rejects_unregistered_repo() {
        let state = make_state(vec![]);
        let repos = state.repos.lock().expect("lock");
        let result: Result<(), String> = repos.repos.iter()
            .find(|r| r.id == "nonexistent")
            .map(|_| Ok(()))
            .unwrap_or_else(|| Err(format!("Repo '{}' not in registered repos", "nonexistent")));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in registered repos"));
    }

    #[test]
    fn test_get_repo_config_returns_provider_and_path() {
        let dir = std::env::temp_dir().join("postlane_test_get_repo_config_ac");
        write_config(&dir, r#"{
            "version": 1,
            "scheduler": { "provider": "zernio", "profile_id": "" }
        }"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_config_impl("r1", &state).expect("should succeed");
        assert_eq!(result.0, dir.to_str().unwrap());
        assert_eq!(result.1, "zernio");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_config_errors_on_missing_repo() {
        let state = make_state(vec![]);
        let result = get_repo_config_impl("nonexistent", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in registered repos"));
    }

    #[test]
    fn test_get_repo_config_errors_on_missing_config_file() {
        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: "/nonexistent/path/that/cannot/exist".to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repo_config_impl("r1", &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repo_config_errors_when_provider_missing_from_config() {
        let dir = std::env::temp_dir().join("postlane_test_get_repo_config_no_provider_ac");
        write_config(&dir, r#"{ "version": 1, "platforms": ["x"] }"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_config_impl("r1", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheduler.provider"));

        let _ = fs::remove_dir_all(&dir);
    }
}
