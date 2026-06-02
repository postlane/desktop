// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use super::{error_response, ServerState};

#[derive(Deserialize)]
pub(super) struct RegisterWorkspaceRequest {
    pub workspace_path: String,
    pub name: String,
    pub project_id: String,
}

pub(super) async fn register_workspace_handler(
    State(state): State<ServerState>,
    Json(payload): Json<RegisterWorkspaceRequest>,
) -> Response {
    if payload.project_id.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "project_id is required".to_string());
    }
    if payload.workspace_path.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "workspace_path is required".to_string());
    }

    let entry = crate::workspace_entry::WorkspaceEntry {
        id: payload.project_id.clone(),
        name: payload.name.clone(),
        workspace_path: payload.workspace_path.clone(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut repos = state.repos.lock().await;
    if !repos.workspaces.iter().any(|w| w.id == payload.project_id) {
        repos.workspaces.push(entry.clone());
    }

    if let Err(e) = crate::storage::write_repos(&state.repos_path, &repos) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write repos.json: {:?}", e),
        );
    }
    drop(repos);

    if let Some(tx) = &state.watcher_tx {
        let _ = tx.try_send((payload.project_id.clone(), payload.workspace_path.clone()));
    }

    if let Some(app_handle) = &state.app_handle {
        use tauri::Manager;
        if let Some(app_state) = app_handle.try_state::<crate::app_state::AppState>() {
            add_workspace_to_app_repos(&app_state, &entry);
        }
    }

    (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
}

/// Updates the in-memory AppState repos so `get_all_drafts` finds the workspace
/// immediately without requiring an app restart.
pub(crate) fn add_workspace_to_app_repos(
    app_state: &crate::app_state::AppState,
    entry: &crate::workspace_entry::WorkspaceEntry,
) {
    if let Ok(mut repos) = app_state.lock_repos() {
        if !repos.workspaces.iter().any(|w| w.id == entry.id) {
            repos.workspaces.push(entry.clone());
        }
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

    fn make_entry(id: &str) -> crate::workspace_entry::WorkspaceEntry {
        crate::workspace_entry::WorkspaceEntry {
            id: id.to_string(),
            name: "ws".to_string(),
            workspace_path: "/some/path".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    /// HTTP registration must update AppState.repos so get_all_drafts finds the workspace.
    #[test]
    fn test_add_workspace_to_app_repos_inserts_entry() {
        let state = make_app_state();
        let entry = make_entry("proj-abc");
        add_workspace_to_app_repos(&state, &entry);
        let repos = state.lock_repos().expect("lock");
        assert!(repos.workspaces.iter().any(|w| w.id == "proj-abc"));
    }

    #[test]
    fn test_add_workspace_to_app_repos_is_idempotent() {
        let state = make_app_state();
        let entry = make_entry("proj-abc");
        add_workspace_to_app_repos(&state, &entry);
        add_workspace_to_app_repos(&state, &entry);
        let repos = state.lock_repos().expect("lock");
        assert_eq!(repos.workspaces.iter().filter(|w| w.id == "proj-abc").count(), 1);
    }
}
