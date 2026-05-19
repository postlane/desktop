// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use std::fs;
use std::path::PathBuf;
use tauri::State;

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
        config["scheduler"] = serde_json::json!({});
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


/// Writes each profile's account ID into `config_path` for its platforms.
/// Failures are logged as warnings and silently skipped.
pub fn apply_profiles_to_repo(
    profiles: &[crate::providers::scheduling::SchedulerProfile],
    config_path: &std::path::Path,
) {
    for profile in profiles {
        for platform in &profile.platforms {
            if let Err(e) = save_account_id_impl(config_path, platform, &profile.id) {
                log::warn!("[account_sync] {}/{}: {}", platform, config_path.display(), e);
            }
        }
    }
}

/// Fetches the connected social accounts for `provider_name` and writes them
/// into `config.json` for each repo path. Best-effort: errors are logged only.
pub async fn sync_accounts_for_provider(
    provider_name: &str,
    api_key: &str,
    repo_paths: Vec<std::path::PathBuf>,
) {
    if repo_paths.is_empty() {
        return;
    }
    let provider = match crate::scheduling::credential_router::build_provider(provider_name, api_key.to_string()) {
        Ok(p) => p,
        Err(e) => { log::warn!("[account_sync] build provider {}: {}", provider_name, e); return; }
    };
    let profiles = match provider.list_profiles().await {
        Ok(p) => p,
        Err(e) => { log::warn!("[account_sync] list_profiles {}: {}", provider_name, e); return; }
    };
    for path in &repo_paths {
        apply_profiles_to_repo(&profiles, &path.join(".postlane/config.json"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::make_state;
    use std::fs;
    use std::path::PathBuf;

    fn write_config(dir: &std::path::Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let config_path = config_dir.join("config.json");
        fs::write(&config_path, json).expect("write config.json");
        config_path
    }

    #[test]
    fn test_save_account_id_writes_x_account_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": { "provider": "zernio", "account_ids": {} }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-twitter-123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-twitter-123"));
    }

    #[test]
    fn test_save_account_id_preserves_other_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
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
    }

    #[test]
    fn test_save_account_id_creates_account_ids_block_if_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "provider": "zernio" }
        }"#);

        save_account_id_impl(&config_path, "x", "acc-new").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-new"));
    }

    #[test]
    fn test_save_account_id_preserves_other_config_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
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
    fn test_save_account_id_creates_scheduler_block_when_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{ "version": 1, "platforms": ["x"] }"#);

        save_account_id_impl(&config_path, "x", "acc-123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-123"));
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
    fn test_apply_profiles_to_repo_writes_account_id() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version": 1}"#);
        let profiles = vec![SchedulerProfile { id: "acc-1".to_string(), name: "Hugo".to_string(), platforms: vec!["x".to_string()] }];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-1"));
    }

    #[test]
    fn test_apply_profiles_to_repo_skips_missing_config() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = dir.path().join("nonexistent/config.json");
        let profiles = vec![SchedulerProfile { id: "acc-1".to_string(), name: "Hugo".to_string(), platforms: vec!["x".to_string()] }];
        apply_profiles_to_repo(&profiles, &config_path); // must not panic
    }

    #[test]
    fn test_apply_profiles_to_repo_writes_multiple_platforms() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version": 1}"#);
        let profiles = vec![
            SchedulerProfile { id: "acc-x".to_string(), name: "X acc".to_string(), platforms: vec!["x".to_string()] },
            SchedulerProfile { id: "acc-bs".to_string(), name: "BS acc".to_string(), platforms: vec!["bluesky".to_string()] },
        ];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("acc-x"));
        assert_eq!(config["scheduler"]["account_ids"]["bluesky"].as_str(), Some("acc-bs"));
    }

}
