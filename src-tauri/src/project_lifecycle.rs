// SPDX-License-Identifier: BUSL-1.1

//! Create, delete, and register project operations.
//!
//! These commands wrap the corresponding `project_api` functions, adding token
//! retrieval from the OS keyring and local side-effects (writing config.json,
//! deregistering repos, emitting events).

use crate::app_state::AppState;
use crate::license::POSTLANE_API_BASE;
use crate::project_api::{
    create_project_with_client, list_projects_with_client, update_project_org_login_with_client,
};
use crate::project_config_ops::{sha256_hex, write_project_id_to_config_impl};
use crate::project_registry::{require_license_token, ProjectSummary, SESSION_EXPIRED_ERROR};
use crate::project_validation::validate_project_id;
use crate::providers::scheduling::build_client;
use tauri::{Emitter, State};
use tauri_plugin_keyring::KeyringExt;

/// Calls `DELETE {base_url}/v1/projects/{project_id}`.
/// On 204: deregisters all repos whose config.json matches `project_id`.
/// On any other status (including 401, 403): returns `Err` without touching local state.
pub async fn delete_project_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    state: &crate::app_state::AppState,
) -> Result<(), String> {
    validate_project_id(project_id)?;
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let resp = client.delete(&url).bearer_auth(token).send().await
        .map_err(|e| format!("Network error: {}", e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(SESSION_EXPIRED_ERROR.to_string());
    }
    if status.as_u16() != 204 {
        return Err(format!("Backend returned {} — project not deleted", status));
    }
    let matching = crate::repo_project_filter::list_repos_for_project_impl(project_id, state)?;
    for repo in &matching {
        if let Err(e) = crate::repo_project_filter::unregister_repo_impl(&repo.id, state) {
            log::error!("[delete_project] failed to remove repo '{}': {}", repo.id, e);
            return Err(format!(
                "Project deleted remotely but failed to remove repo '{}' from local registry: {}",
                repo.id, e
            ));
        }
    }
    Ok(())
}

/// Calls `POST {base_url}/v1/projects/{project_id}/repos`, then writes `project_id` to config.
pub async fn register_repo_with_project_with_client(
    project_id: &str,
    repo_path: &str,
    description: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    repos: &crate::storage::ReposConfig,
) -> Result<String, String> {
    validate_project_id(project_id)?;
    let is_registered = repos.repos.iter().any(|r| r.path == repo_path);
    if !is_registered {
        return Err(format!("Path '{}' is not in the registered repos list", repo_path));
    }

    let path_hash = sha256_hex(repo_path);
    let url = format!("{}/v1/projects/{}/repos", base_url, project_id);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "repo_path_hash": path_hash, "description": description }))
        .send()
        .await
        .map_err(|e| format!("Backend error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Backend returned {}", resp.status()));
    }

    write_project_id_to_config_impl(repo_path, project_id, repos)
}

/// Tauri command: deletes a project on the backend and deregisters all local repos.
#[tauri::command]
pub async fn delete_project(
    project_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    delete_project_with_client(&project_id, &client, POSTLANE_API_BASE, &token, &state).await?;
    let _ = app.emit(crate::platform_constants::PROJECTS_CHANGED_EVENT, ());
    Ok(())
}

/// Tauri command: lists all projects for the signed-in user.
#[tauri::command]
pub async fn list_projects(app: tauri::AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    list_projects_with_client(&client, POSTLANE_API_BASE, &token).await
}

/// Tauri command: creates a new project on the backend.
#[tauri::command]
pub async fn create_project(
    name: String,
    workspace_type: String,
    provider_org_login: Option<String>,
    provider_group_path: Option<String>,
    app: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let (project_id, project_name, wt) =
        create_project_with_client(
            &name,
            &workspace_type,
            provider_org_login.as_deref(),
            provider_group_path.as_deref(),
            &client,
            POSTLANE_API_BASE,
            &token,
        )
            .await
            .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "project_id": project_id, "name": project_name, "workspace_type": wt }))
}

/// Sets `provider_org_login` on an existing project. Used by the v1.2 upgrade flow.
#[tauri::command]
pub async fn update_project_org_login(
    project_id: String,
    org_login: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    update_project_org_login_with_client(&project_id, &org_login, &client, POSTLANE_API_BASE, &token).await
}

/// Tauri command: registers a repo with a project on the backend and writes project_id to config.
#[tauri::command]
pub async fn register_repo_with_project(
    project_id: String,
    repo_path: String,
    description: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?
        .clone();
    let client = build_client();
    register_repo_with_project_with_client(
        &project_id, &repo_path, &description, &client, POSTLANE_API_BASE, &token, &repos,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_api::create_project_with_client;
    use crate::project_config_ops::read_project_id_from_path_impl;
    use crate::providers::scheduling::build_client;
    use crate::storage::{Repo, ReposConfig};
    use crate::test_fixtures::{make_state as make_state_with_repos, make_repo as make_repo_entry};
    use httpmock::prelude::*;
    use std::fs;

    fn make_repos(paths: &[&str]) -> ReposConfig {
        ReposConfig {
            version: 1,
            repos: paths
                .iter()
                .enumerate()
                .map(|(i, p)| Repo {
                    id: format!("r{}", i),
                    name: format!("Repo{}", i),
                    path: p.to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                })
                .collect(),
        }
    }

    // ── register_repo_with_project ───────────────────────────────────────────

    #[tokio::test]
    async fn test_registers_repo_and_writes_project_id() {
        let dir = std::env::temp_dir().join("postlane_test_register_repo_pr");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write");

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path_matches(regex::Regex::new(r"/v1/projects/.+/repos").unwrap());
            then.status(200).json_body(serde_json::json!({ "repo_id": "repo-uuid-123" }));
        });

        let repos = make_repos(&[dir.to_str().unwrap()]);
        let notice = register_repo_with_project_with_client(
            "proj-abc", dir.to_str().unwrap(), "The desktop app",
            &build_client(), &server.base_url(), "tok", &repos,
        ).await.expect("should succeed");

        let content = fs::read_to_string(config_dir.join("config.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-abc"));
        assert!(notice.contains("project_id"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_register_repo_rejects_path_not_in_repos() {
        let repos = make_repos(&["/other/path"]);
        let result = register_repo_with_project_with_client(
            "proj-abc", "/not/registered", "desc",
            &build_client(), "http://127.0.0.1:19995", "tok", &repos,
        ).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    // ── delete_project ───────────────────────────────────────────────────────

    fn write_project_config(repo_dir: &std::path::Path, project_id: &str) {
        let pl_dir = repo_dir.join(".postlane");
        fs::create_dir_all(&pl_dir).expect("create .postlane");
        fs::write(
            pl_dir.join("config.json"),
            format!(r#"{{"project_id":"{}"}}"#, project_id),
        ).expect("write config.json");
    }

    #[tokio::test]
    async fn test_delete_project_succeeds_on_204() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_del_proj_204");
        write_project_config(&dir, "proj-to-delete");

        let state = make_state_with_repos(vec![make_repo_entry("r1", dir.to_str().unwrap())]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-to-delete");
            then.status(204);
        });

        let result = delete_project_with_client("proj-to-delete", &build_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_ok(), "204 must return Ok: {:?}", result);

        let repos = state.repos.lock().expect("lock");
        assert!(!repos.repos.iter().any(|r| r.id == "r1"), "r1 must be deregistered after 204");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_delete_project_deregisters_local_repos_only_after_204() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_del_proj_403_noderegister");
        write_project_config(&dir, "proj-protected");

        let state = make_state_with_repos(vec![make_repo_entry("r1", dir.to_str().unwrap())]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-protected");
            then.status(403);
        });

        let result = delete_project_with_client("proj-protected", &build_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_err(), "403 must return Err");

        let repos = state.repos.lock().expect("lock");
        assert!(repos.repos.iter().any(|r| r.id == "r1"), "r1 must NOT be deregistered on 403");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_delete_project_returns_error_on_403() {
        let state = make_state_with_repos(vec![]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-abc");
            then.status(403);
        });

        let result = delete_project_with_client("proj-abc", &build_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_err(), "403 must return Err");
    }

    #[tokio::test]
    async fn test_delete_project_returns_error_on_network_failure() {
        let state = make_state_with_repos(vec![]);
        let result = delete_project_with_client("proj-abc", &build_client(), "http://127.0.0.1:19993", "tok", &state).await;
        assert!(result.is_err(), "network failure must return Err");
    }

    // ── integration: full wizard path ─────────────────────────────────────────

    /// Exercises the full create → register → read_config chain in one test.
    #[tokio::test]
    async fn test_full_wizard_path_create_register_read_config() {
        let dir = std::env::temp_dir().join("postlane_test_full_wizard_path");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
            .expect("write initial config");

        let server = MockServer::start();
        let project_id = "wizard-integration-uuid";

        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "project_id": project_id,
                "name": "Integration Workspace",
                "tier": "free",
                "workspace_type": "personal",
            }));
        });
        server.mock(|when, then| {
            when.method(POST).path(format!("/v1/projects/{}/repos", project_id));
            then.status(200).json_body(serde_json::json!({ "repo_id": "repo-int-001" }));
        });

        let client = build_client();
        let repos = make_repos(&[dir.to_str().unwrap()]);

        let (returned_id, returned_name, workspace_type) =
            create_project_with_client("Integration Workspace", "personal", None, None, &client, &server.base_url(), "tok")
                .await
                .expect("create_project should succeed");

        assert_eq!(returned_id, project_id);
        assert_eq!(returned_name, "Integration Workspace");
        assert_eq!(workspace_type, "personal");

        register_repo_with_project_with_client(
            &returned_id, dir.to_str().unwrap(), "Integration test repo",
            &client, &server.base_url(), "tok", &repos,
        )
        .await
        .expect("register_repo should succeed");

        let read_back = read_project_id_from_path_impl(dir.to_str().unwrap(), &repos)
            .expect("read_project_id should not error");

        assert_eq!(
            read_back.as_deref(),
            Some(project_id),
            "project_id written by register_repo must be readable by read_project_id_from_path"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
