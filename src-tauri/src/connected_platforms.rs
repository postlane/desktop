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

/// Resolves the config.json path and credential key for a repo.
///
/// For legacy per-repo entries (in `AppState.repos`): returns
///   `({repo_path}/.postlane/config.json, repo_id)`.
///
/// For workspace child repos (in a workspace's repos.json): returns
///   `({workspace}/config.json, project_id)`. Workspace credentials are stored
///   under project_id, not child repo_id.
pub(crate) fn resolve_config_and_cred_id(
    repo_id: &str,
    state: &AppState,
) -> Result<(PathBuf, String), String> {
    let repos = state.lock_repos()?;

    // 1. Try legacy per-repo array first.
    if let Some(repo) = repos.repos.iter().find(|r| r.id == repo_id) {
        let config = PathBuf::from(&repo.path).join(".postlane/config.json");
        return Ok((config, repo_id.to_string()));
    }

    // 2. Search workspace children.
    for ws in &repos.workspaces {
        if !ws.active {
            continue;
        }
        let ws_path = PathBuf::from(&ws.workspace_path);
        let ws_repos_path = ws_path.join("repos.json");
        let Ok(ws_repos) = crate::workspace_repos::read_workspace_repos(&ws_repos_path) else {
            continue;
        };
        if ws_repos.repos.iter().any(|r| r.id == repo_id) {
            let config = ws_path.join("config.json");
            return Ok((config, ws.id.clone()));
        }
    }

    Err(format!("Repo '{}' not found in any registered repo or workspace", repo_id))
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
    let (config_path, cred_id) = resolve_config_and_cred_id(&repo_id, &state)?;
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
        &cred_id,
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

    // ── Workspace child repo resolution ────────────────────────────────────────
    // Workspace child repos are NOT in AppState.repos (the legacy array). Their
    // config comes from {workspace}/config.json and their credentials are keyed
    // by project_id, not by child repo_id.

    fn make_app_state_with_workspace(
        workspace_path: &std::path::Path,
        project_id: &str,
        child_repo_id: &str,
        child_repo_name: &str,
    ) -> crate::app_state::AppState {
        use crate::workspace_entry::WorkspaceEntry;
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig};

        // Write workspace config.json
        let ws_config = serde_json::json!({ "project_id": project_id, "schema_version": 4 });
        std::fs::write(
            workspace_path.join("config.json"),
            serde_json::to_string_pretty(&ws_config).unwrap(),
        ).expect("write workspace config.json");

        // Write workspace repos.json
        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: child_repo_id.to_string(),
                name: child_repo_name.to_string(),
                path: workspace_path.join(child_repo_name).to_str().unwrap().to_string(),
                posts_dir: child_repo_name.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        crate::workspace_repos::write_workspace_repos(
            &workspace_path.join("repos.json"),
            &ws_repos,
        ).expect("write workspace repos.json");

        let repos_config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: project_id.to_string(),
                name: "test-workspace".to_string(),
                workspace_path: workspace_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };

        let tmp_repos = tempfile::NamedTempFile::new().expect("temp file");
        crate::app_state::AppState::new_with_path(repos_config, tmp_repos.path().to_path_buf())
    }

    /// Workspace child repos must resolve to workspace config + project_id as
    /// credential key — not to a per-repo .postlane path that doesn't exist.
    #[test]
    fn test_resolve_config_and_cred_id_finds_workspace_child() {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        let state = make_app_state_with_workspace(
            tmp.path(), "proj-ws-123", "child-repo-uuid", "myrepo",
        );
        let (config_path, cred_id) = resolve_config_and_cred_id("child-repo-uuid", &state)
            .expect("must resolve workspace child repo");
        assert_eq!(
            config_path,
            tmp.path().join("config.json"),
            "config path must be workspace config.json, not per-repo .postlane/config.json",
        );
        assert_eq!(
            cred_id, "proj-ws-123",
            "credential id must be project_id for workspace child repos",
        );
    }

    /// Legacy per-repo entries still resolve to per-repo config + repo_id.
    #[test]
    fn test_resolve_config_and_cred_id_legacy_repo_unchanged() {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        let repo_path = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo_path.join(".postlane")).expect("create .postlane");
        let repos_config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![],
            repos: vec![crate::storage::Repo {
                id: "legacy-id".to_string(),
                name: "my-repo".to_string(),
                path: repo_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        let state = crate::app_state::AppState::new_with_path(
            repos_config,
            tmp.path().join("repos.json"),
        );
        let (config_path, cred_id) = resolve_config_and_cred_id("legacy-id", &state)
            .expect("must resolve legacy repo");
        assert_eq!(config_path, repo_path.join(".postlane/config.json"));
        assert_eq!(cred_id, "legacy-id", "credential id must be repo_id for legacy repos");
    }

    /// list_connected_platforms_impl with workspace project_id as cred key returns
    /// platforms when scheduler cred is stored under project_id.
    #[test]
    fn test_workspace_repo_platforms_resolved_with_project_id_cred() {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        let ws_config = serde_json::json!({
            "project_id": "proj-ws-456",
            "schema_version": 4,
            "scheduler": { "account_ids": { "bluesky": "my-handle" } }
        });
        let config_path = tmp.path().join("config.json");
        std::fs::write(&config_path, serde_json::to_string_pretty(&ws_config).unwrap())
            .expect("write config");

        // Credential stored under project_id, not child repo_id
        let platforms = list_connected_platforms_impl(
            &config_path,
            "proj-ws-456",   // project_id used as cred_id
            false,
            &|key| key == "upload_post/proj-ws-456",
        );
        assert!(
            platforms.contains(&"bluesky".to_string()),
            "bluesky must appear when cred is keyed by project_id: got {:?}",
            platforms,
        );
    }
}
