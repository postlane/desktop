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
        repos.workspaces.push(entry);
    }

    if let Err(e) = crate::storage::write_repos(&state.repos_path, &repos) {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write repos.json: {:?}", e),
        );
    }

    (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
}
