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
