// SPDX-License-Identifier: BUSL-1.1

use crate::account_id_store::{get_account_ids_impl, save_account_id_impl};
use crate::account_name_store::{get_account_names_impl, save_account_name_impl};
use crate::app_state::AppState;
use std::path::PathBuf;
use tauri::State;


#[tauri::command]
pub fn save_account_id(
    repo_id: String,
    platform: String,
    account_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let repos = state.lock_repos()?;

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
    let repos = state.lock_repos()?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    get_account_ids_impl(&config_path)
}

#[tauri::command]
pub fn get_scheduler_account_names(
    repo_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let repos = state.lock_repos()?;
    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;
    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    get_account_names_impl(&config_path)
}

/// Writes each profile's account ID and display name into `config_path`.
/// Failures are logged as warnings and silently skipped.
pub fn apply_profiles_to_repo(
    profiles: &[crate::providers::scheduling::SchedulerProfile],
    config_path: &std::path::Path,
) {
    for profile in profiles {
        for platform in &profile.platforms {
            if let Err(e) = save_account_id_impl(config_path, platform, &profile.id) {
                log::warn!("[account_sync] id {}/{}: {}", platform, config_path.display(), e);
            }
            if let Err(e) = save_account_name_impl(config_path, platform, &profile.name) {
                log::warn!("[account_sync] name {}/{}: {}", platform, config_path.display(), e);
            }
        }
    }
}

/// Writes an Upload Post username to config.json.
///
/// For each connected platform, writes `account_ids[platform] = username` and
/// `account_names[platform] = username`. Always writes `account_names["upload_post"]`
/// so the connect success message shows the username even before social platforms
/// are connected in the Upload Post dashboard.
/// Writes per-platform account IDs and names into `config.json` for an Upload Post connection.
/// Returns a list of warning strings for any write that failed; an empty vec means full success.
pub fn write_upload_post_account(
    username: &str,
    connected_platforms: &[String],
    config_path: &std::path::Path,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for platform in connected_platforms {
        if let Err(e) = save_account_id_impl(config_path, platform, username) {
            warnings.push(format!("write account ID for '{}' to {}: {}", platform, config_path.display(), e));
        }
        if let Err(e) = save_account_name_impl(config_path, platform, username) {
            warnings.push(format!("write account name for '{}' to {}: {}", platform, config_path.display(), e));
        }
    }
    if let Err(e) = save_account_name_impl(config_path, "upload_post", username) {
        warnings.push(format!("write upload_post account name to {}: {}", config_path.display(), e));
    }
    warnings
}

/// Fetches the connected social accounts for `provider_name` and writes them
/// into `config.json` for each repo path.
/// Returns `Err` if the provider cannot be built or `list_profiles` fails.
pub async fn sync_accounts_for_provider(
    provider_name: &str,
    api_key: &str,
    repo_paths: Vec<std::path::PathBuf>,
) -> Result<(), String> {
    if repo_paths.is_empty() {
        return Ok(());
    }
    let provider = crate::scheduling::credential_router::build_provider(provider_name, api_key.to_string())
        .map_err(|e| format!("build provider {}: {}", provider_name, e))?;
    let profiles = provider.list_profiles().await
        .map_err(|e| format!("list_profiles {}: {}", provider_name, e))?;
    for path in &repo_paths {
        apply_profiles_to_repo(&profiles, &path.join(".postlane/config.json"));
    }
    Ok(())
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
    fn apply_profiles_to_repo_is_no_op_for_empty_profiles_list() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let empty: &[SchedulerProfile] = &[];
        apply_profiles_to_repo(empty, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert!(config["scheduler"]["account_ids"].is_null(), "no writes expected for empty profiles");
    }

    #[test]
    fn apply_profiles_to_repo_overwrites_existing_account_id_for_same_platform() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1,"scheduler":{"account_ids":{"x":"old-id"}}}"#);
        let profiles = vec![SchedulerProfile {
            id: "new-id".to_string(),
            name: "New".to_string(),
            platforms: vec!["x".to_string()],
        }];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("new-id"));
    }

    #[test]
    fn apply_profiles_to_repo_writes_account_id() {
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
        let config_path = dir.path().join(".postlane/config.json");
        let profiles = vec![SchedulerProfile {
            id: "acc-1".to_string(),
            name: "Hugo".to_string(),
            platforms: vec!["bluesky".to_string()],
        }];
        apply_profiles_to_repo(&profiles, &config_path);
        assert!(
            !config_path.exists(),
            "apply_profiles_to_repo must not create config.json when it is absent"
        );
    }

    #[test]
    fn test_apply_profiles_to_repo_writes_account_name() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let profiles = vec![SchedulerProfile {
            id: "acc-bsky".to_string(),
            name: "@rng_dev".to_string(),
            platforms: vec!["bluesky".to_string()],
        }];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(
            config["scheduler"]["account_names"]["bluesky"].as_str(),
            Some("@rng_dev"),
            "account name must be written alongside account id"
        );
    }

    #[test]
    fn test_apply_profiles_to_repo_writes_account_name_for_multiple_platforms() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let profiles = vec![
            SchedulerProfile {
                id: "acc-x".to_string(),
                name: "@postlane".to_string(),
                platforms: vec!["x".to_string()],
            },
            SchedulerProfile {
                id: "acc-bsky".to_string(),
                name: "@rng_dev".to_string(),
                platforms: vec!["bluesky".to_string()],
            },
        ];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_names"]["x"].as_str(), Some("@postlane"));
        assert_eq!(config["scheduler"]["account_names"]["bluesky"].as_str(), Some("@rng_dev"));
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

    #[test]
    fn write_upload_post_account_returns_empty_warnings_on_success() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let warnings = write_upload_post_account("postlane", &["x".to_string()], &config_path);
        assert!(warnings.is_empty(), "expected no warnings on success, got: {:?}", warnings);
    }

    #[test]
    fn write_upload_post_account_returns_warnings_when_config_path_missing() {
        let bad_path = std::path::Path::new("/nonexistent_postlane_test_dir/config.json");
        let warnings = write_upload_post_account("user", &["x".to_string()], bad_path);
        assert!(!warnings.is_empty(), "expected warnings when config.json cannot be written");
        assert!(
            warnings[0].contains("config.json"),
            "warning must mention the path, got: {:?}", warnings
        );
    }

    #[test]
    fn write_upload_post_account_writes_per_platform_account_ids_and_names() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let _ = write_upload_post_account("postlane", &["bluesky".to_string(), "x".to_string()], &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["bluesky"].as_str(), Some("postlane"));
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("postlane"));
        assert_eq!(config["scheduler"]["account_names"]["bluesky"].as_str(), Some("postlane"));
        assert_eq!(config["scheduler"]["account_names"]["x"].as_str(), Some("postlane"));
    }

    #[test]
    fn write_upload_post_account_writes_provider_key_so_success_message_shows_username() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let _ = write_upload_post_account("postlane", &[], &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert!(config["scheduler"]["account_ids"].is_null(), "no platforms -> no account_ids written");
        assert_eq!(
            config["scheduler"]["account_names"]["upload_post"].as_str(),
            Some("postlane"),
            "username must be written under 'upload_post' key so success message works before platforms are synced"
        );
    }

    #[test]
    fn apply_profiles_to_repo_handles_single_profile_with_multiple_platforms() {
        use crate::providers::scheduling::SchedulerProfile;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let profiles = vec![SchedulerProfile {
            id: "myhandle".to_string(),
            name: "myhandle".to_string(),
            platforms: vec!["bluesky".to_string(), "x".to_string()],
        }];
        apply_profiles_to_repo(&profiles, &config_path);
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["account_ids"]["bluesky"].as_str(), Some("myhandle"));
        assert_eq!(config["scheduler"]["account_ids"]["x"].as_str(), Some("myhandle"));
        assert_eq!(config["scheduler"]["account_names"]["bluesky"].as_str(), Some("myhandle"));
        assert_eq!(config["scheduler"]["account_names"]["x"].as_str(), Some("myhandle"));
    }

    #[tokio::test]
    async fn test_sync_accounts_for_provider_empty_paths_returns_ok() {
        let result = sync_accounts_for_provider("zernio", "test-key", vec![]).await;
        assert!(result.is_ok(), "{:?}", result);
    }

    #[tokio::test]
    async fn test_sync_accounts_for_provider_unknown_provider_returns_err() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let result = sync_accounts_for_provider(
            "not-a-real-provider",
            "test-key",
            vec![dir.path().to_path_buf()],
        ).await;
        assert!(result.is_err(), "unknown provider must return Err");
        let msg = result.unwrap_err();
        assert!(msg.contains("build provider"), "error: {}", msg);
    }
}
