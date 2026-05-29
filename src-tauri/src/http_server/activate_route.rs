// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use super::ServerState;

#[derive(Deserialize)]
pub struct ActivateParams {
    pub token: String,
    pub new_link: Option<String>,
}

/// Receives the license token forwarded from the browser after OAuth completes.
/// Sends the token to the activation channel; the receiver in lib.rs validates it
/// against the backend, stores it in the keyring, and emits `license:activated`.
///
/// No bearer auth: the token itself is the credential — validated by the backend.
pub(super) async fn activate_handler(
    State(state): State<ServerState>,
    Query(params): Query<ActivateParams>,
) -> Response {
    if params.token.split('.').count() != 3 {
        return (StatusCode::BAD_REQUEST, "Invalid token format").into_response();
    }

    log::info!("[activate] token received (length={})", params.token.len());

    let new_link = params.new_link.as_deref() == Some("1");
    if let Some(tx) = &state.activation_tx {
        match tx.try_send((params.token, new_link)) {
            Ok(()) => {}
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                log::warn!("[activate] activation channel full");
                return (StatusCode::SERVICE_UNAVAILABLE, "Activation in progress — try again in a moment").into_response();
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                log::error!("[activate] activation channel closed");
                return (StatusCode::SERVICE_UNAVAILABLE, "Activation unavailable").into_response();
            }
        }
    }

    Html(concat!(
        "<!doctype html><html><head><title>Postlane Activated</title></head>",
        r#"<body style="font-family:sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0;background:#f8f9fa">"#,
        r#"<div style="text-align:center;max-width:400px;padding:2rem">"#,
        r#"<h1 style="font-size:1.5rem;color:#1a1a1a">You&#x2019;re signed in</h1>"#,
        r#"<p style="color:#6c757d">Postlane is activated. You can close this tab and return to the app.</p>"#,
        "</div></body></html>",
    )).into_response()
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
    async fn test_activate_returns_200_and_sends_token_to_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, bool)>(1);
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState {
            token: "tok".to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: Some(tx),
            watcher_tx: None,
            projects: empty_projects(),
        };
        let app = create_router(state);
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=header.payload.sig")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let (received_token, _) = rx.try_recv().expect("token should have been sent");
        assert_eq!(received_token, "header.payload.sig");
    }

    #[tokio::test]
    async fn test_activate_passes_new_link_true_when_flag_set() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, bool)>(1);
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState {
            token: "tok".to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: Some(tx),
            watcher_tx: None,
            projects: empty_projects(),
        };
        let app = create_router(state);
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c&new_link=1")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let (token, new_link) = rx.try_recv().expect("should have received");
        assert_eq!(token, "a.b.c");
        assert!(new_link, "new_link should be true when flag is set");
    }

    #[tokio::test]
    async fn test_activate_passes_new_link_false_when_flag_absent() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, bool)>(1);
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState {
            token: "tok".to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: Some(tx),
            watcher_tx: None,
            projects: empty_projects(),
        };
        let app = create_router(state);
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let (token, new_link) = rx.try_recv().expect("should have received");
        assert_eq!(token, "a.b.c");
        assert!(!new_link, "new_link should be false when flag is absent");
    }

    #[tokio::test]
    async fn test_activate_rejects_malformed_token() {
        let app = create_router(make_state("tok"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=only.twosegments")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_activate_returns_200_when_no_channel_configured() {
        let app = create_router(make_state("tok"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_activate_requires_no_bearer_auth() {
        let app = create_router(make_state("tok"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_activate_returns_503_when_channel_is_full() {
        let (tx, _rx) = tokio::sync::mpsc::channel::<(String, bool)>(1);
        tx.try_send(("filler.filler.filler".to_string(), false)).unwrap();
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState {
            token: "tok".to_string(),
            repos: Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
                version: 1, workspaces: vec![], repos: vec![],
            })),
            repos_path,
            activation_tx: Some(tx),
            watcher_tx: None,
            projects: empty_projects(),
        };
        let app = create_router(state);
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_activate_html_body_contains_no_backslash() {
        let app = create_router(make_state("tok"));
        let response = app.oneshot(
            axum::http::Request::builder()
                .uri("/activate?token=a.b.c")
                .body(axum::body::Body::empty()).unwrap(),
        ).await.unwrap();
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body_bytes).unwrap();
        assert!(!body_str.contains('\\'), "HTML response must not contain backslashes, got: {}", body_str);
    }
}
