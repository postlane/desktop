// SPDX-License-Identifier: BUSL-1.1

use crate::license::validator::{validate_token_with_client, LicenseCache, LicenseState};
use chrono::Utc;

#[derive(Debug)]
pub enum DeepLinkError {
    /// URL does not match `postlane://activate?token=...`
    InvalidUrl(String),
    /// Token string is not a valid JWT structure (three dot-separated segments)
    MalformedToken,
    /// Backend returned 401 — token is invalid or expired
    TokenRejected,
    /// Backend is unavailable (503 / network error)
    BackendUnavailable,
    /// Keyring write failed
    KeyringWrite(String),
    /// Cache write failed
    CacheWrite(String),
}

impl std::fmt::Display for DeepLinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl(u) => write!(f, "URL is not a postlane://activate link: {}", u),
            Self::MalformedToken => write!(f, "Token is not a valid JWT (expected three segments)"),
            Self::TokenRejected => write!(f, "Token was rejected by the license server"),
            Self::BackendUnavailable => write!(f, "License server is unavailable — try again later"),
            Self::KeyringWrite(e) => write!(f, "Failed to store token in keyring: {}", e),
            Self::CacheWrite(e) => write!(f, "Failed to write license cache: {}", e),
        }
    }
}

/// Extracts and validates the token from a `postlane://activate?token=...` URL.
/// Returns the raw JWT string on success.
pub fn parse_activate_url(url: &str) -> Result<String, DeepLinkError> {
    let parsed = url::Url::parse(url)
        .map_err(|_| DeepLinkError::InvalidUrl(url.to_owned()))?;

    if parsed.scheme() != "postlane" || parsed.host_str() != Some("activate") {
        return Err(DeepLinkError::InvalidUrl(url.to_owned()));
    }

    let token = parsed
        .query_pairs()
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| DeepLinkError::InvalidUrl(url.to_owned()))?;

    if token.split('.').count() != 3 {
        return Err(DeepLinkError::MalformedToken);
    }

    Ok(token)
}

/// Validates `token` against the backend, stores it in the keyring and cache on success,
/// and returns the `display_name` to show in the confirmation banner.
pub async fn handle_activate(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    mut keyring_write: impl FnMut(&str) -> Result<(), String>,
    mut cache_write: impl FnMut(&LicenseCache) -> Result<(), String>,
) -> Result<String, DeepLinkError> {
    match validate_token_with_client(token, client, base_url).await {
        Ok(LicenseState::Valid { user, repos }) => {
            keyring_write(token).map_err(DeepLinkError::KeyringWrite)?;
            let cache = LicenseCache {
                version: 1,
                validated_at: Utc::now(),
                user: user.clone(),
                repos,
            };
            cache_write(&cache).map_err(DeepLinkError::CacheWrite)?;
            Ok(user.display_name)
        }
        Ok(LicenseState::Expired) => Err(DeepLinkError::TokenRejected),
        Ok(LicenseState::Unconfigured) => Err(DeepLinkError::MalformedToken),
        Ok(LicenseState::Offline { .. }) | Err(_) => Err(DeepLinkError::BackendUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;

    // ── parse_activate_url ──────────────────────────────────────────────────

    #[test]
    fn test_parse_activate_url_extracts_token() {
        let url = "postlane://activate?token=header.payload.sig";
        let token = parse_activate_url(url).unwrap();
        assert_eq!(token, "header.payload.sig");
    }

    #[test]
    fn test_parse_activate_url_rejects_wrong_scheme() {
        let url = "https://activate?token=a.b.c";
        assert!(matches!(parse_activate_url(url), Err(DeepLinkError::InvalidUrl(_))));
    }

    #[test]
    fn test_parse_activate_url_rejects_wrong_host() {
        let url = "postlane://other?token=a.b.c";
        assert!(matches!(parse_activate_url(url), Err(DeepLinkError::InvalidUrl(_))));
    }

    #[test]
    fn test_parse_activate_url_rejects_missing_token() {
        let url = "postlane://activate?foo=bar";
        assert!(matches!(parse_activate_url(url), Err(DeepLinkError::InvalidUrl(_))));
    }

    #[test]
    fn test_parse_activate_url_rejects_two_segment_token() {
        let url = "postlane://activate?token=only.twosegments";
        assert!(matches!(parse_activate_url(url), Err(DeepLinkError::MalformedToken)));
    }

    #[test]
    fn test_parse_activate_url_rejects_four_segment_token() {
        let url = "postlane://activate?token=a.b.c.d";
        assert!(matches!(parse_activate_url(url), Err(DeepLinkError::MalformedToken)));
    }

    // ── handle_activate ─────────────────────────────────────────────────────

    fn valid_response() -> serde_json::Value {
        serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": "Ada Lovelace", "avatar_url": null },
            "repos": []
        })
    }

    #[tokio::test]
    async fn test_handle_activate_stores_token_and_returns_display_name() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(200).json_body(valid_response());
        });

        let mut stored_token = String::new();
        let mut cache_written = false;
        let client = build_client();

        let result = handle_activate(
            "header.payload.sig",
            &client,
            &server.base_url(),
            |t| { stored_token = t.to_owned(); Ok(()) },
            |_| { cache_written = true; Ok(()) },
        )
        .await;

        assert_eq!(result.unwrap(), "Ada Lovelace");
        assert_eq!(stored_token, "header.payload.sig");
        assert!(cache_written);
    }

    #[tokio::test]
    async fn test_handle_activate_does_not_store_token_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(401).json_body(serde_json::json!({"valid": false, "reason": "expired"}));
        });

        let mut stored = false;
        let client = build_client();

        let result = handle_activate(
            "header.payload.sig",
            &client,
            &server.base_url(),
            |_| { stored = true; Ok(()) },
            |_| Ok(()),
        )
        .await;

        assert!(matches!(result, Err(DeepLinkError::TokenRejected)));
        assert!(!stored, "token must not be stored on 401");
    }

    #[tokio::test]
    async fn test_handle_activate_does_not_store_token_on_503() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/license/validate");
            then.status(503);
        });

        let mut stored = false;
        let client = build_client();

        let result = handle_activate(
            "header.payload.sig",
            &client,
            &server.base_url(),
            |_| { stored = true; Ok(()) },
            |_| Ok(()),
        )
        .await;

        assert!(matches!(result, Err(DeepLinkError::BackendUnavailable)));
        assert!(!stored, "token must not be stored on 503");
    }

    #[tokio::test]
    async fn test_handle_activate_returns_backend_unavailable_on_network_error() {
        let client = build_client();
        let result = handle_activate(
            "header.payload.sig",
            &client,
            "http://127.0.0.1:19999", // nothing listening
            |_| Ok(()),
            |_| Ok(()),
        )
        .await;
        assert!(matches!(result, Err(DeepLinkError::BackendUnavailable)));
    }
}
