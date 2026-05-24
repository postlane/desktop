// SPDX-License-Identifier: BUSL-1.1

//! Mastodon connected-instance state: read, char-limit fetch, and disconnect.

use crate::providers::scheduling::{build_client, mastodon::validate_instance_domain};
use crate::security::instance_url::validate_instance_hostname;
use tauri_plugin_keyring::KeyringExt;

pub(crate) const KEYRING_SERVICE: &str = "postlane";

pub(crate) fn active_instance_key(project_id: &str) -> String {
    format!("mastodon_active_instance/{}", project_id)
}

pub(crate) fn active_username_key(project_id: &str) -> String {
    format!("mastodon_active_username/{}", project_id)
}

pub(crate) fn access_token_key(project_id: &str, instance: &str) -> String {
    format!("mastodon/{}/{}", project_id, instance)
}

/// Connected Mastodon account — instance hostname plus the account handle.
#[derive(serde::Serialize)]
pub struct MastodonAccount {
    pub instance: String,
    pub username: String,
}

/// Returns the hostname of the currently connected Mastodon instance for the given project, or `None`.
#[tauri::command]
pub fn get_mastodon_connected_instance(project_id: String, app: tauri::AppHandle) -> Result<Option<String>, String> {
    app.keyring()
        .get_password(KEYRING_SERVICE, &active_instance_key(&project_id))
        .map_err(|e| format!("Keyring read error: {}", e))
}

/// Returns the connected Mastodon account (instance + username) for the given project, or `None`.
#[tauri::command]
pub fn get_mastodon_connected_account(project_id: String, app: tauri::AppHandle) -> Result<Option<MastodonAccount>, String> {
    let instance = app.keyring()
        .get_password(KEYRING_SERVICE, &active_instance_key(&project_id))
        .map_err(|e| format!("Keyring read error: {}", e))?;
    let username = app.keyring()
        .get_password(KEYRING_SERVICE, &active_username_key(&project_id))
        .map_err(|e| format!("Keyring read error: {}", e))?;
    match (instance, username) {
        (Some(instance), Some(username)) => Ok(Some(MastodonAccount { instance, username })),
        _ => Ok(None),
    }
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

fn keys_for_disconnect(project_id: &str, instance: &str, is_active: bool) -> Vec<String> {
    let mut keys = vec![
        access_token_key(project_id, instance),
        format!("mastodon_client_id/{}", instance),
        format!("mastodon_client_secret/{}", instance),
    ];
    if is_active {
        keys.push(active_instance_key(project_id));
        keys.push(active_username_key(project_id));
    }
    keys
}

/// Removes all Mastodon credentials from the OS keyring for the given instance and project.
/// Only clears the active-instance pointer if this instance is currently active.
#[tauri::command]
pub fn disconnect_mastodon(project_id: String, instance: String, app: tauri::AppHandle) -> Result<(), String> {
    validate_instance_hostname(&instance)?;

    let active = app
        .keyring()
        .get_password(KEYRING_SERVICE, &active_instance_key(&project_id))
        .unwrap_or(None);
    let is_active = active.as_deref() == Some(instance.as_str());

    let mut errors = Vec::new();
    for key in keys_for_disconnect(&project_id, &instance, is_active) {
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

    // ── Project-scoped key format (§ mastodon-scope) ──────────────────────────

    #[test]
    fn test_active_instance_key_is_project_scoped() {
        assert_eq!(active_instance_key("proj-1"), "mastodon_active_instance/proj-1");
    }

    #[test]
    fn test_active_username_key_is_project_scoped() {
        assert_eq!(active_username_key("proj-1"), "mastodon_active_username/proj-1");
    }

    #[test]
    fn test_access_token_key_is_project_and_instance_scoped() {
        assert_eq!(access_token_key("proj-1", "mastodon.social"), "mastodon/proj-1/mastodon.social");
    }

    #[test]
    fn test_different_projects_have_different_active_instance_keys() {
        assert_ne!(active_instance_key("proj-1"), active_instance_key("proj-2"));
    }

    #[test]
    fn test_access_token_keys_isolated_across_projects() {
        assert_ne!(
            access_token_key("proj-1", "mastodon.social"),
            access_token_key("proj-2", "mastodon.social"),
        );
    }

    #[test]
    fn test_access_token_keys_differ_across_instances_within_same_project() {
        assert_ne!(
            access_token_key("proj-1", "mastodon.social"),
            access_token_key("proj-1", "fosstodon.org"),
        );
    }

    // ── Disconnect key sets ───────────────────────────────────────────────────

    #[test]
    fn test_disconnect_active_keys_are_project_scoped() {
        let keys = keys_for_disconnect("proj-1", "mastodon.social", true);
        assert!(keys.contains(&active_instance_key("proj-1")));
        assert!(keys.contains(&active_username_key("proj-1")));
        assert!(!keys.contains(&active_instance_key("proj-2")),
            "disconnect must not touch other projects' active-instance key");
    }

    #[test]
    fn test_disconnect_access_token_key_is_project_scoped() {
        let keys = keys_for_disconnect("proj-1", "mastodon.social", false);
        assert!(keys.contains(&access_token_key("proj-1", "mastodon.social")));
        assert!(!keys.contains(&access_token_key("proj-2", "mastodon.social")),
            "disconnect must not touch other projects' access token");
    }

    #[test]
    fn test_disconnect_spares_active_keys_when_not_active_instance() {
        let keys = keys_for_disconnect("proj-1", "fosstodon.org", false);
        assert!(!keys.contains(&active_instance_key("proj-1")));
        assert!(!keys.contains(&active_username_key("proj-1")));
    }

    #[test]
    fn test_disconnect_access_token_key_matches_exchange_key() {
        let project_id = "proj-1";
        let instance = "mastodon.social";
        let exchange_key = access_token_key(project_id, instance);
        let disconnect_keys = keys_for_disconnect(project_id, instance, false);
        assert!(disconnect_keys.contains(&exchange_key),
            "disconnect must delete the same key that exchange_mastodon_code writes");
    }

    // ── char limit HTTP ───────────────────────────────────────────────────────

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
