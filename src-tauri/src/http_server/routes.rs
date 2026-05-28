// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;
use super::ServerState;

pub(super) async fn auth_middleware(
    State(state): State<ServerState>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if token.as_bytes().ct_eq(state.token.as_bytes()).unwrap_u8() != 1 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

pub(super) async fn health_handler() -> StatusCode {
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::super::{create_router, ServerState};
    use axum::http::StatusCode;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn empty_projects() -> Arc<tokio::sync::RwLock<Vec<crate::project_registry::ProjectSummary>>> {
        Arc::new(tokio::sync::RwLock::new(vec![]))
    }

    fn make_state(token: &str) -> ServerState {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        ServerState {
            token: token.to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: None,
            watcher_tx: None,
            projects: empty_projects(),
        }
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_no_content() {
        let app = create_router(make_state("test-token"));
        let response = app
            .oneshot(axum::http::Request::builder().uri("/health").body(axum::body::Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_health_endpoint_no_auth_required() {
        let app = create_router(make_state("test-token"));
        let response = app
            .oneshot(axum::http::Request::builder().uri("/health").body(axum::body::Body::empty()).unwrap())
            .await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_send_endpoint_requires_auth() {
        let app = create_router(make_state("test-token-123"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/send")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"repo_path": "/test", "post_folder": "test-post"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_send_endpoint_rejects_wrong_token() {
        let app = create_router(make_state("correct-token"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/send")
                .header("content-type", "application/json")
                .header("authorization", "Bearer wrong-token")
                .body(axum::body::Body::from(r#"{"repo_path": "/test", "post_folder": "test-post"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_register_endpoint_requires_auth() {
        let app = create_router(make_state("test-token"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(r#"{"path": "/test"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// Security: token comparison must use constant-time equality to prevent timing attacks.
    /// This test verifies both correct rejection and correct acceptance behaviour.
    #[tokio::test]
    async fn test_timing_safe_token_comparison_rejects_wrong_token() {
        let correct = "a".repeat(64);
        let wrong_last = format!("{}b", "a".repeat(63));
        let app = create_router(make_state(&correct));
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/send")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", wrong_last))
                .body(axum::body::Body::from(r#"{"repo_path": "/test", "post_folder": "test-post"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_timing_safe_token_comparison_accepts_correct_token() {
        let correct = "a".repeat(64);
        let app = create_router(make_state(&correct));
        let response = app.oneshot(
            axum::http::Request::builder()
                .method("POST").uri("/send")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", correct))
                .body(axum::body::Body::from(r#"{"repo_path": "/nonexistent", "post_folder": "test-post"}"#))
                .unwrap(),
        ).await.unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED, "correct token must pass auth middleware");
    }
}
