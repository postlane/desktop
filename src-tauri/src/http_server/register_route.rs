// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use super::{error_response, RegisterRequest, RegisterResponse, ServerState};

/// Validates that `path` is a git repo with `.postlane/config.json`.
/// Returns `(canonical_str, repo_name)` on success.
fn validate_repo_path_for_register(path: &str) -> Result<(String, String), (StatusCode, String)> {
    let canonical_path = std::fs::canonicalize(path)
        .map_err(|_| (StatusCode::FORBIDDEN, "Path not found or not accessible".to_string()))?;

    if !canonical_path.join(".git").exists() {
        return Err((StatusCode::BAD_REQUEST, "Not a git repository".to_string()));
    }

    if !canonical_path.join(".postlane").join("config.json").exists() {
        return Err((StatusCode::BAD_REQUEST, ".postlane/config.json not found - run postlane init first".to_string()));
    }

    let canonical_str = canonical_path
        .to_str()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Path contains invalid UTF-8 characters".to_string()))?;

    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Unable to extract repository name from path".to_string()))?;

    Ok((canonical_str.to_string(), name.to_string()))
}

pub(super) async fn register_handler(
    State(state): State<ServerState>,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    let (canonical_str, name) = match validate_repo_path_for_register(&payload.path) {
        Ok(v) => v,
        Err((status, msg)) => return error_response(status, msg),
    };

    let repo = crate::storage::Repo {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.clone(),
        path: canonical_str.clone(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut repos = state.repos.lock().await;
    repos.repos.push(repo.clone());

    if let Err(e) = crate::storage::write_repos(&state.repos_path, &repos) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write repos.json: {:?}", e),
        );
    }
    drop(repos);

    if let Some(tx) = &state.watcher_tx {
        let _ = tx.try_send((repo.id.clone(), canonical_str));
    }

    if let Some(app_handle) = &state.app_handle {
        use tauri::Manager;
        if let Some(app_state) = app_handle.try_state::<crate::app_state::AppState>() {
            add_legacy_repo_to_app_repos(&app_state, &repo);
        }
    }

    (StatusCode::OK, Json(RegisterResponse { success: true, name })).into_response()
}

/// Updates the in-memory AppState repos so `get_all_drafts` finds the legacy repo
/// immediately without requiring an app restart.
pub(crate) fn add_legacy_repo_to_app_repos(
    app_state: &crate::app_state::AppState,
    repo: &crate::storage::Repo,
) {
    if let Ok(mut repos) = app_state.lock_repos() {
        repos.repos.push(repo.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::create_router;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_app_state() -> crate::app_state::AppState {
        let tmp = tempfile::TempDir::new().expect("tmp dir");
        crate::app_state::AppState::new_with_path(
            crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] },
            tmp.path().join("repos.json"),
        )
    }

    fn make_repo(id: &str, path: &str) -> crate::storage::Repo {
        crate::storage::Repo {
            id: id.to_string(),
            name: "repo".to_string(),
            path: path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // register_route.rs lines 12-34: validate_repo_path_for_register

    #[test]
    fn test_validate_repo_path_for_register_returns_forbidden_for_nonexistent_path() {
        let result = validate_repo_path_for_register("/no/such/path/does-not-exist-postlane-test");
        assert!(result.is_err(), "nonexistent path must return an error");
        let (status, _) = result.expect_err("checked above");
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_validate_repo_path_for_register_returns_bad_request_when_no_git_dir() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let result = validate_repo_path_for_register(
            tmp.path().to_str().expect("valid utf8 path"),
        );
        assert!(result.is_err(), "path without .git must return an error");
        let (status, msg) = result.expect_err("checked above");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            msg.contains("git"),
            "error must mention git repository: {}",
            msg
        );
    }

    #[test]
    fn test_validate_repo_path_for_register_returns_bad_request_when_no_postlane_config() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git");
        let result = validate_repo_path_for_register(
            tmp.path().to_str().expect("valid utf8 path"),
        );
        assert!(result.is_err(), "path without .postlane/config.json must return an error");
        let (status, msg) = result.expect_err("checked above");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            msg.contains("config.json"),
            "error must mention config.json: {}",
            msg
        );
    }

    #[test]
    fn test_validate_repo_path_for_register_returns_ok_for_valid_repo() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git");
        std::fs::create_dir_all(tmp.path().join(".postlane")).expect("create .postlane");
        std::fs::write(
            tmp.path().join(".postlane").join("config.json"),
            b"{}",
        ).expect("write config.json");
        let path_str = tmp.path().to_str().expect("valid utf8 path");
        let result = validate_repo_path_for_register(path_str);
        let (canonical_str, name) = result.expect("valid repo must return Ok");
        assert!(!canonical_str.is_empty(), "canonical path must not be empty");
        let expected_name = tmp.path().file_name()
            .and_then(|n| n.to_str())
            .expect("temp dir has a file name");
        assert_eq!(name, expected_name, "name must equal the folder's base name");
    }

    // register_route.rs lines 36-76: register_handler via tower oneshot

    fn make_register_state() -> super::super::ServerState {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            workspaces: vec![],
            repos: vec![],
        }));
        let repos_dir = tempfile::TempDir::new().expect("create repos temp dir");
        let repos_path = repos_dir.path().join("repos.json");
        std::mem::forget(repos_dir);
        super::super::ServerState {
            token: "tok".to_string(),
            repos,
            repos_path,
            activation_tx: None,
            watcher_tx: None,
            app_handle: None,
            projects: Arc::new(tokio::sync::RwLock::new(vec![])),
        }
    }

    #[tokio::test]
    async fn test_register_handler_returns_forbidden_for_nonexistent_path() {
        let state = make_register_state();
        let app = create_router(state);
        let body = r#"{"path":"/no/such/path/does-not-exist-handler-test"}"#;
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/register")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::from(body))
                    .expect("build request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_register_handler_returns_200_and_name_for_valid_repo() {
        let tmp = tempfile::TempDir::new().expect("create repo temp dir");
        std::fs::create_dir_all(tmp.path().join(".git")).expect("create .git");
        std::fs::create_dir_all(tmp.path().join(".postlane")).expect("create .postlane");
        std::fs::write(
            tmp.path().join(".postlane").join("config.json"),
            b"{}",
        ).expect("write config.json");
        let repo_path = std::fs::canonicalize(tmp.path())
            .expect("canonicalize repo path")
            .to_string_lossy()
            .to_string();

        let state = make_register_state();
        let app = create_router(state);
        let body = format!(r#"{{"path":"{}"}}"#, repo_path);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/register")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::from(body))
                    .expect("build request"),
            )
            .await
            .expect("oneshot");
        assert_eq!(response.status(), StatusCode::OK, "valid repo must return 200");
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: serde_json::Value =
            serde_json::from_slice(&body_bytes).expect("parse response JSON");
        assert_eq!(json["success"].as_bool(), Some(true), "success must be true");
        assert!(
            json["name"].as_str().is_some(),
            "name must be present in the response"
        );
    }

    /// HTTP registration must update AppState.repos so get_all_drafts finds the repo.
    #[test]
    fn test_add_legacy_repo_to_app_repos_inserts_entry() {
        let state = make_app_state();
        let repo = make_repo("r1", "/some/repo");
        add_legacy_repo_to_app_repos(&state, &repo);
        let repos = state.lock_repos().expect("lock");
        assert!(repos.repos.iter().any(|r| r.id == "r1"));
    }
}
