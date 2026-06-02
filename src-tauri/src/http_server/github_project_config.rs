// SPDX-License-Identifier: BUSL-1.1
// 20.6.8 — GET /github-project-config?org_login=<org>

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use super::{error_response, ServerState};

#[derive(Deserialize)]
pub struct OrgLoginParams {
    pub org_login: String,
}

#[derive(Serialize)]
pub struct GitHubProjectConfigResponse {
    pub project_id: String,
    pub project_name: String,
}

/// Returns the `project_id` and `project_name` for the project whose
/// `provider_org_login` matches the requested `org_login`.
/// Auth required (session token). 404 when no matching project exists.
pub async fn github_project_config_handler(
    State(state): State<ServerState>,
    Query(params): Query<OrgLoginParams>,
) -> Response {
    if params.org_login.trim().is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "org_login is required".to_string());
    }

    let projects = state.projects.read().await;
    let found = projects.iter().find(|p| {
        p.provider_org_login.as_deref() == Some(params.org_login.trim())
    });

    match found {
        Some(p) => (StatusCode::OK, Json(GitHubProjectConfigResponse {
            project_id: p.id.clone(),
            project_name: p.name.clone(),
        })).into_response(),
        None => error_response(StatusCode::NOT_FOUND, "no project for that org".to_string()),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_server::{create_router, ServerState};
    use crate::project_registry::ProjectSummary;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn make_state_with_projects(token: &str, projects: Vec<ProjectSummary>) -> ServerState {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        // Keep TempDir alive by leaking it — test-only, small footprint.
        std::mem::forget(tmp);
        ServerState {
            token: token.to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: None,
            watcher_tx: None,
            app_handle: None,
            projects: Arc::new(tokio::sync::RwLock::new(projects)),
        }
    }

    fn make_project(id: &str, name: &str, org: Option<&str>) -> ProjectSummary {
        ProjectSummary {
            id: id.to_string(),
            name: name.to_string(),
            workspace_type: "organization".to_string(),
            tier: "free".to_string(),
            billing_active: true,
            is_owner: true,
            provider_org_login: org.map(|s| s.to_string()),
        }
    }

    #[tokio::test]
    async fn test_returns_200_with_project_id_for_known_org() {
        let state = make_state_with_projects("tok", vec![
            make_project("proj-uuid-1", "Acme", Some("acme-org")),
        ]);
        let app = create_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/github-project-config?org_login=acme-org")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["project_id"].as_str(), Some("proj-uuid-1"));
        assert_eq!(body["project_name"].as_str(), Some("Acme"));
    }

    #[tokio::test]
    async fn test_returns_404_for_unknown_org() {
        let state = make_state_with_projects("tok", vec![
            make_project("proj-1", "Other", Some("other-org")),
        ]);
        let app = create_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/github-project-config?org_login=unknown-org")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_requires_auth_returns_401_without_token() {
        let state = make_state_with_projects("tok", vec![
            make_project("proj-1", "Acme", Some("acme-org")),
        ]);
        let app = create_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/github-project-config?org_login=acme-org")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_returns_400_for_empty_org_login() {
        let state = make_state_with_projects("tok", vec![]);
        let app = create_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/github-project-config?org_login=")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_returns_404_when_project_has_no_org_login() {
        let state = make_state_with_projects("tok", vec![
            make_project("proj-1", "Personal", None),
        ]);
        let app = create_router(state);
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/github-project-config?org_login=personal")
                    .header("authorization", "Bearer tok")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
