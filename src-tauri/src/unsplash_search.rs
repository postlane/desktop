// SPDX-License-Identifier: BUSL-1.1

// Unsplash API requires `Authorization: Client-ID {key}` (NOT Bearer).
// Using Bearer returns 401 with no distinguishing error message.
// https://unsplash.com/documentation#user-authentication

use tauri::State;
use tauri_plugin_keyring::KeyringExt;

use crate::app_state::AppState;

const KEYRING_SERVICE: &str = "postlane";
const UNSPLASH_KEY_ACCOUNT: &str = "postlane/unsplash_access_key";
const UNSPLASH_API_BASE: &str = "https://api.unsplash.com";

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct UnsplashPhoto {
    pub id: String,
    pub description: Option<String>,
    pub urls: UnsplashUrls,
    pub links: UnsplashLinks,
    pub user: UnsplashUser,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct UnsplashUrls {
    pub raw: String,
    pub full: String,
    pub regular: String,
    pub small: String,
    pub thumb: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct UnsplashLinks {
    pub download_location: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct UnsplashUser {
    pub name: String,
    pub links: UnsplashUserLinks,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
pub struct UnsplashUserLinks {
    pub html: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct SearchResponse {
    results: Vec<UnsplashPhoto>,
}

pub fn save_unsplash_key_impl(app: &tauri::AppHandle, access_key: &str) -> Result<(), String> {
    app.keyring()
        .set_password(KEYRING_SERVICE, UNSPLASH_KEY_ACCOUNT, access_key)
        .map_err(|e| format!("Failed to save Unsplash key: {}", e))
}

pub fn delete_unsplash_key_impl(app: &tauri::AppHandle) -> Result<(), String> {
    app.keyring()
        .delete_password(KEYRING_SERVICE, UNSPLASH_KEY_ACCOUNT)
        .map_err(|e| format!("Failed to delete Unsplash key: {}", e))
}

pub fn get_unsplash_key_impl(app: &tauri::AppHandle) -> Option<String> {
    app.keyring()
        .get_password(KEYRING_SERVICE, UNSPLASH_KEY_ACCOUNT)
        .ok()
        .flatten()
}

pub fn has_unsplash_key_impl(app: &tauri::AppHandle) -> bool {
    get_unsplash_key_impl(app).is_some()
}

pub async fn search_unsplash_impl(
    query: &str,
    access_key: &str,
    base_url: &str,
    page: u32,
) -> Result<Vec<UnsplashPhoto>, String> {
    let encoded_query: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let url = format!("{}/search/photos?query={}&per_page=20&page={}", base_url, encoded_query, page);
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        // Authorization: Client-ID is required; Bearer returns 401 with no error detail.
        .header("Authorization", format!("Client-ID {}", access_key))
        .send()
        .await
        .map_err(|e| format!("Unsplash request failed: {}", e))?;
    match response.status().as_u16() {
        200 => {
            let body: SearchResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Unsplash response: {}", e))?;
            Ok(body.results)
        }
        429 => Err("rate_limit".to_string()),
        401 => Err("unauthorized".to_string()),
        status => Err(format!("Unsplash API error: {}", status)),
    }
}

#[tauri::command]
pub async fn save_unsplash_key(
    access_key: String,
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    save_unsplash_key_impl(&app, &access_key)
}

#[tauri::command]
pub async fn delete_unsplash_key(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    delete_unsplash_key_impl(&app)
}

#[tauri::command]
pub async fn has_unsplash_key(
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<bool, String> {
    Ok(has_unsplash_key_impl(&app))
}

pub async fn trigger_unsplash_download_impl(
    download_location: &str,
    access_key: &str,
    allowed_base: &str,
) -> Result<(), String> {
    let prefix = format!("{}/", allowed_base);
    if !download_location.starts_with(&prefix) {
        return Err(format!(
            "Invalid download_location: must start with {} (got: {})",
            prefix, download_location
        ));
    }
    reqwest::Client::new()
        .get(download_location)
        .header("Authorization", format!("Client-ID {}", access_key))
        .send()
        .await
        .map_err(|e| format!("trigger_unsplash_download failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn trigger_unsplash_download(
    download_location: String,
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    let key = get_unsplash_key_impl(&app)
        .ok_or_else(|| "No Unsplash API key configured".to_string())?;
    trigger_unsplash_download_impl(&download_location, &key, UNSPLASH_API_BASE).await
}

#[tauri::command]
pub async fn search_unsplash(
    query: String,
    page: Option<u32>,
    app: tauri::AppHandle,
    _state: State<'_, AppState>,
) -> Result<Vec<UnsplashPhoto>, String> {
    let key = get_unsplash_key_impl(&app)
        .ok_or_else(|| "No Unsplash API key configured".to_string())?;
    search_unsplash_impl(&query, &key, UNSPLASH_API_BASE, page.unwrap_or(1)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn test_search_unsplash_sends_page_parameter() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/search/photos").query_param("page", "3");
            then.status(200).json_body(serde_json::json!({
                "results": [], "total": 0, "total_pages": 0
            }));
        });
        let result = search_unsplash_impl("rust", "key", &server.base_url(), 3).await;
        mock.assert();
        assert!(result.is_ok(), "should send page=3: {:?}", result);
    }

    // 21.8.27: search request must use Authorization: Client-ID {key} (not Bearer).
    #[tokio::test]
    async fn test_search_unsplash_sends_client_id_header() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/search/photos")
                .header_exists("Authorization")
                .header("Authorization", "Client-ID test-key-123");
            then.status(200).json_body(serde_json::json!({
                "results": [],
                "total": 0,
                "total_pages": 0
            }));
        });
        let result = search_unsplash_impl("rust programming", "test-key-123", &server.base_url(), 1).await;
        mock.assert();
        assert!(result.is_ok(), "should succeed with correct header: {:?}", result);
    }

    #[tokio::test]
    async fn test_search_unsplash_does_not_use_bearer_header() {
        let server = MockServer::start();
        // Only respond 200 if the header is Client-ID — Bearer must not be sent.
        let mock = server.mock(|when, then| {
            when.method(GET).path("/search/photos").header("Authorization", "Bearer test-key-123");
            then.status(401);
        });
        let result = search_unsplash_impl("test", "test-key-123", &server.base_url(), 1).await;
        // The mock for Bearer should not have been called.
        assert_eq!(mock.hits(), 0, "Authorization: Bearer must not be sent");
        // The request may fail (no matching mock for Client-ID), but must not succeed via Bearer.
        drop(result);
    }

    #[tokio::test]
    async fn test_search_unsplash_returns_rate_limit_error_on_429() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/photos");
            then.status(429);
        });
        let result = search_unsplash_impl("test", "key", &server.base_url(), 1).await;
        assert_eq!(result, Err("rate_limit".to_string()));
    }

    #[tokio::test]
    async fn test_search_unsplash_returns_unauthorized_error_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/photos");
            then.status(401);
        });
        let result = search_unsplash_impl("test", "key", &server.base_url(), 1).await;
        assert_eq!(result, Err("unauthorized".to_string()));
    }

    // 21.8.29 — trigger_unsplash_download

    #[tokio::test]
    async fn test_trigger_download_rejects_non_unsplash_url() {
        let result = trigger_unsplash_download_impl(
            "https://evil.com/photos/abc/download",
            "key",
            "https://api.unsplash.com",
        )
        .await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("api.unsplash.com"),
            "error must name the required prefix"
        );
    }

    #[tokio::test]
    async fn test_trigger_download_sends_client_id_header() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/photos/abc/download")
                .header("Authorization", "Client-ID demo-key");
            then.status(200)
                .json_body(serde_json::json!({ "url": "https://images.unsplash.com/photo-abc" }));
        });
        let url = format!("{}/photos/abc/download", server.base_url());
        let result = trigger_unsplash_download_impl(&url, "demo-key", &server.base_url()).await;
        mock.assert();
        assert!(result.is_ok(), "should succeed: {:?}", result);
    }

    #[tokio::test]
    async fn test_trigger_download_succeeds_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/photos/abc/download");
            then.status(200)
                .json_body(serde_json::json!({ "url": "https://images.unsplash.com/photo-abc" }));
        });
        let url = format!("{}/photos/abc/download", server.base_url());
        let result = trigger_unsplash_download_impl(&url, "key", &server.base_url()).await;
        assert!(result.is_ok(), "200 should return Ok: {:?}", result);
    }

    #[tokio::test]
    async fn test_search_unsplash_returns_results_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/search/photos");
            then.status(200).json_body(serde_json::json!({
                "results": [{
                    "id": "photo-abc",
                    "description": "A test photo",
                    "urls": {
                        "raw": "https://images.unsplash.com/photo-abc",
                        "full": "https://images.unsplash.com/photo-abc?w=2000",
                        "regular": "https://images.unsplash.com/photo-abc?w=1080",
                        "small": "https://images.unsplash.com/photo-abc?w=400",
                        "thumb": "https://images.unsplash.com/photo-abc?w=200"
                    },
                    "links": {
                        "download_location": "https://api.unsplash.com/photos/abc/download"
                    },
                    "user": {
                        "name": "Jane Doe",
                        "links": { "html": "https://unsplash.com/@janedoe" }
                    }
                }],
                "total": 1,
                "total_pages": 1
            }));
        });
        let result = search_unsplash_impl("test", "key", &server.base_url(), 1).await;
        assert!(result.is_ok(), "should parse results: {:?}", result);
        let photos = result.unwrap();
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].id, "photo-abc");
        assert_eq!(photos[0].user.name, "Jane Doe");
        assert_eq!(
            photos[0].links.download_location,
            "https://api.unsplash.com/photos/abc/download"
        );
    }
}
