// SPDX-License-Identifier: BUSL-1.1

//! Testable writer for scheduler credentials.
//!
//! The `CredentialEnv` struct and `save_scheduler_credential_core` function
//! contain the business logic extracted from the `save_scheduler_credential`
//! Tauri command, allowing it to be unit tested without a live AppHandle.

use crate::scheduler_credentials::get_credential_keyring_key;

/// Return type for `save_scheduler_credential_core`.
///
/// `account_names` is the merged display-name map returned to the frontend.
/// `warnings` collects non-fatal sync failures (config write errors, account
/// sync errors) that should be logged at the Tauri command level. An empty
/// vec means all secondary operations succeeded.
#[derive(Debug)]
pub struct CredentialSaveResult {
    pub account_names: std::collections::HashMap<String, String>,
    pub warnings: Vec<String>,
}

/// Injectable environment for `save_scheduler_credential_core`.
/// Groups the three keyring/mastodon closures so the function stays under
/// the 7-argument clippy limit.
pub struct CredentialEnv<'a> {
    /// Writes `(key, value)` to the "postlane" keyring service.
    pub set_keyring: &'a (dyn Fn(&str, &str) -> Result<(), String> + Send + Sync),
    /// Returns `true` if Mastodon is connected for the given project ID.
    pub has_mastodon_active: &'a (dyn Fn(&str) -> bool + Send + Sync),
    /// Returns `true` if the given keyring key is present in "postlane".
    pub has_keyring_key: &'a (dyn Fn(&str) -> bool + Send + Sync),
}

/// Testable core of `save_scheduler_credential`.
///
/// Writes the credential to the keyring, syncs account IDs into each repo's
/// `config.json`, syncs `connected_platforms`, and returns the merged
/// `account_names` map. No `AppHandle` or `State` parameters.
pub async fn save_scheduler_credential_core(
    provider: &str,
    api_key: &str,
    repo_id: &str,
    username: Option<&str>,
    matching_paths: &[std::path::PathBuf],
    env: &CredentialEnv<'_>,
) -> Result<CredentialSaveResult, String> {
    let mut warnings: Vec<String> = Vec::new();

    // Hard fail: if the keyring write fails, the credential is not stored at all.
    let keyring_key = get_credential_keyring_key(provider, repo_id);
    (env.set_keyring)(&keyring_key, api_key)
        .map_err(|e| format!("Failed to store credential: {}", e))?;

    // Soft fail: account sync writes account IDs into repo config.json files.
    // A failure here means the scheduler will fall back to the account it had
    // before — the user's connection is live, so we warn but do not abort.
    if provider == "upload_post" {
        if let Some(uname) = username {
            use crate::providers::scheduling::upload_post::UploadPostProvider;
            let up = UploadPostProvider::new(api_key.to_string());
            let platforms = up.validate_profile(uname).await
                .map_err(|e| format!("Username '{}' not recognised by Upload Post: {}", uname, e))?;
            for repo_path in matching_paths {
                let config_path = repo_path.join(".postlane/config.json");
                let path_warnings = crate::account_config::write_upload_post_account(
                    uname, &platforms, &config_path,
                );
                warnings.extend(path_warnings);
            }
        }
    } else {
        let path_bufs: Vec<std::path::PathBuf> = matching_paths.to_vec();
        if let Err(e) = crate::account_config::sync_accounts_for_provider(provider, api_key, path_bufs).await {
            warnings.push(format!("account sync for '{}' failed — scheduler may use stale account IDs: {}", provider, e));
        }
    }

    // Soft fail: connected_platforms sync updates the display state in config.json.
    // A write failure here means the queue may show stale badge state — log and continue.
    for repo_path in matching_paths {
        let config_path = repo_path.join(".postlane/config.json");
        let project_id = crate::connected_platforms::read_project_id_from_config(&config_path);
        let mastodon_active = project_id.as_deref().map(env.has_mastodon_active).unwrap_or(false);
        if let Err(e) = crate::platform_config_sync::sync_connected_platforms_to_config_impl(
            &config_path, repo_id, mastodon_active, env.has_keyring_key,
        ) {
            warnings.push(format!("platform sync for '{}' failed — queue badges may be stale: {}", repo_path.display(), e));
        }
    }

    let mut account_names = std::collections::HashMap::new();
    for repo_path in matching_paths {
        let config_path = repo_path.join(".postlane/config.json");
        if let Ok(names) = crate::account_name_store::get_account_names_impl(&config_path) {
            account_names.extend(names);
        }
    }
    Ok(CredentialSaveResult { account_names, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_core_result_has_warnings_when_platform_sync_fails() {
        // Place a directory at config.json — sync_connected_platforms_to_config_impl
        // calls fs::read_to_string which fails with "Is a directory", returning Err.
        // Verifies that per-repo sync errors surface as warnings, not hard failures.
        let dir = tempfile::TempDir::new().expect("temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("create .postlane");
        // config.json is a directory — read_to_string returns IsADirectory
        std::fs::create_dir_all(postlane.join("config.json")).expect("create dir at config.json path");

        let env = CredentialEnv {
            set_keyring: &|_, _| Ok(()),
            has_mastodon_active: &|_| false,
            has_keyring_key: &|_| false,
        };
        let result = save_scheduler_credential_core(
            "zernio", "key", "repo-id", None, &[dir.path().to_path_buf()], &env,
        ).await;

        assert!(result.is_ok(), "credential save must succeed even when platform sync fails");
        let save_result = result.unwrap();
        assert!(
            save_result.warnings.iter().any(|w| w.contains("platform sync")),
            "expected a platform-sync warning in: {:?}", save_result.warnings
        );
    }

    #[tokio::test]
    async fn test_core_result_has_no_warnings_on_clean_success() {
        let env = CredentialEnv {
            set_keyring: &|_, _| Ok(()),
            has_mastodon_active: &|_| false,
            has_keyring_key: &|_| false,
        };
        let result = save_scheduler_credential_core(
            "zernio", "key", "repo-id", None, &[], &env,
        ).await;
        assert!(result.is_ok());
        assert!(result.unwrap().warnings.is_empty(), "expected no warnings with no paths");
    }

    #[tokio::test]
    async fn test_save_credential_core_writes_keyring_and_returns_account_names() {
        let keyring_calls: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
            std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let keyring_ref = keyring_calls.clone();
        let env = CredentialEnv {
            set_keyring: &|key, _val| {
                keyring_ref.lock().unwrap().push(key.to_string());
                Ok(())
            },
            has_mastodon_active: &|_| false,
            has_keyring_key: &|_| false,
        };
        let result = save_scheduler_credential_core(
            "zernio", "test-api-key", "repo-abc", None, &[], &env,
        ).await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let calls = keyring_calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &["zernio/repo-abc"]);
    }

    #[tokio::test]
    async fn test_save_credential_core_returns_empty_map_when_no_paths() {
        let env = CredentialEnv {
            set_keyring: &|_key, _val| Ok(()),
            has_mastodon_active: &|_| false,
            has_keyring_key: &|_| false,
        };
        let result = save_scheduler_credential_core(
            "zernio", "test-api-key", "repo-abc", None, &[], &env,
        ).await;
        assert!(result.is_ok());
        assert!(result.unwrap().account_names.is_empty());
    }

    #[tokio::test]
    async fn test_save_credential_core_returns_err_when_keyring_fails() {
        let env = CredentialEnv {
            set_keyring: &|_key, _val| Err("keyring locked".to_string()),
            has_mastodon_active: &|_| false,
            has_keyring_key: &|_| false,
        };
        let result = save_scheduler_credential_core(
            "zernio", "test-api-key", "repo-abc", None, &[], &env,
        ).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to store credential"));
    }
}
