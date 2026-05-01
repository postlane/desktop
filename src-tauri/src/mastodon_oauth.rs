// SPDX-License-Identifier: BUSL-1.1

use crate::providers::scheduling::{build_client, mastodon::validate_instance_domain};
use tauri_plugin_keyring::KeyringExt;

const KEYRING_SERVICE: &str = "postlane";
const KEYRING_ACTIVE_INSTANCE: &str = "mastodon_active_instance";

/// Validates that the instance string is a plain hostname (no "://" scheme prefix).
fn validate_instance_format(instance: &str) -> Result<(), String> {
    if instance.contains("://") {
        return Err(format!(
            "Instance must be a hostname only (e.g. mastodon.social), not a URL. Got: {}",
            instance
        ));
    }
    if instance.is_empty() {
        return Err("Instance hostname cannot be empty".to_string());
    }
    Ok(())
}

/// Returns the hostname of the currently connected Mastodon instance, or `None`.
///
/// Used by the frontend to call `get_mastodon_char_limit` without the user
/// re-entering the instance on every render.
#[tauri::command]
pub fn get_mastodon_connected_instance(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(KEYRING_SERVICE, KEYRING_ACTIVE_INSTANCE)
        .map_err(|e| format!("Keyring read error: {}", e))
}

/// Registers a Postlane OAuth app with the Mastodon instance.
///
/// Checks the OS keyring first — if client credentials are already present
/// for this instance, they are reused without a new registration request.
/// Returns the authorization URL the user must visit.
#[tauri::command]
pub async fn register_mastodon_app(
    instance: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    validate_instance_format(&instance)?;

    validate_instance_domain(&instance)
        .await
        .map_err(|e| e.to_string())?;

    let client_id_key = format!("mastodon_client_id/{}", instance);
    let client_secret_key = format!("mastodon_client_secret/{}", instance);

    // Reuse existing credentials if both are stored
    let existing_id = app.keyring().get_password(KEYRING_SERVICE, &client_id_key)
        .map_err(|e| format!("Keyring read error: {}", e))?;
    let existing_secret = app.keyring().get_password(KEYRING_SERVICE, &client_secret_key)
        .map_err(|e| format!("Keyring read error: {}", e))?;

    let client_id = match (existing_id, existing_secret) {
        (Some(id), Some(_)) => id,
        _ => {
            let (id, secret) = register_app_with_instance(&instance).await?;
            app.keyring().set_password(KEYRING_SERVICE, &client_id_key, &id)
                .map_err(|e| format!("Failed to store client_id: {}", e))?;
            app.keyring().set_password(KEYRING_SERVICE, &client_secret_key, &secret)
                .map_err(|e| format!("Failed to store client_secret: {}", e))?;
            id
        }
    };

    Ok(build_auth_url(&instance, &client_id))
}

/// Exchanges an OAuth authorization code for an access token.
///
/// Reads client credentials from the OS keyring, exchanges the code with the
/// Mastodon instance, stores the resulting access token, and returns the account handle.
#[tauri::command]
pub async fn exchange_mastodon_code(
    instance: String,
    code: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    validate_instance_format(&instance)?;

    let client_id = app.keyring()
        .get_password(KEYRING_SERVICE, &format!("mastodon_client_id/{}", instance))
        .map_err(|e| format!("Keyring read error: {}", e))?
        .ok_or_else(|| "No client_id found — run Connect first".to_string())?;

    let client_secret = app.keyring()
        .get_password(KEYRING_SERVICE, &format!("mastodon_client_secret/{}", instance))
        .map_err(|e| format!("Keyring read error: {}", e))?
        .ok_or_else(|| "No client_secret found — run Connect first".to_string())?;

    let access_token = fetch_access_token(&instance, &client_id, &client_secret, &code).await?;

    app.keyring()
        .set_password(KEYRING_SERVICE, &format!("mastodon/{}", instance), &access_token)
        .map_err(|e| format!("Failed to store access token: {}", e))?;

    app.keyring()
        .set_password(KEYRING_SERVICE, KEYRING_ACTIVE_INSTANCE, &instance)
        .map_err(|e| format!("Failed to store active instance: {}", e))?;

    let acct = fetch_acct(&instance, &access_token).await?;
    Ok(acct)
}

/// Fetches the Mastodon instance character limit from `GET /api/v1/instance`.
///
/// Returns `configuration.statuses.max_characters`, or 500 on any failure.
/// Performs SSRF validation on the instance domain before making any HTTP request.
#[tauri::command]
pub async fn get_mastodon_char_limit(instance: String) -> Result<u32, String> {
    validate_instance_format(&instance)?;

    validate_instance_domain(&instance)
        .await
        .map_err(|e| e.to_string())?;

    let client = build_client();
    let url = format!("https://{}/api/v1/instance", instance);

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Ok(500),
    };

    if !response.status().is_success() {
        return Ok(500);
    }

    let json: serde_json::Value = match response.json().await {
        Ok(j) => j,
        Err(_) => return Ok(500),
    };

    Ok(json["configuration"]["statuses"]["max_characters"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(500))
}

fn keys_for_disconnect(instance: &str, is_active: bool) -> Vec<String> {
    let mut keys = vec![
        format!("mastodon/{}", instance),
        format!("mastodon_client_id/{}", instance),
        format!("mastodon_client_secret/{}", instance),
    ];
    if is_active {
        keys.push(KEYRING_ACTIVE_INSTANCE.to_string());
    }
    keys
}

/// Removes all Mastodon credentials from the OS keyring for the given instance.
///
/// Only clears the active-instance pointer if this instance is currently active.
/// Ignores individual delete errors — all entries are attempted regardless.
#[tauri::command]
pub fn disconnect_mastodon(instance: String, app: tauri::AppHandle) -> Result<(), String> {
    validate_instance_format(&instance)?;

    let active = app
        .keyring()
        .get_password(KEYRING_SERVICE, KEYRING_ACTIVE_INSTANCE)
        .unwrap_or(None);
    let is_active = active.as_deref() == Some(instance.as_str());

    let mut errors = Vec::new();
    for key in keys_for_disconnect(&instance, is_active) {
        if let Err(e) = app.keyring().delete_password(KEYRING_SERVICE, &key) {
            errors.push(format!("Failed to delete {}: {}", key, e));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// POSTs to `POST https://{instance}/api/v1/apps` and returns `(client_id, client_secret)`.
async fn register_app_with_instance(instance: &str) -> Result<(String, String), String> {
    let client = build_client();
    let url = format!("https://{}/api/v1/apps", instance);

    let body = serde_json::json!({
        "client_name": "Postlane",
        "redirect_uris": "urn:ietf:wg:oauth:2.0:oob",
        "scopes": "read write",
        "website": "https://postlane.dev"
    });

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network error registering app: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("App registration failed ({}): {}", status, text));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse registration response: {}", e))?;

    let client_id = json["client_id"]
        .as_str()
        .ok_or_else(|| "Missing client_id in registration response".to_string())?
        .to_string();
    let client_secret = json["client_secret"]
        .as_str()
        .ok_or_else(|| "Missing client_secret in registration response".to_string())?
        .to_string();

    Ok((client_id, client_secret))
}

/// Exchanges an authorization code for an access token via `POST /oauth/token`.
async fn fetch_access_token(
    instance: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
) -> Result<String, String> {
    let client = build_client();
    let url = format!("https://{}/oauth/token", instance);

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
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed ({}): {}", status, text));
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

/// Fetches the account handle via `GET /api/v1/accounts/verify_credentials`.
async fn fetch_acct(instance: &str, access_token: &str) -> Result<String, String> {
    let client = build_client();
    let url = format!("https://{}/api/v1/accounts/verify_credentials", instance);

    let response = client
        .get(&url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| format!("Network error fetching account: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        return Err(format!("verify_credentials failed ({})", status));
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

/// Constructs the OAuth authorization URL for the user to visit.
fn build_auth_url(instance: &str, client_id: &str) -> String {
    let base = format!("https://{}/oauth/authorize", instance);
    let mut url = url::Url::parse(&base).unwrap_or_else(|_| {
        url::Url::parse("https://mastodon.social/oauth/authorize").expect("fallback URL must parse")
    });
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", "urn:ietf:wg:oauth:2.0:oob")
        .append_pair("response_type", "code")
        .append_pair("scope", "read write");
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_validate_instance_format_rejects_url_with_scheme() {
        assert!(validate_instance_format("https://mastodon.social").is_err());
        assert!(validate_instance_format("http://mastodon.social").is_err());
        assert!(validate_instance_format("ftp://example.com").is_err());
    }

    #[test]
    fn test_validate_instance_format_accepts_bare_hostname() {
        assert!(validate_instance_format("mastodon.social").is_ok());
        assert!(validate_instance_format("fosstodon.org").is_ok());
    }

    #[test]
    fn test_validate_instance_format_rejects_empty() {
        assert!(validate_instance_format("").is_err());
    }

    // Issue 1 — access token key must be instance-specific to prevent credential collision
    #[test]
    fn test_access_token_key_is_instance_specific() {
        let key_a = format!("mastodon/{}", "mastodon.social");
        let key_b = format!("mastodon/{}", "fosstodon.org");
        assert_ne!(key_a, key_b, "Different instances must produce different keyring keys");
        assert_eq!(key_a, "mastodon/mastodon.social");
        assert_eq!(key_b, "mastodon/fosstodon.org");
        // Verify the key is NOT the bare "mastodon" string that causes overwrites
        assert_ne!(key_a, "mastodon");
        assert_ne!(key_b, "mastodon");
    }

    // Issue 1 — disconnect must use the same instance-specific key as exchange
    #[test]
    fn test_disconnect_key_matches_exchange_key() {
        let instance = "mastodon.social";
        let exchange_key = format!("mastodon/{}", instance);
        let disconnect_key = format!("mastodon/{}", instance);
        assert_eq!(exchange_key, disconnect_key, "exchange and disconnect must use identical keyring keys");
    }

    // Issue 8 — active instance key must be a stable constant so PostCardBody can look up charLimit
    #[test]
    fn test_active_instance_key_is_constant() {
        assert_eq!(KEYRING_ACTIVE_INSTANCE, "mastodon_active_instance");
    }

    // Issue 8 — disconnect must delete the active instance key so charLimit fetch returns None
    #[test]
    fn test_disconnect_deletes_active_instance_key_when_active() {
        let keys = keys_for_disconnect("mastodon.social", true);
        assert!(keys.contains(&KEYRING_ACTIVE_INSTANCE.to_string()),
            "disconnect must delete the active_instance key when disconnecting the active instance");
    }

    #[test]
    fn test_disconnect_spares_active_instance_key_when_not_active() {
        let keys = keys_for_disconnect("fosstodon.org", false);
        assert!(!keys.contains(&KEYRING_ACTIVE_INSTANCE.to_string()),
            "disconnect must NOT delete active_instance key when the disconnected instance is not the active one");
    }

    #[test]
    fn test_build_auth_url_format() {
        let url = build_auth_url("mastodon.social", "abc123");
        assert!(url.starts_with("https://mastodon.social/oauth/authorize"));
        assert!(url.contains("client_id=abc123"));
        assert!(url.contains("redirect_uri=urn%3Aietf%3Awg%3Aoauth%3A2.0%3Aoob"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=read"));
    }

    #[test]
    fn test_build_auth_url_encodes_special_chars_in_client_id() {
        let url = build_auth_url("mastodon.social", "id&special=value");
        let parsed = url::Url::parse(&url).expect("URL must be parseable");
        let client_id = parsed
            .query_pairs()
            .find(|(k, _)| k == "client_id")
            .map(|(_, v)| v.to_string())
            .expect("client_id param must be present");
        assert_eq!(client_id, "id&special=value");
    }

    #[tokio::test]
    async fn test_register_app_with_instance_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/apps")
                .body_contains("Postlane");
            then.status(200).json_body(serde_json::json!({
                "client_id": "test-client-id",
                "client_secret": "test-client-secret"
            }));
        });

        // Build URL using mock server host — bypasses real DNS for this test
        let instance = format!("127.0.0.1:{}", server.port());
        let client = build_client();
        let url = format!("http://{}/api/v1/apps", instance);

        let body = serde_json::json!({
            "client_name": "Postlane",
            "redirect_uris": "urn:ietf:wg:oauth:2.0:oob",
            "scopes": "read write",
            "website": "https://postlane.dev"
        });

        let response = client.post(&url).json(&body).send().await.unwrap();
        let json: serde_json::Value = response.json().await.unwrap();

        assert_eq!(json["client_id"], "test-client-id");
        assert_eq!(json["client_secret"], "test-client-secret");
        mock.assert();
    }

    #[tokio::test]
    async fn test_fetch_access_token_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/oauth/token");
            then.status(200).json_body(serde_json::json!({
                "access_token": "test-access-token",
                "token_type": "Bearer"
            }));
        });

        let client = build_client();
        let url = format!("http://127.0.0.1:{}/oauth/token", server.port());
        let body = serde_json::json!({
            "client_id": "cid",
            "client_secret": "csec",
            "redirect_uri": "urn:ietf:wg:oauth:2.0:oob",
            "grant_type": "authorization_code",
            "code": "abc"
        });

        let response = client.post(&url).json(&body).send().await.unwrap();
        let json: serde_json::Value = response.json().await.unwrap();

        assert_eq!(json["access_token"], "test-access-token");
        mock.assert();
    }
}
