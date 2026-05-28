// SPDX-License-Identifier: BUSL-1.1

use crate::license::POSTLANE_API_BASE;
use crate::project_registry::{require_license_token, SESSION_EXPIRED_ERROR};
use crate::project_validation::validate_project_id;
use crate::providers::scheduling::build_client;
use tauri::Emitter;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_registry::SESSION_EXPIRED_ERROR;
    use crate::providers::scheduling::build_client;
    use crate::test_fixtures::{make_state as make_state_with_repos, make_repo as make_repo_entry};
    use httpmock::prelude::*;
    use std::fs;

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

    #[tokio::test]
    async fn test_delete_project_returns_session_expired_error_on_401() {
        let state = make_state_with_repos(vec![]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-abc");
            then.status(401);
        });

        let result = delete_project_with_client("proj-abc", &build_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_err(), "401 must return Err");
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
    }

    #[tokio::test]
    async fn test_delete_project_rejects_invalid_project_id() {
        let state = make_state_with_repos(vec![]);
        let result = delete_project_with_client("", &build_client(), "http://127.0.0.1:19993", "tok", &state).await;
        assert!(result.is_err(), "empty project_id must return Err");
    }

    #[tokio::test]
    async fn test_delete_project_returns_error_when_unregister_fails_after_204() {
        use std::os::unix::fs::PermissionsExt;

        let home = dirs::home_dir().expect("home dir");
        let repo_dir = home.join("postlane_test_del_proj_unreg_fail");
        write_project_config(&repo_dir, "proj-unreg-fail");

        // Place repos.json in a temp dir we will make read-only so the write fails
        let repos_dir = tempfile::TempDir::new().expect("temp dir");
        let repos_path = repos_dir.path().join("repos.json");
        let repo = make_repo_entry("r-unreg", repo_dir.to_str().unwrap());
        let config = crate::storage::ReposConfig { version: 1, workspaces: vec![], repos: vec![repo] };
        let state = crate::app_state::AppState::new_with_path(config, repos_path.clone());

        // Write repos.json so AppState initialises correctly, then seal the directory
        let repos = state.repos.lock().expect("lock");
        crate::storage::write_repos(&repos_path, &repos).expect("initial write");
        drop(repos);
        let ro_perms = std::fs::Permissions::from_mode(0o444);
        std::fs::set_permissions(repos_dir.path(), ro_perms).expect("set read-only");

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-unreg-fail");
            then.status(204);
        });

        let result = delete_project_with_client(
            "proj-unreg-fail",
            &build_client(),
            &server.base_url(),
            "tok",
            &state,
        )
        .await;

        // Restore permissions so TempDir can clean up
        let rw_perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(repos_dir.path(), rw_perms);
        let _ = fs::remove_dir_all(&repo_dir);

        assert!(result.is_err(), "failure to write repos.json after 204 must return Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("failed to remove repo") || msg.contains("Failed to write repos.json"),
            "error must mention repo removal failure, got: {}",
            msg
        );
    }
}
