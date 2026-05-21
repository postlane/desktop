// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_plugin_keyring::KeyringExt;

use crate::license::POSTLANE_API_BASE;
use crate::security::api_error::format_api_error;

/// A repository returned by the GitHub App installation.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GitHubAppRepo {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub private: bool,
    pub html_url: String,
}

#[derive(Deserialize)]
struct AppReposBody {
    repos: Vec<GitHubAppRepo>,
}

#[derive(Deserialize)]
struct InstallationStatusBody {
    installed: bool,
}

/// Backfills `provider_org_login` on an existing project via the Postlane API.
/// Only updates the field when it is currently null; a 200 with `updated: false`
/// means it was already set, which is also fine.
pub async fn backfill_project_org_login_impl(
    api_base: &str,
    project_id: &str,
    org_login: &str,
    token: &str,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;

    let url = format!("{}/v1/projects", api_base);
    let body = serde_json::json!({ "project_id": project_id, "provider_org_login": org_login });
    let resp = client
        .patch(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("backfill_project_org_login request failed: {}", e))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format_api_error("backfill_project_org_login", resp.status().as_u16(), ""))
    }
}

#[tauri::command]
pub async fn backfill_project_org_login(
    project_id: String,
    org_login: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token".to_string())?;

    backfill_project_org_login_impl(POSTLANE_API_BASE, &project_id, &org_login, &token).await
}

/// Checks whether the GitHub App is installed for the given project by querying
/// the Postlane API. Returns `true` if installed, `false` if not, or `Err` on
/// network or auth failure.
pub async fn check_github_app_installed_impl(
    api_base: &str,
    project_id: &str,
    token: &str,
) -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;

    let url = format!("{}/v1/github/installation?project_id={}", api_base, project_id);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("check_github_app_installed request failed: {}", e))?;

    match resp.status().as_u16() {
        200 => {
            let body: InstallationStatusBody = resp
                .json()
                .await
                .map_err(|e| format!("check_github_app_installed parse failed: {}", e))?;
            Ok(body.installed)
        }
        401 => Err("session_expired".to_string()),
        403 => Err("forbidden".to_string()),
        status => Err(format_api_error("check_github_app_installed", status, "")),
    }
}

#[tauri::command]
pub async fn check_github_app_installed(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    check_github_app_installed_impl(POSTLANE_API_BASE, &project_id, &token).await
}

/// Disconnects the GitHub App from a project by deleting its installation record.
/// Returns `Ok(())` on success or when no installation exists (idempotent).
pub async fn disconnect_github_app_impl(
    api_base: &str,
    project_id: &str,
    token: &str,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;

    let url = format!("{}/v1/github/installation?project_id={}", api_base, project_id);
    let resp = client
        .delete(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("disconnect_github_app request failed: {}", e))?;

    match resp.status().as_u16() {
        200 | 204 | 404 => Ok(()),
        401 => Err("session_expired".to_string()),
        403 => Err("forbidden".to_string()),
        status => Err(format_api_error("disconnect_github_app", status, "")),
    }
}

#[tauri::command]
pub async fn disconnect_github_app(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    disconnect_github_app_impl(POSTLANE_API_BASE, &project_id, &token).await
}

/// Fetches the list of repos accessible via the GitHub App installation for the
/// given project, by calling the Postlane API. Returns an empty vec (not an error)
/// when no installation exists for the project.
pub async fn list_github_app_repos_impl(
    api_base: &str,
    project_id: &str,
    token: &str,
) -> Result<Vec<GitHubAppRepo>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;

    let url = format!("{}/v1/github/app-repos?project_id={}", api_base, project_id);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("list_github_app_repos request failed: {}", e))?;

    match resp.status().as_u16() {
        200 => {
            let body: AppReposBody = resp
                .json()
                .await
                .map_err(|e| format!("list_github_app_repos parse failed: {}", e))?;
            Ok(body.repos)
        }
        401 => Err("session_expired".to_string()),
        403 => Err("forbidden".to_string()),
        status => Err(format_api_error("list_github_app_repos", status, "")),
    }
}

#[tauri::command]
pub async fn list_github_app_repos(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<Vec<GitHubAppRepo>, String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    list_github_app_repos_impl(POSTLANE_API_BASE, &project_id, &token).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    // ── list_github_app_repos_impl ────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_github_app_repos_returns_repos_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/github/app-repos")
                .query_param("project_id", "proj-1")
                .header("Authorization", "Bearer tok");
            then.status(200).json_body(serde_json::json!({
                "repos": [
                    { "id": 1, "name": "my-repo", "full_name": "org/my-repo", "private": false, "html_url": "https://github.com/org/my-repo" }
                ]
            }));
        });
        let result = list_github_app_repos_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let repos = result.unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "my-repo");
        assert_eq!(repos[0].full_name, "org/my-repo");
        assert_eq!(repos[0].html_url, "https://github.com/org/my-repo");
        assert!(!repos[0].private);
    }

    #[tokio::test]
    async fn test_list_github_app_repos_returns_empty_vec() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/app-repos");
            then.status(200).json_body(serde_json::json!({ "repos": [] }));
        });
        let result = list_github_app_repos_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_github_app_repos_returns_err_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/app-repos");
            then.status(401);
        });
        let result = list_github_app_repos_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Err("session_expired".to_string()));
    }

    #[tokio::test]
    async fn test_list_github_app_repos_returns_err_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/app-repos");
            then.status(403);
        });
        let result = list_github_app_repos_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Err("forbidden".to_string()));
    }

    #[tokio::test]
    async fn test_list_github_app_repos_returns_err_on_503() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/app-repos");
            then.status(503);
        });
        let result = list_github_app_repos_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("503"));
    }

    // ── disconnect_github_app_impl ────────────────────────────────────────────

    #[tokio::test]
    async fn test_disconnect_github_app_returns_ok_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/v1/github/installation")
                .query_param("project_id", "proj-1")
                .header("Authorization", "Bearer tok");
            then.status(200);
        });
        let result = disconnect_github_app_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[tokio::test]
    async fn test_disconnect_github_app_returns_ok_on_404() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/github/installation");
            then.status(404);
        });
        let result = disconnect_github_app_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_ok(), "expected Ok on 404 (idempotent), got {:?}", result);
    }

    #[tokio::test]
    async fn test_disconnect_github_app_returns_err_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/github/installation");
            then.status(401);
        });
        let result = disconnect_github_app_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Err("session_expired".to_string()));
    }

    #[tokio::test]
    async fn test_disconnect_github_app_returns_err_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/github/installation");
            then.status(403);
        });
        let result = disconnect_github_app_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Err("forbidden".to_string()));
    }

    // ── existing tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_backfill_returns_ok_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/projects")
                .header("Authorization", "Bearer tok")
                .json_body(serde_json::json!({
                    "project_id": "proj-1",
                    "provider_org_login": "postlane"
                }));
            then.status(200).json_body(serde_json::json!({ "updated": true }));
        });

        let result = backfill_project_org_login_impl(&server.base_url(), "proj-1", "postlane", "tok").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_backfill_returns_ok_when_already_set() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({ "updated": false }));
        });

        let result = backfill_project_org_login_impl(&server.base_url(), "proj-1", "postlane", "tok").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_backfill_returns_err_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects");
            then.status(403);
        });

        let result = backfill_project_org_login_impl(&server.base_url(), "proj-1", "postlane", "tok").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_returns_true_when_installed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/github/installation")
                .query_param("project_id", "proj-1")
                .header("Authorization", "Bearer tok");
            then.status(200).json_body(serde_json::json!({ "installed": true }));
        });

        let result = check_github_app_installed_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Ok(true));
    }

    #[tokio::test]
    async fn test_returns_false_when_not_installed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/github/installation")
                .query_param("project_id", "proj-1");
            then.status(200).json_body(serde_json::json!({ "installed": false }));
        });

        let result = check_github_app_installed_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Ok(false));
    }

    #[tokio::test]
    async fn test_returns_err_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/installation");
            then.status(401);
        });

        let result = check_github_app_installed_impl(&server.base_url(), "proj-1", "tok").await;
        assert_eq!(result, Err("session_expired".to_string()));
    }

    #[tokio::test]
    async fn test_returns_err_on_503() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/github/installation");
            then.status(503);
        });

        let result = check_github_app_installed_impl(&server.base_url(), "proj-1", "tok").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("503"));
    }
}
