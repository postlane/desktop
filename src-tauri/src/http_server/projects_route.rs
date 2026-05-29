// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::State,
    response::{IntoResponse, Json},
};
use super::ServerState;

pub(super) async fn projects_handler(State(state): State<ServerState>) -> impl IntoResponse {
    let projects = state.projects.read().await;
    Json(projects.clone())
}
