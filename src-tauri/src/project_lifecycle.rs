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
use crate::project_config_ops::write_project_id_to_config_impl;
use crate::repo_init_config::sha256_hex;
use crate::project_registry::{require_license_token, ProjectSummary};
use crate::project_validation::validate_project_id;
use crate::providers::scheduling::build_client;
use tauri::{Manager, State};
use tauri_plugin_keyring::KeyringExt;

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
    let result = serde_json::json!({ "project_id": project_id, "name": project_name, "workspace_type": wt });
    match list_projects_with_client(&client, POSTLANE_API_BASE, &token).await {
        Ok(list) => {
            let state: tauri::State<AppState> = app.state();
            *state.projects_cache.write().await = list;
        }
        Err(e) => log::warn!("[create_project] failed to refresh projects cache: {}", e),
    }
    Ok(result)
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
    let repos = state.lock_repos()?.clone();
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
    use httpmock::prelude::*;
    use std::fs;

    fn make_repos(paths: &[&str]) -> ReposConfig {
        ReposConfig {
            version: 1, workspaces: vec![], repos: paths
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
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write");

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path_matches(regex::Regex::new(r"/v1/projects/.+/repos").unwrap());
            then.status(200).json_body(serde_json::json!({ "repo_id": "repo-uuid-123" }));
        });

        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let notice = register_repo_with_project_with_client(
            "proj-abc", dir.path().to_str().unwrap(), "The desktop app",
            &build_client(), &server.base_url(), "tok", &repos,
        ).await.expect("should succeed");

        let content = fs::read_to_string(config_dir.join("config.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-abc"));
        assert!(notice.contains("project_id"));
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

    #[tokio::test]
    async fn test_register_repo_returns_err_on_backend_error() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path_matches(regex::Regex::new(r"/v1/projects/.+/repos").unwrap());
            then.status(500);
        });

        let result = register_repo_with_project_with_client(
            "proj-abc", dir.path().to_str().unwrap(), "desc",
            &build_client(), &server.base_url(), "tok", &repos,
        ).await;
        assert!(result.is_err(), "500 must return Err");
        assert!(result.unwrap_err().contains("Backend returned"), "error must mention Backend returned");
    }

    #[tokio::test]
    async fn test_register_repo_returns_err_on_401() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path_matches(regex::Regex::new(r"/v1/projects/.+/repos").unwrap());
            then.status(401);
        });

        let result = register_repo_with_project_with_client(
            "proj-abc", dir.path().to_str().unwrap(), "desc",
            &build_client(), &server.base_url(), "tok", &repos,
        ).await;
        assert!(result.is_err(), "401 must return Err");
        assert!(result.unwrap_err().contains("Backend returned"), "error must mention Backend returned");
    }

    #[tokio::test]
    async fn test_register_repo_rejects_invalid_project_id() {
        let repos = make_repos(&["/some/path"]);
        let result = register_repo_with_project_with_client(
            "", "/some/path", "desc",
            &build_client(), "http://127.0.0.1:19993", "tok", &repos,
        ).await;
        assert!(result.is_err(), "empty project_id must return Err");
    }

    // register_repo line 45 — network failure before any response → "Backend error"
    #[tokio::test]
    async fn test_register_repo_returns_err_on_network_failure() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let result = register_repo_with_project_with_client(
            "proj-abc",
            dir.path().to_str().unwrap(),
            "desc",
            &build_client(),
            "http://127.0.0.1:1",  // port 1 = nothing listening → connection refused
            "tok",
            &repos,
        ).await;
        assert!(result.is_err(), "connection refused must return Err");
        assert!(
            result.unwrap_err().contains("Backend error"),
            "network failure message must say 'Backend error'"
        );
    }

    // ── integration: full wizard path ─────────────────────────────────────────

    /// Exercises the full create → register → read_config chain in one test.
    #[tokio::test]
    async fn test_full_wizard_path_create_register_read_config() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
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
        let repos = make_repos(&[dir.path().to_str().unwrap()]);

        let (returned_id, returned_name, workspace_type) =
            create_project_with_client("Integration Workspace", "personal", None, None, &client, &server.base_url(), "tok")
                .await
                .expect("create_project should succeed");

        assert_eq!(returned_id, project_id);
        assert_eq!(returned_name, "Integration Workspace");
        assert_eq!(workspace_type, "personal");

        register_repo_with_project_with_client(
            &returned_id, dir.path().to_str().unwrap(), "Integration test repo",
            &client, &server.base_url(), "tok", &repos,
        )
        .await
        .expect("register_repo should succeed");

        let read_back = read_project_id_from_path_impl(dir.path().to_str().unwrap(), &repos)
            .expect("read_project_id should not error");

        assert_eq!(
            read_back.as_deref(),
            Some(project_id),
            "project_id written by register_repo must be readable by read_project_id_from_path"
        );
    }
}
