// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use super::{error_response, SendRequest, SendResponse, ServerState};

async fn validate_registered_path(
    repos: &tokio::sync::MutexGuard<'_, crate::storage::ReposConfig>,
    repo_path: &str,
) -> Result<std::path::PathBuf, (StatusCode, String)> {
    let canonical_path = std::fs::canonicalize(repo_path)
        .map_err(|_| (StatusCode::FORBIDDEN, "Path not found or not accessible".to_string()))?;

    let path_str = canonical_path.to_string_lossy();
    if !repos.repos.iter().any(|r| r.path == path_str.as_ref()) {
        return Err((StatusCode::FORBIDDEN, "Path not registered in repos.json".to_string()));
    }

    Ok(canonical_path)
}

/// Validates that `post_folder` contains no path-traversal characters and that the
/// folder + its `meta.json` exist under `canonical_path/.postlane/posts/`.
fn validate_post_folder(
    canonical_path: &std::path::Path,
    post_folder: &str,
) -> Result<std::path::PathBuf, (StatusCode, String)> {
    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err((StatusCode::BAD_REQUEST, "Invalid post folder: path traversal not permitted".to_string()));
    }

    let post_path = canonical_path.join(".postlane/posts").join(post_folder);
    if !post_path.exists() {
        return Err((StatusCode::BAD_REQUEST, format!("Post folder does not exist: {}", post_folder)));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err((StatusCode::BAD_REQUEST, "meta.json not found in post folder".to_string()));
    }

    Ok(meta_path)
}

/// Reads and parses `meta.json`, stamps `status=sent` and `sent_at`, then writes back atomically.
fn mark_meta_as_sent(meta_path: &std::path::Path) -> Result<(), (StatusCode, String)> {
    let meta_content = std::fs::read_to_string(meta_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read meta.json: {}", e)))?;

    let mut meta: serde_json::Value = serde_json::from_str(&meta_content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse meta.json: {}", e)))?;

    meta["status"] = serde_json::json!("sent");
    meta["sent_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());

    let json_str = serde_json::to_string_pretty(&meta)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to serialize meta.json: {}", e)))?;

    let temp_path = meta_path.with_extension("json.tmp");
    std::fs::write(&temp_path, &json_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write meta.json: {}", e)))?;

    std::fs::rename(&temp_path, meta_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to rename meta.json: {}", e)))?;

    Ok(())
}

pub(super) async fn send_handler(
    State(state): State<ServerState>,
    Json(payload): Json<SendRequest>,
) -> Response {
    let repos = state.repos.lock().await;

    // SECURITY NOTE: TOCTOU race window exists between checking is_registered and
    // using the path (below). Another thread could unregister the repo after our check
    // but before we use the path. However, practical risk is low because:
    // 1. Unregistration requires explicit user action (rare during active operations)
    // 2. Path validation continues to work even if repo is unregistered
    // 3. Worst case: we process a post for an unregistered repo (benign - no security impact)
    // 4. Alternative (holding lock during file I/O) would block all other operations
    // We accept this minimal race window to avoid blocking operations.
    let canonical_path = match validate_registered_path(&repos, &payload.repo_path).await {
        Ok(p) => p,
        Err((status, msg)) => return error_response(status, msg),
    };
    drop(repos);

    let meta_path = match validate_post_folder(&canonical_path, &payload.post_folder) {
        Ok(p) => p,
        Err((status, msg)) => return error_response(status, msg),
    };

    if let Err((status, msg)) = mark_meta_as_sent(&meta_path) {
        return error_response(status, msg);
    }

    (StatusCode::OK, Json(SendResponse { success: true })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::create_router;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn empty_projects() -> Arc<tokio::sync::RwLock<Vec<crate::project_registry::ProjectSummary>>> {
        Arc::new(tokio::sync::RwLock::new(vec![]))
    }

    fn make_state_with_tmp_repo() -> (ServerState, String) {
        let repo_dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = std::fs::canonicalize(repo_dir.path()).expect("temp dir exists");
        let path_str = canonical.to_str().expect("valid utf8").to_string();
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![crate::storage::Repo {
                id: "test-id".to_string(),
                name: "test".to_string(),
                path: path_str.clone(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            }],
        }));
        let repos_dir = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = repos_dir.path().join("repos.json");
        std::mem::forget(repos_dir);
        std::mem::forget(repo_dir);
        (ServerState {
            token: "tok".to_string(),
            repos,
            repos_path,
            activation_tx: None,
            watcher_tx: None,
            projects: empty_projects(),
        }, path_str)
    }

    #[tokio::test]
    async fn test_send_rejects_path_traversal() {
        let (state, path_str) = make_state_with_tmp_repo();
        let app = create_router(state);
        for folder in &["../secret", "sub/dir", "sub\\dir", "../../etc/passwd"] {
            let body = format!(r#"{{"repo_path":"{}","post_folder":"{}"}}"#, path_str, folder);
            let response = app.clone().oneshot(
                axum::http::Request::builder()
                    .method("POST").uri("/send")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::from(body))
                    .unwrap(),
            ).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "folder: {}", folder);
        }
    }

    #[tokio::test]
    async fn test_send_accepts_valid_post_folder_name() {
        let (state, path_str) = make_state_with_tmp_repo();
        let app = create_router(state);
        let body = format!(r#"{{"repo_path":"{}","post_folder":"2024-01-01-launch"}}"#, path_str);
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/send")
                .header("content-type", "application/json")
                .header("authorization", "Bearer tok")
                .body(axum::body::Body::from(body))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body_bytes).unwrap();
        assert!(!body_str.contains("path traversal"), "valid folder name must not trigger traversal error: {}", body_str);
    }
}
