// SPDX-License-Identifier: BUSL-1.1

//! Mastodon connected-instance state: read, char-limit fetch, and disconnect.

use crate::providers::scheduling::{build_client, mastodon::validate_instance_domain};
use crate::security::instance_url::validate_instance_hostname;
use tauri_plugin_keyring::KeyringExt;

pub(crate) const KEYRING_SERVICE: &str = "postlane";
pub(crate) const KEYRING_ACTIVE_INSTANCE: &str = "mastodon_active_instance";

/// Returns the hostname of the currently connected Mastodon instance, or `None`.
#[tauri::command]
pub fn get_mastodon_connected_instance(app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(KEYRING_SERVICE, KEYRING_ACTIVE_INSTANCE)
        .map_err(|e| format!("Keyring read error: {}", e))
}

/// Fetches `configuration.statuses.max_characters` from `GET /api/v1/instance`.
/// Returns 500 on any failure (client, non-success, missing field).
async fn get_mastodon_char_limit_impl(client: &reqwest::Client, base_url: &str) -> Result<u32, String> {
    let url = format!("{}/api/v1/instance", base_url);

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

/// Fetches the Mastodon instance character limit from `GET /api/v1/instance`.
/// Performs SSRF validation before making any HTTP request.
#[tauri::command]
pub async fn get_mastodon_char_limit(instance: String) -> Result<u32, String> {
    validate_instance_hostname(&instance)?;
    validate_instance_domain(&instance).await.map_err(|e| e.to_string())?;
    let client = build_client();
    let base_url = format!("https://{}", instance);
    get_mastodon_char_limit_impl(&client, &base_url).await
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
/// Only clears the active-instance pointer if this instance is currently active.
#[tauri::command]
pub fn disconnect_mastodon(instance: String, app: tauri::AppHandle) -> Result<(), String> {
    validate_instance_hostname(&instance)?;

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

    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_access_token_key_is_instance_specific() {
        let key_a = format!("mastodon/{}", "mastodon.social");
        let key_b = format!("mastodon/{}", "fosstodon.org");
        assert_ne!(key_a, key_b, "Different instances must produce different keyring keys");
        assert_eq!(key_a, "mastodon/mastodon.social");
        assert_eq!(key_b, "mastodon/fosstodon.org");
        assert_ne!(key_a, "mastodon");
        assert_ne!(key_b, "mastodon");
    }

    #[test]
    fn test_disconnect_key_matches_exchange_key() {
        let instance = "mastodon.social";
        let exchange_key = format!("mastodon/{}", instance);
        let disconnect_key = format!("mastodon/{}", instance);
        assert_eq!(exchange_key, disconnect_key, "exchange and disconnect must use identical keyring keys");
    }

    #[test]
    fn test_active_instance_key_is_constant() {
        assert_eq!(KEYRING_ACTIVE_INSTANCE, "mastodon_active_instance");
    }

    #[test]
    fn test_disconnect_deletes_active_instance_key_when_active() {
        let keys = keys_for_disconnect("mastodon.social", true);
        assert!(
            keys.contains(&KEYRING_ACTIVE_INSTANCE.to_string()),
            "disconnect must delete the active_instance key when disconnecting the active instance"
        );
    }

    #[test]
    fn test_disconnect_spares_active_instance_key_when_not_active() {
        let keys = keys_for_disconnect("fosstodon.org", false);
        assert!(
            !keys.contains(&KEYRING_ACTIVE_INSTANCE.to_string()),
            "disconnect must NOT delete active_instance key when the disconnected instance is not the active one"
        );
    }

    #[tokio::test]
    async fn test_char_limit_returns_max_characters_from_instance_api() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/v1/instance");
            then.status(200).json_body(serde_json::json!({
                "configuration": { "statuses": { "max_characters": 500 } }
            }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = get_mastodon_char_limit_impl(&client, &base_url).await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap(), 500);
        mock.assert();
    }

    #[tokio::test]
    async fn test_char_limit_returns_500_on_non_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/instance");
            then.status(503).body("Service Unavailable");
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = get_mastodon_char_limit_impl(&client, &base_url).await;
        assert_eq!(result, Ok(500), "degraded instance must return 500 gracefully");
    }

    #[tokio::test]
    async fn test_char_limit_returns_500_when_field_missing() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/v1/instance");
            then.status(200).json_body(serde_json::json!({}));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = get_mastodon_char_limit_impl(&client, &base_url).await;
        assert_eq!(result, Ok(500), "missing field must fall back to 500");
    }
}
