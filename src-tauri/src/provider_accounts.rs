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

/// A single connected provider account, as shown in the desktop switcher's
/// icon rail (checklist 24.4.10).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ProviderAccountSummary {
    pub id: String,
    pub provider: String,
    pub provider_account_id: Option<String>,
    pub label: Option<String>,
    pub is_primary: bool,
}

// ── list_provider_accounts ──────────────────────────────────────────────────

/// Response shape from `GET /v1/account/provider-accounts`.
#[derive(Deserialize)]
struct ProviderAccountsResponse {
    accounts: Vec<ProviderAccountSummary>,
}

/// Calls `GET {base_url}/v1/account/provider-accounts` with the license
/// token and returns every connected provider account for the current user.
pub async fn list_provider_accounts_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<ProviderAccountSummary>, String> {
    let url = format!("{}/v1/account/provider-accounts", base_url);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching provider accounts: {}", e))?;

    match resp.status().as_u16() {
        200 => {
            let body: ProviderAccountsResponse = resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse provider accounts response: {}", e))?;
            Ok(body.accounts)
        }
        401 => Err(SESSION_EXPIRED_ERROR.to_string()),
        _ => Err(format!("Unexpected status {} fetching provider accounts", resp.status())),
    }
}

#[tauri::command]
pub async fn list_provider_accounts(
    app: tauri::AppHandle,
) -> Result<Vec<ProviderAccountSummary>, String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let client = build_client();
    list_provider_accounts_with_client(&client, POSTLANE_API_BASE, &token).await
}

// ── remove_provider_account ─────────────────────────────────────────────────

/// Calls `DELETE {base_url}/v1/account/provider-accounts/{id}` with the
/// license token. Returns a distinguishable error for the primary-account
/// case (409) so the UI can disable the "Remove" action rather than let the
/// user hit a rejected request.
pub async fn remove_provider_account_with_client(
    id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/account/provider-accounts/{}", base_url, id);
    let resp = client
        .delete(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Network error removing provider account: {}", e))?;

    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err(SESSION_EXPIRED_ERROR.to_string()),
        404 => Err("account_not_found".to_string()),
        409 => Err("cannot_remove_primary".to_string()),
        _ => Err(format!("Unexpected status {} removing provider account", resp.status())),
    }
}

#[tauri::command]
pub async fn remove_provider_account(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let client = build_client();
    remove_provider_account_with_client(&id, &client, POSTLANE_API_BASE, &token).await
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;
    use httpmock::Method::{DELETE, GET};

    fn build_test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("test client")
    }

    // ── list_provider_accounts ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_provider_accounts_returns_vec_on_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/provider-accounts");
            then.status(200).json_body(serde_json::json!({
                "accounts": [
                    { "id": "row-1", "provider": "github", "provider_account_id": "111", "label": "alice", "is_primary": true },
                    { "id": "row-2", "provider": "github", "provider_account_id": "222", "label": "alice-work", "is_primary": false }
                ]
            }));
        });

        let result = list_provider_accounts_with_client(&build_test_client(), &server.base_url(), "token").await;
        let accounts = result.expect("should succeed");
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].id, "row-1");
        assert!(accounts[0].is_primary);
        assert!(!accounts[1].is_primary);
    }

    #[tokio::test]
    async fn test_list_provider_accounts_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/provider-accounts");
            then.status(401);
        });

        let result = list_provider_accounts_with_client(&build_test_client(), &server.base_url(), "expired").await;
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
    }

    #[tokio::test]
    async fn test_list_provider_accounts_returns_empty_when_none_connected() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/provider-accounts");
            then.status(200).json_body(serde_json::json!({ "accounts": [] }));
        });

        let result = list_provider_accounts_with_client(&build_test_client(), &server.base_url(), "token").await;
        assert!(result.expect("should succeed").is_empty());
    }

    #[tokio::test]
    async fn test_list_provider_accounts_returns_error_on_network_failure() {
        let result = list_provider_accounts_with_client(&build_test_client(), "http://127.0.0.1:19995", "tok").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_provider_accounts_returns_error_on_unexpected_status() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/account/provider-accounts");
            then.status(503);
        });
        let result = list_provider_accounts_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert!(result.is_err(), "unexpected 5xx must return Err");
        let msg = result.unwrap_err();
        assert!(msg.contains("503"), "error must include the status code, got: {}", msg);
    }

    // ── remove_provider_account ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_remove_provider_account_returns_ok_on_200() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE).path("/v1/account/provider-accounts/row-2");
            then.status(200).json_body(serde_json::json!({ "removed": true }));
        });

        let result = remove_provider_account_with_client("row-2", &build_test_client(), &server.base_url(), "token").await;
        assert!(result.is_ok());
        mock.assert();
    }

    #[tokio::test]
    async fn test_remove_provider_account_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(DELETE).path("/v1/account/provider-accounts/row-2");
            then.status(401);
        });

        let result = remove_provider_account_with_client("row-2", &build_test_client(), &server.base_url(), "expired").await;
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR);
    }

    #[tokio::test]
    async fn test_remove_provider_account_returns_account_not_found_on_404() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(DELETE).path("/v1/account/provider-accounts/not-mine");
            then.status(404).json_body(serde_json::json!({ "error": "not_found" }));
        });

        let result = remove_provider_account_with_client("not-mine", &build_test_client(), &server.base_url(), "token").await;
        assert_eq!(result.unwrap_err(), "account_not_found");
    }

    #[tokio::test]
    async fn test_remove_provider_account_returns_cannot_remove_primary_on_409() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(DELETE).path("/v1/account/provider-accounts/row-1");
            then.status(409).json_body(serde_json::json!({ "error": "cannot_remove_primary" }));
        });

        let result = remove_provider_account_with_client("row-1", &build_test_client(), &server.base_url(), "token").await;
        assert_eq!(result.unwrap_err(), "cannot_remove_primary");
    }

    #[tokio::test]
    async fn test_remove_provider_account_returns_error_on_network_failure() {
        let result = remove_provider_account_with_client("row-1", &build_test_client(), "http://127.0.0.1:19996", "tok").await;
        assert!(result.is_err());
    }
}
