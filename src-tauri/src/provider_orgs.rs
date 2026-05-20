// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri_plugin_keyring::KeyringExt;

use crate::license::POSTLANE_API_BASE;
use crate::project_registry::SESSION_EXPIRED_ERROR;

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client")
}

/// Single org entry returned by the provider API and surfaced to the frontend.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct OrgSummary {
    pub login: String,
    pub display_name: String,
    pub avatar_url: String,
    pub is_personal: bool,
    /// True when a Postlane project already exists for this org.
    pub has_project: bool,
    /// The project id of the existing project, if has_project is true.
    pub project_id: Option<String>,
}

// ── list_provider_orgs ─────────────────────────────────────────────────────

/// Response shape from `GET /v1/provider/orgs`.
#[derive(Deserialize)]
struct ProviderOrgsResponse {
    orgs: Vec<OrgSummary>,
}

/// Calls `GET {base_url}/v1/provider/orgs?provider={provider}` with the
/// license token and returns the list of provider orgs.
pub async fn list_provider_orgs_with_client(
    provider: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<OrgSummary>, String> {
    let url = format!("{}/v1/provider/orgs?provider={}", base_url, provider);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching provider orgs: {}", e))?;

    match resp.status().as_u16() {
        200 => {
            let body: ProviderOrgsResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse provider orgs response: {}", e))?;
            Ok(body.orgs)
        }
        401 => Err(SESSION_EXPIRED_ERROR.to_string()),
        403 => Err("scope_not_granted".to_string()),
        _ => Err(format!("Unexpected status {} fetching provider orgs", resp.status())),
    }
}

#[tauri::command]
pub async fn list_provider_orgs(
    provider: String,
    app: tauri::AppHandle,
) -> Result<Vec<OrgSummary>, String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let client = build_client();
    list_provider_orgs_with_client(&provider, &client, POSTLANE_API_BASE, &token).await
}

// ── list_linked_providers ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct LinkedProvidersResponse {
    providers: Vec<String>,
}

/// Calls `GET {base_url}/v1/account/providers` and returns the list of SSO
/// providers linked to the current user's account.
pub async fn list_linked_providers_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<String>, String> {
    let url = format!("{}/v1/account/providers", base_url);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching linked providers: {}", e))?;

    match resp.status().as_u16() {
        200 => {
            let body: LinkedProvidersResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse linked providers response: {}", e))?;
            Ok(body.providers)
        }
        401 => Err(SESSION_EXPIRED_ERROR.to_string()),
        _ => Err(format!("Unexpected status {} fetching linked providers", resp.status())),
    }
}

#[tauri::command]
pub async fn list_linked_providers(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let client = build_client();
    list_linked_providers_with_client(&client, POSTLANE_API_BASE, &token).await
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;
    use httpmock::Method::{GET};

    fn build_test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("test client")
    }

    // ── list_provider_orgs ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_provider_orgs_returns_vec_on_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/provider/orgs")
                .query_param("provider", "github");
            then.status(200).json_body(serde_json::json!({
                "orgs": [
                    {
                        "login": "hugoelliott",
                        "display_name": "Hugo Elliott",
                        "avatar_url": "https://avatars.githubusercontent.com/u/1",
                        "is_personal": true,
                        "has_project": false,
                        "project_id": null
                    },
                    {
                        "login": "postlane",
                        "display_name": "Postlane",
                        "avatar_url": "https://avatars.githubusercontent.com/orgs/postlane",
                        "is_personal": false,
                        "has_project": true,
                        "project_id": "proj-abc-123"
                    }
                ]
            }));
        });

        let result = list_provider_orgs_with_client(
            "github", &build_test_client(), &server.base_url(), "token",
        ).await;
        let orgs = result.expect("should succeed");
        assert_eq!(orgs.len(), 2);
        assert_eq!(orgs[0].login, "hugoelliott");
        assert!(orgs[0].is_personal);
        assert!(!orgs[0].has_project);
        assert!(orgs[0].project_id.is_none());
        assert_eq!(orgs[1].login, "postlane");
        assert!(orgs[1].has_project);
        assert_eq!(orgs[1].project_id.as_deref(), Some("proj-abc-123"));
    }

    #[tokio::test]
    async fn test_list_provider_orgs_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/provider/orgs");
            then.status(401);
        });

        let result = list_provider_orgs_with_client(
            "github", &build_test_client(), &server.base_url(), "expired",
        ).await;
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
    }

    #[tokio::test]
    async fn test_list_provider_orgs_returns_scope_not_granted_on_403() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/provider/orgs");
            then.status(403).json_body(serde_json::json!({ "error": "scope_not_granted" }));
        });

        let result = list_provider_orgs_with_client(
            "github", &build_test_client(), &server.base_url(), "token",
        ).await;
        assert_eq!(result.unwrap_err(), "scope_not_granted");
    }

    #[tokio::test]
    async fn test_list_provider_orgs_passes_provider_query_param() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/v1/provider/orgs")
                .query_param("provider", "gitlab");
            then.status(200).json_body(serde_json::json!({ "orgs": [] }));
        });

        list_provider_orgs_with_client(
            "gitlab", &build_test_client(), &server.base_url(), "tok",
        ).await.expect("should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn test_list_provider_orgs_returns_error_on_network_failure() {
        let result = list_provider_orgs_with_client(
            "github", &build_test_client(), "http://127.0.0.1:19993", "tok",
        ).await;
        assert!(result.is_err());
    }

    // ── list_linked_providers ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_linked_providers_returns_all_linked_providers() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/providers");
            then.status(200).json_body(serde_json::json!({
                "providers": ["github", "gitlab"]
            }));
        });

        let result = list_linked_providers_with_client(
            &build_test_client(), &server.base_url(), "token",
        ).await;
        let providers = result.expect("should succeed");
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"github".to_string()));
        assert!(providers.contains(&"gitlab".to_string()));
    }

    #[tokio::test]
    async fn test_list_linked_providers_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/providers");
            then.status(401);
        });

        let result = list_linked_providers_with_client(
            &build_test_client(), &server.base_url(), "expired",
        ).await;
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
    }

    #[tokio::test]
    async fn test_list_linked_providers_returns_empty_when_none_linked() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/providers");
            then.status(200).json_body(serde_json::json!({ "providers": [] }));
        });

        let result = list_linked_providers_with_client(
            &build_test_client(), &server.base_url(), "token",
        ).await;
        assert!(result.expect("should succeed").is_empty());
    }
}
