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
        path: canonical_str,
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut repos = state.repos.lock().await;
    repos.repos.push(repo);

    if let Err(e) = crate::storage::write_repos(&state.repos_path, &repos) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write repos.json: {:?}", e),
        );
    }

    (StatusCode::OK, Json(RegisterResponse { success: true, name })).into_response()
}
