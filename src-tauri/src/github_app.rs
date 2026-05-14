// SPDX-License-Identifier: BUSL-1.1

use serde::Deserialize;
use std::time::Duration;
use tauri_plugin_keyring::KeyringExt;

use crate::license::POSTLANE_API_BASE;

#[derive(Deserialize)]
struct InstallationStatusBody {
    installed: bool,
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
        status => Err(format!("check_github_app_installed: HTTP {}", status)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

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
