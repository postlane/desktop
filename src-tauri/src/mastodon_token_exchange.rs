// SPDX-License-Identifier: BUSL-1.1

//! Mastodon OAuth token exchange and account handle verification.

use crate::app_state::AppState;
use crate::mastodon_connection::{access_token_key, active_instance_key, active_username_key, KEYRING_SERVICE};
use crate::providers::scheduling::build_client;
use crate::security::api_error::format_api_error;
use crate::security::instance_url::validate_instance_hostname;
use tauri::{Emitter, State};
use tauri_plugin_keyring::KeyringExt;

/// Exchanges an OAuth authorization code for an access token.
///
/// Reads client credentials from the OS keyring, exchanges the code with the
/// Mastodon instance, stores the resulting access token scoped to the project, and returns the account handle.
#[tauri::command]
pub async fn exchange_mastodon_code(
    instance: String,
    code: String,
    project_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    validate_instance_hostname(&instance)?;

    let client_id = app.keyring()
        .get_password(KEYRING_SERVICE, &format!("mastodon_client_id/{}", instance))
        .map_err(|e| format!("Keyring read error: {}", e))?
        .ok_or_else(|| "No client_id found — run Connect first".to_string())?;

    let client_secret = app.keyring()
        .get_password(KEYRING_SERVICE, &format!("mastodon_client_secret/{}", instance))
        .map_err(|e| format!("Keyring read error: {}", e))?
        .ok_or_else(|| "No client_secret found — run Connect first".to_string())?;

    let client = build_client();
    let base_url = format!("https://{}", instance);
    let access_token =
        fetch_access_token(&client, &base_url, &client_id, &client_secret, &code).await?;

    app.keyring()
        .set_password(KEYRING_SERVICE, &access_token_key(&project_id, &instance), &access_token)
        .map_err(|e| format!("Failed to store access token: {}", e))?;

    app.keyring()
        .set_password(KEYRING_SERVICE, &active_instance_key(&project_id), &instance)
        .map_err(|e| format!("Failed to store active instance: {}", e))?;

    let acct = fetch_acct(&client, &base_url, &access_token).await?;

    app.keyring()
        .set_password(KEYRING_SERVICE, &active_username_key(&project_id), &acct)
        .map_err(|e| format!("Failed to store active username: {}", e))?;

    sync_mastodon_connected_platforms(&project_id, &app, &state);

    let _ = app.emit("platform-connected", ());
    Ok(acct)
}

fn sync_mastodon_connected_platforms(project_id: &str, app: &tauri::AppHandle, state: &AppState) {
    let repos = match state.lock_repos() {
        Ok(r) => r,
        Err(e) => { log::warn!("sync_mastodon_connected_platforms: {}", e); return; }
    };
    for repo in &repos.repos {
        let config_path = std::path::PathBuf::from(&repo.path).join(".postlane/config.json");
        if crate::config_paths::read_project_id_from_config(&config_path).as_deref() != Some(project_id) {
            continue;
        }
        let _ = crate::project_config_ops::sync_connected_platforms_to_config_impl(
            &config_path, &repo.id, true,
            &|key| app.keyring().get_password(KEYRING_SERVICE, key).unwrap_or(None).is_some(),
        );
    }
}

/// Exchanges an authorization code for an access token via `POST {base_url}/oauth/token`.
pub(crate) async fn fetch_access_token(
    client: &reqwest::Client,
    base_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<String, String> {
    let url = format!("{}/oauth/token", base_url);

    let body = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret,
        "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
        "grant_type": "authorization_code",
        "code": code
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error exchanging code: {}", e))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(format_api_error("Token exchange failed", status, &text));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    json["access_token"]
        .as_str()
        .ok_or_else(|| "Missing access_token in token response".to_string())
        .map(String::from)
}

/// Fetches the account handle via `GET {base_url}/api/v1/accounts/verify_credentials`.
pub(crate) async fn fetch_acct(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<String, String> {
    let url = format!("{}/api/v1/accounts/verify_credentials", base_url);

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching account: {}", e))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(format_api_error("verify_credentials failed", status, &text));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse account response: {}", e))?;

    json["acct"]
        .as_str()
        .ok_or_else(|| "Missing acct in account response".to_string())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mastodon_connection::{access_token_key, active_instance_key, active_username_key};
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;

    // ── Key format (§ mastodon-scope) ─────────────────────────────────────────

    #[test]
    fn test_exchange_writes_project_scoped_access_token_key() {
        let key = access_token_key("proj-1", "mastodon.social");
        assert_eq!(key, "mastodon/proj-1/mastodon.social");
        assert_ne!(key, access_token_key("proj-2", "mastodon.social"),
            "exchange must write to a project-scoped key, not a global one");
    }

    #[test]
    fn test_exchange_writes_project_scoped_active_instance_key() {
        let key = active_instance_key("proj-1");
        assert_eq!(key, "mastodon_active_instance/proj-1");
        assert_ne!(key, active_instance_key("proj-2"));
    }

    #[test]
    fn test_exchange_writes_project_scoped_active_username_key() {
        let key = active_username_key("proj-1");
        assert_eq!(key, "mastodon_active_username/proj-1");
        assert_ne!(key, active_username_key("proj-2"));
    }

    #[tokio::test]
    async fn test_fetch_access_token_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/oauth/token");
            then.status(200).json_body(serde_json::json!({ "access_token": "tok123" }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_access_token(&client, &base_url, "cid", "csec", "authcode").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), "tok123");
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_access_token_error_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/oauth/token");
            then.status(401).body("Unauthorized");
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_access_token(&client, &base_url, "cid", "csec", "badcode").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("401"), "error must contain status code");
    }

    #[tokio::test]
    async fn test_fetch_access_token_missing_access_token() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/oauth/token");
            then.status(200).json_body(serde_json::json!({}));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_access_token(&client, &base_url, "cid", "csec", "authcode").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Missing access_token"),
            "error must mention Missing access_token"
        );
    }

    #[tokio::test]
    async fn test_fetch_acct_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/v1/accounts/verify_credentials");
            then.status(200).json_body(serde_json::json!({ "acct": "user@mastodon.social" }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_acct(&client, &base_url, "mytoken").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), "user@mastodon.social");
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_acct_error_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/accounts/verify_credentials");
            then.status(401).body("Unauthorized");
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_acct(&client, &base_url, "badtoken").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("401"), "error must contain status code");
    }

    #[tokio::test]
    async fn test_fetch_acct_missing_acct_field() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/accounts/verify_credentials");
            then.status(200).json_body(serde_json::json!({}));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = fetch_acct(&client, &base_url, "mytoken").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Missing acct"),
            "error must mention Missing acct"
        );
    }
}
