// SPDX-License-Identifier: BUSL-1.1
//
// Tauri command for validating an Upload-Post username and saving account IDs.
// The username is validated case-sensitively via the Upload-Post API before
// being written into account_ids for each connected platform.

use crate::account_id_store::save_account_id_impl;
use crate::app_state::AppState;
use crate::providers::scheduling::upload_post::UploadPostProvider;
use crate::scheduler_credentials::get_credential_keyring_key;
use tauri::Emitter;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

pub(crate) async fn validate_and_save_username(
    api_key: &str,
    username: &str,
    config_path: &Path,
) -> Result<Vec<String>, String> {
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Err("Username cannot be empty.".to_string());
    }
    let provider = UploadPostProvider::new(api_key.to_string());
    let platforms = provider
        .validate_profile(trimmed)
        .await
        .map_err(|e| e.to_string())?;
    for platform in &platforms {
        save_account_id_impl(config_path, platform, trimmed)
            .map_err(|e| format!("Failed to save account for {}: {}", platform, e))?;
    }
    Ok(platforms)
}

fn repo_config_path(repo_id: &str, state: &AppState) -> Result<PathBuf, String> {
    let repos = state.lock_repos()?;
    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not in registered repos", repo_id))?;
    Ok(PathBuf::from(&repo.path).join(".postlane/config.json"))
}

#[tauri::command]
pub async fn validate_upload_post_username(
    repo_id: String,
    username: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let config_path = repo_config_path(&repo_id, &state)?;
    let key = get_credential_keyring_key("upload_post", &repo_id);
    let api_key = app
        .keyring()
        .get_password("postlane", &key)
        .map_err(|e| format!("Failed to read keyring: {}", e))?
        .ok_or_else(|| {
            "No Upload Post API key configured. Save your API key in Settings first.".to_string()
        })?;
    let result = validate_and_save_username(&api_key, &username, &config_path).await?;
    let _ = app.emit("platform-connected", ());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(dir: &Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let config_path = config_dir.join("config.json");
        fs::write(&config_path, json).expect("write config.json");
        config_path
    }

    #[tokio::test]
    async fn test_validate_and_save_rejects_empty_username() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let err = validate_and_save_username("api-key", "  ", &config_path)
            .await
            .unwrap_err();
        assert!(err.contains("empty"), "got: {}", err);
    }

    #[tokio::test]
    async fn test_validate_and_save_returns_err_on_404() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/uploadposts/users/WrongCase");
            then.status(404);
        });

        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let mut provider = UploadPostProvider::new("api-key".to_string());
        provider.base_url = server.base_url();

        let err = provider.validate_profile("WrongCase").await.unwrap_err();
        assert!(err.to_string().contains("case-sensitive"));
        // config must be unchanged
        let content = fs::read_to_string(&config_path).expect("read");
        assert!(!content.contains("WrongCase"));
    }

    #[tokio::test]
    async fn test_validate_and_save_writes_account_ids_for_each_platform() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/uploadposts/users/myhandle");
            then.status(200).json_body(serde_json::json!({
                "profile": {
                    "username": "myhandle",
                    "social_accounts": {
                        "bluesky": {"display_name": "myhandle.bsky.social"},
                        "x": {"display_name": "myhandle"}
                    }
                },
                "success": true
            }));
        });

        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let mut provider = UploadPostProvider::new("api-key".to_string());
        provider.base_url = server.base_url();

        let platforms = provider.validate_profile("myhandle").await.unwrap();
        for platform in &platforms {
            save_account_id_impl(&config_path, platform, "myhandle").unwrap();
        }

        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(
            config["scheduler"]["account_ids"]["bluesky"].as_str(),
            Some("myhandle")
        );
        assert_eq!(
            config["scheduler"]["account_ids"]["x"].as_str(),
            Some("myhandle")
        );
    }

    #[tokio::test]
    async fn test_validate_and_save_returns_empty_list_when_no_platforms() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/uploadposts/users/bare");
            then.status(200).json_body(serde_json::json!({"username": "bare"}));
        });

        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let mut provider = UploadPostProvider::new("api-key".to_string());
        provider.base_url = server.base_url();

        let platforms = provider.validate_profile("bare").await.unwrap();
        assert!(platforms.is_empty());
        // nothing written to config
        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert!(config["scheduler"]["account_ids"].is_null());
    }

    #[tokio::test]
    async fn test_validate_and_save_trims_whitespace_from_username() {
        use httpmock::prelude::*;

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/uploadposts/users/trimmed");
            then.status(200).json_body(serde_json::json!({
                "profile": {
                    "username": "trimmed",
                    "social_accounts": {
                        "bluesky": {"display_name": "trimmed.bsky.social"}
                    }
                },
                "success": true
            }));
        });

        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let mut provider = UploadPostProvider::new("api-key".to_string());
        provider.base_url = server.base_url();

        let platforms = provider.validate_profile("trimmed").await.unwrap();
        for platform in &platforms {
            save_account_id_impl(&config_path, platform, "trimmed").unwrap();
        }

        let content = fs::read_to_string(&config_path).expect("read");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(
            config["scheduler"]["account_ids"]["bluesky"].as_str(),
            Some("trimmed")
        );
    }
}
