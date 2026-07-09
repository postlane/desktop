// SPDX-License-Identifier: BUSL-1.1

//! Startup project-cache warm. Fetches the user's project list from the API on
//! app startup so the local HTTP server's `/api/v1/projects` endpoint is ready
//! before the CLI runs `postlane init`.

use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_keyring::KeyringExt;
use crate::project_registry::ProjectSummary;

/// Fetches projects from `base_url/api/v1/projects` and writes them to `cache`.
///
/// Failures are swallowed — startup continues whether or not the fetch succeeds.
/// This is the inner logic, separated from the `AppHandle` dependency so tests
/// can call it directly with a mock server URL.
pub async fn warm_projects_cache(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    cache: &Arc<tokio::sync::RwLock<Vec<ProjectSummary>>>,
) {
    match crate::project_api::list_projects_with_client(client, base_url, token).await {
        Ok(projects) => {
            let count = projects.len();
            *cache.write().await = projects;
            log::info!("[startup_projects] warmed cache with {} project(s)", count);
        }
        Err(e) => {
            log::warn!("[startup_projects] failed to warm projects cache: {}", e);
        }
    }
}

/// Spawns a one-shot task that warms the in-memory projects cache from the API.
/// Called once from `spawn_http_server` after the HTTP server starts.
pub fn spawn_startup_projects_refresh(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let token = match app_handle.keyring().get_password("postlane", "license") {
            Ok(Some(t)) => t,
            Ok(None) => {
                log::debug!("[startup_projects] no license token — skipping project cache warm");
                return;
            }
            Err(e) => {
                log::warn!("[startup_projects] keyring error reading license token: {}", e);
                return;
            }
        };
        let client = crate::providers::scheduling::build_client();
        let state: tauri::State<crate::app_state::AppState> = app_handle.state();
        warm_projects_cache(&client, crate::license::POSTLANE_API_BASE, &token, &state.projects_cache).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn make_cache() -> Arc<tokio::sync::RwLock<Vec<ProjectSummary>>> {
        Arc::new(tokio::sync::RwLock::new(vec![]))
    }

    fn make_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    #[tokio::test]
    async fn test_warm_projects_cache_populates_on_success() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "projects": [
                    {"id": "proj-123", "name": "my-proj", "workspace_type": "personal",
                     "tier": "free", "billing_active": false, "is_owner": true, "status": "free_owned"}
                ]
            }));
        });
        let cache = make_cache();
        warm_projects_cache(&make_client(), &server.base_url(), "test-tok", &cache).await;
        assert_eq!(cache.read().await.len(), 1);
        assert_eq!(cache.read().await[0].id, "proj-123");
    }

    #[tokio::test]
    async fn test_warm_projects_cache_empty_on_api_error() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(401);
        });
        let cache = make_cache();
        warm_projects_cache(&make_client(), &server.base_url(), "bad-tok", &cache).await;
        // Must not panic; cache stays empty
        assert_eq!(cache.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_warm_projects_cache_empty_on_network_error() {
        let cache = make_cache();
        // Port 1 is always refused
        warm_projects_cache(&make_client(), "http://127.0.0.1:1", "tok", &cache).await;
        assert_eq!(cache.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_warm_projects_cache_multiple_projects() {
        let server = MockServer::start();
        let _m = server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "projects": [
                    {"id": "a", "name": "alpha", "workspace_type": "personal", "tier": "free", "billing_active": false, "is_owner": true, "status": "free_owned"},
                    {"id": "b", "name": "beta",  "workspace_type": "personal", "tier": "free", "billing_active": false, "is_owner": true, "status": "free_owned"}
                ]
            }));
        });
        let cache = make_cache();
        warm_projects_cache(&make_client(), &server.base_url(), "tok", &cache).await;
        assert_eq!(cache.read().await.len(), 2);
    }
}
