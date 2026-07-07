// SPDX-License-Identifier: BUSL-1.1

//! Shared helpers for resolving config.json paths and reading project IDs.
//!
//! Multiple modules (scheduler_credentials, startup_sync, credential_migration,
//! connected_platforms, repo_queries) all need to locate a repo's config.json
//! and extract `project_id` from it. Centralising here prevents the same logic
//! appearing in four different files.

use crate::app_state::AppState;
use std::path::{Path, PathBuf};

/// Returns the `project_id` field from a config.json at `config_path`,
/// or `None` if the file is absent, unreadable, or lacks the field.
pub fn read_project_id_from_config(config_path: &Path) -> Option<String> {
    let json: serde_json::Value = crate::init::read_json_file(config_path).ok()?;
    json["project_id"].as_str().map(str::to_string)
}

/// Resolves the config.json path and credential key for `repo_id`.
///
/// For legacy per-repo entries (in `AppState.repos`): returns
///   `({repo_path}/.postlane/config.json, repo_id)`.
///
/// For workspace child repos (in a workspace's repos.json): returns
///   `({workspace}/config.json, project_id)`. Workspace credentials are stored
///   under project_id, not child repo_id.
pub fn resolve_config_and_cred_id(
    repo_id: &str,
    state: &AppState,
) -> Result<(PathBuf, String), String> {
    let repos = state.lock_repos()?;

    // 1. Try legacy per-repo array first.
    if let Some(repo) = repos.repos.iter().find(|r| r.id == repo_id) {
        let config = PathBuf::from(&repo.path).join(".postlane/config.json");
        // If this legacy repo's project_id matches a registered workspace, use the workspace
        // project_id as the credential key. Credentials saved via workspace settings are stored
        // under the workspace ID, so legacy repos associated with that workspace must match it.
        let cred_id = read_project_id_from_config(&config)
            .filter(|pid| repos.workspaces.iter().any(|w| w.active && &w.id == pid))
            .unwrap_or_else(|| repo_id.to_string());
        return Ok((config, cred_id));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    license_status: None,
    is_owner: None,
    status_updated_at: None,
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

    // ── resolve_config_and_cred_id ─────────────────────────────────────────────

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

    #[test]
    fn test_resolve_config_and_cred_id_legacy_repo_uses_workspace_project_id_when_matching() {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        let repo_path = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo_path.join(".postlane")).expect("create .postlane");
        let config_json = serde_json::json!({ "project_id": "ws-proj-abc" });
        std::fs::write(
            repo_path.join(".postlane/config.json"),
            serde_json::to_string(&config_json).unwrap(),
        ).expect("write config.json");

        let ws_path = tmp.path().join("workspace");
        let repos_config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-proj-abc".to_string(),
                name: "my-workspace".to_string(),
                workspace_path: ws_path.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![crate::storage::Repo {
                id: "legacy-repo-id".to_string(),
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
        let (config_path, cred_id) = resolve_config_and_cred_id("legacy-repo-id", &state)
            .expect("must resolve");
        assert_eq!(config_path, repo_path.join(".postlane/config.json"));
        assert_eq!(cred_id, "ws-proj-abc",
            "cred_id must be workspace project_id when legacy repo's project_id matches a workspace");
    }

    // ── read_project_id_from_config ────────────────────────────────────────────

    #[test]
    fn test_read_project_id_returns_id_when_present() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let config = tmp.path().join("config.json");
        fs::write(&config, r#"{"project_id":"proj-abc","version":1}"#)
            .expect("write config.json");
        let result = read_project_id_from_config(&config);
        assert_eq!(result, Some("proj-abc".to_string()));
    }

    #[test]
    fn test_read_project_id_returns_none_when_file_missing() {
        let result = read_project_id_from_config(
            std::path::Path::new("/nonexistent/path/config.json"),
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_project_id_returns_none_when_json_invalid() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let config = tmp.path().join("config.json");
        fs::write(&config, b"not valid json at all").expect("write config.json");
        let result = read_project_id_from_config(&config);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_project_id_returns_none_when_field_absent() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let config = tmp.path().join("config.json");
        fs::write(&config, r#"{"version":1,"schema_version":4}"#)
            .expect("write config.json");
        let result = read_project_id_from_config(&config);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_project_id_returns_none_when_field_is_not_a_string() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let config = tmp.path().join("config.json");
        fs::write(&config, r#"{"project_id":42}"#).expect("write config.json");
        let result = read_project_id_from_config(&config);
        assert_eq!(result, None, "numeric project_id must not be coerced to a string");
    }
}
