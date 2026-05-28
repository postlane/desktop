// SPDX-License-Identifier: BUSL-1.1

use crate::account_id_store::get_account_ids_impl;
use crate::app_state::AppState;
use crate::mastodon_connection::{active_instance_key, KEYRING_SERVICE};
use crate::scheduler_credentials::{get_credential_keyring_key, VALID_PROVIDERS};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

/// Returns platform slugs that have a working connection for `repo_id`.
///
/// A platform is connected when either:
/// (a) Mastodon active instance is present in keyring (`mastodon_active = true`), or
/// (b) `config.json → scheduler.account_ids[platform]` is non-empty AND at least one
///     scheduler provider has a credential stored in keyring for this repo.
pub(crate) fn list_connected_platforms_impl(
    config_path: &Path,
    repo_id: &str,
    mastodon_active: bool,
    has_keyring_key: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    let mut platforms: BTreeSet<String> = BTreeSet::new();

    if mastodon_active {
        platforms.insert("mastodon".to_string());
    }

    if let Ok(account_ids) = get_account_ids_impl(config_path) {
        let has_scheduler = VALID_PROVIDERS
            .iter()
            .any(|p| has_keyring_key(&get_credential_keyring_key(p, repo_id)));
        if has_scheduler {
            for (platform, id) in &account_ids {
                if !id.is_empty() {
                    platforms.insert(platform.clone());
                }
            }
        }
    }

    platforms.into_iter().collect()
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

pub(crate) fn read_project_id_from_config(config_path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(config_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    json["project_id"].as_str().map(str::to_string)
}

/// Returns the platform slugs for which a connection exists for the given repo.
/// Called once per repo group when the queue loads (not per post card).
#[tauri::command]
pub fn list_connected_platforms(
    repo_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let config_path = repo_config_path(&repo_id, &state)?;
    let mastodon_active = read_project_id_from_config(&config_path)
        .map(|project_id| {
            app.keyring()
                .get_password(KEYRING_SERVICE, &active_instance_key(&project_id))
                .unwrap_or(None)
                .is_some()
        })
        .unwrap_or(false);
    Ok(list_connected_platforms_impl(
        &config_path,
        &repo_id,
        mastodon_active,
        &|key| {
            app.keyring()
                .get_password(KEYRING_SERVICE, key)
                .unwrap_or(None)
                .is_some()
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn write_config(dir: &std::path::Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let path = config_dir.join("config.json");
        fs::write(&path, json).expect("write config.json");
        path
    }

    // ── Project-scoped mastodon key (§ mastodon-scope) ────────────────────────

    #[test]
    fn test_mastodon_active_key_differs_across_projects() {
        use crate::mastodon_connection::active_instance_key;
        let key_p1 = active_instance_key("proj-1");
        let key_p2 = active_instance_key("proj-2");
        assert_ne!(key_p1, key_p2,
            "list_connected_platforms must check a project-scoped mastodon key to prevent cross-project bleed");
        assert!(key_p1.contains("proj-1"));
        assert!(key_p2.contains("proj-2"));
    }

    // §21.11.12 — mastodon direct connection (KEYRING_ACTIVE_INSTANCE present)
    #[test]
    fn test_mastodon_included_when_active_instance_present() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let platforms = list_connected_platforms_impl(&config_path, "r1", true, &|_| false);
        assert!(platforms.contains(&"mastodon".to_string()));
    }

    // §21.11.14 — mastodon not included when instance absent
    #[test]
    fn test_mastodon_excluded_when_no_active_instance() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let platforms = list_connected_platforms_impl(&config_path, "r1", false, &|_| false);
        assert!(!platforms.contains(&"mastodon".to_string()));
    }

    // §21.11.13 — scheduler account ID + provider credential → platform connected
    #[test]
    fn test_scheduler_platform_included_when_account_id_and_provider_cred_present() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "account_ids": { "bluesky": "myhandle" } }
        }"#);
        let platforms = list_connected_platforms_impl(
            &config_path, "r1", false,
            &|key| key == "zernio/r1",
        );
        assert!(platforms.contains(&"bluesky".to_string()));
    }

    // §21.11.14 — account ID present but no provider credential → not connected
    #[test]
    fn test_scheduler_platform_excluded_when_no_provider_cred() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "account_ids": { "bluesky": "myhandle" } }
        }"#);
        let platforms = list_connected_platforms_impl(&config_path, "r1", false, &|_| false);
        assert!(!platforms.contains(&"bluesky".to_string()));
    }

    // §21.11.14 — empty account ID not counted as connected
    #[test]
    fn test_empty_account_id_not_connected() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "account_ids": { "x": "" } }
        }"#);
        let platforms = list_connected_platforms_impl(
            &config_path, "r1", false,
            &|key| key == "zernio/r1",
        );
        assert!(!platforms.contains(&"x".to_string()));
    }

    // Multiple scheduler platforms all included when provider cred present
    #[test]
    fn test_multiple_scheduler_platforms_all_included() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "account_ids": { "x": "h1", "bluesky": "h2" } }
        }"#);
        let platforms = list_connected_platforms_impl(
            &config_path, "r1", true,
            &|key| key == "zernio/r1",
        );
        assert!(platforms.contains(&"x".to_string()));
        assert!(platforms.contains(&"bluesky".to_string()));
        assert!(platforms.contains(&"mastodon".to_string()));
    }

    // Returns empty when nothing is connected
    #[test]
    fn test_returns_empty_when_no_connections() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{"version":1}"#);
        let platforms = list_connected_platforms_impl(&config_path, "r1", false, &|_| false);
        assert!(platforms.is_empty());
    }

    // Missing config.json treated as no connections (not an error)
    #[test]
    fn test_absent_config_returns_empty() {
        let platforms = list_connected_platforms_impl(
            std::path::Path::new("/nonexistent/.postlane/config.json"),
            "r1", false, &|_| false,
        );
        assert!(platforms.is_empty());
    }

    // Result is sorted (deterministic order for frontend)
    #[test]
    fn test_result_is_sorted() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_path = write_config(dir.path(), r#"{
            "version": 1,
            "scheduler": { "account_ids": { "x": "h", "bluesky": "h", "linkedin": "h" } }
        }"#);
        let platforms = list_connected_platforms_impl(
            &config_path, "r1", false,
            &|key| key == "zernio/r1",
        );
        let mut sorted = platforms.clone();
        sorted.sort();
        assert_eq!(platforms, sorted, "result must be in sorted order");
    }
}
