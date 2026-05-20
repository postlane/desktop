// SPDX-License-Identifier: BUSL-1.1

//! Mastodon OAuth app registration and authorization URL construction.

use crate::mastodon_connection::KEYRING_SERVICE;
use crate::providers::scheduling::{build_client, mastodon::validate_instance_domain};
use crate::security::api_error::format_api_error;
use crate::security::instance_url::validate_instance_hostname;
use tauri_plugin_keyring::KeyringExt;

/// Registers a Postlane OAuth app with the Mastodon instance.
///
/// Reuses existing keyring credentials when present to avoid a redundant request.
/// Returns the authorization URL the user must visit.
#[tauri::command]
pub async fn register_mastodon_app(
    instance: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    validate_instance_hostname(&instance)?;
    validate_instance_domain(&instance).await.map_err(|e| e.to_string())?;

    let client_id_key = format!("mastodon_client_id/{}", instance);
    let client_secret_key = format!("mastodon_client_secret/{}", instance);

    let existing_id = app.keyring().get_password(KEYRING_SERVICE, &client_id_key)
        .map_err(|e| format!("Keyring read error: {}", e))?;
    let existing_secret = app.keyring().get_password(KEYRING_SERVICE, &client_secret_key)
        .map_err(|e| format!("Keyring read error: {}", e))?;

    let client_id = match (existing_id, existing_secret) {
        (Some(id), Some(_)) => id,
        _ => {
            let client = build_client();
            let base_url = format!("https://{}", instance);
            let (id, secret) = register_app_with_instance(&client, &base_url).await?;
            app.keyring().set_password(KEYRING_SERVICE, &client_id_key, &id)
                .map_err(|e| format!("Failed to store client_id: {}", e))?;
            app.keyring().set_password(KEYRING_SERVICE, &client_secret_key, &secret)
                .map_err(|e| format!("Failed to store client_secret: {}", e))?;
            id
        }
    };

    build_auth_url(&instance, &client_id)
}

/// POSTs to `POST {base_url}/api/v1/apps` and returns `(client_id, client_secret)`.
pub(crate) async fn register_app_with_instance(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<(String, String), String> {
    let url = format!("{}/api/v1/apps", base_url);

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
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return Err(format_api_error("App registration failed", status, &text));
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

/// Constructs the OAuth authorization URL for the user to visit.
pub(crate) fn build_auth_url(instance: &str, client_id: &str) -> Result<String, String> {
    let base = format!("https://{}/oauth/authorize", instance);
    let mut url = url::Url::parse(&base)
        .map_err(|e| format!("Invalid Mastodon instance '{}': {}", instance, e))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", "urn:ietf:wg:oauth:2.0:oob")
        .append_pair("response_type", "code")
        .append_pair("scope", "read write");
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_build_auth_url_format() {
        let url = build_auth_url("mastodon.social", "abc123").expect("valid instance must not fail");
        assert!(url.starts_with("https://mastodon.social/oauth/authorize"));
        assert!(url.contains("client_id=abc123"));
        assert!(url.contains("redirect_uri=urn%3Aietf%3Awg%3Aoauth%3A2.0%3Aoob"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=read"));
    }

    #[test]
    fn test_build_auth_url_encodes_special_chars_in_client_id() {
        let url = build_auth_url("mastodon.social", "id&special=value")
            .expect("valid instance must not fail");
        let parsed = url::Url::parse(&url).expect("URL must be parseable");
        let client_id = parsed
            .query_pairs()
            .find(|(k, _)| k == "client_id")
            .map(|(_, v)| v.to_string())
            .expect("client_id param must be present");
        assert_eq!(client_id, "id&special=value");
    }

    #[test]
    fn test_build_auth_url_returns_error_for_invalid_instance() {
        let result = build_auth_url("not a valid::hostname", "abc123");
        assert!(
            result.is_err(),
            "invalid instance must return Err, not a silent mastodon.social fallback"
        );
    }

    #[tokio::test]
    async fn test_register_app_success() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/apps");
            then.status(200).json_body(serde_json::json!({
                "client_id": "test-client-id",
                "client_secret": "test-client-secret"
            }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = register_app_with_instance(&client, &base_url).await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let (id, secret) = result.unwrap();
        assert_eq!(id, "test-client-id");
        assert_eq!(secret, "test-client-secret");
        mock.assert();
    }

    #[tokio::test]
    async fn test_register_app_error_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/apps");
            then.status(422).body("Unprocessable Entity");
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = register_app_with_instance(&client, &base_url).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("422"), "error must contain status code");
    }

    #[tokio::test]
    async fn test_register_app_missing_client_id() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/apps");
            then.status(200).json_body(serde_json::json!({ "client_secret": "sec" }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = register_app_with_instance(&client, &base_url).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Missing client_id"),
            "error must mention Missing client_id"
        );
    }

    #[tokio::test]
    async fn test_register_app_missing_client_secret() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/api/v1/apps");
            then.status(200).json_body(serde_json::json!({ "client_id": "cid" }));
        });
        let client = build_client();
        let base_url = format!("http://127.0.0.1:{}", server.port());
        let result = register_app_with_instance(&client, &base_url).await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("Missing client_secret"),
            "error must mention Missing client_secret"
        );
    }
}
