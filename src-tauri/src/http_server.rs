// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct ServerState {
    pub token: String,
    pub repos: Arc<tokio::sync::Mutex<crate::storage::ReposConfig>>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Deserialize)]
pub struct SendRequest {
    pub repo_path: String,
    pub post_folder: String,
}

#[derive(Serialize)]
pub struct SendResponse {
    pub success: bool,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub path: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub success: bool,
    pub name: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Auth middleware - validates Bearer token from Authorization header
async fn auth_middleware(
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

    if token != state.token {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn send_handler(
    State(state): State<ServerState>,
    Json(payload): Json<SendRequest>,
) -> Response {
    // Validate path is registered
    let repos = state.repos.lock().await;

    // Canonicalize the path
    let canonical_path = match std::fs::canonicalize(&payload.repo_path) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Path not found or not accessible".to_string(),
                }),
            )
                .into_response();
        }
    };

    let path_str = canonical_path.to_string_lossy().to_string();
    let is_registered = repos.repos.iter().any(|r| r.path == path_str);

    if !is_registered {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Path not registered in repos.json".to_string(),
            }),
        )
            .into_response();
    }

    // Drop the lock before doing file operations
    drop(repos);

    // Validate post folder exists and has meta.json
    let post_path = canonical_path
        .join(".postlane/posts")
        .join(&payload.post_folder);

    if !post_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Post folder does not exist: {}", payload.post_folder),
            }),
        )
            .into_response();
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "meta.json not found in post folder".to_string(),
            }),
        )
            .into_response();
    }

    // Read and update meta.json (stub send - always succeeds in M3)
    let meta_content = match std::fs::read_to_string(&meta_path) {
        Ok(content) => content,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read meta.json: {}", e),
                }),
            )
                .into_response();
        }
    };

    let mut meta: serde_json::Value = match serde_json::from_str(&meta_content) {
        Ok(m) => m,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to parse meta.json: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Update status to sent (stub - real scheduler integration in M4)
    meta["status"] = serde_json::json!("sent");
    meta["sent_at"] = serde_json::json!(chrono::Utc::now().to_rfc3339());

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    if let Err(e) = std::fs::write(&temp_path, serde_json::to_string_pretty(&meta).unwrap()) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to write meta.json: {}", e),
            }),
        )
            .into_response();
    }

    if let Err(e) = std::fs::rename(&temp_path, &meta_path) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to rename meta.json: {}", e),
            }),
        )
            .into_response();
    }

    (StatusCode::OK, Json(SendResponse { success: true })).into_response()
}

async fn register_handler(
    State(_state): State<ServerState>,
    Json(payload): Json<RegisterRequest>,
) -> Response {
    // Canonicalize the path
    let canonical_path = match std::fs::canonicalize(&payload.path) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: "Path not found or not accessible".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Check .git/ exists
    let git_path = canonical_path.join(".git");
    if !git_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Not a git repository".to_string(),
            }),
        )
            .into_response();
    }

    // Check .postlane/config.json exists
    let config_path = canonical_path.join(".postlane").join("config.json");
    if !config_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: ".postlane/config.json not found - run postlane init first".to_string(),
            }),
        )
            .into_response();
    }

    // TODO: Implement actual registration logic in section 3.6
    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    (
        StatusCode::OK,
        Json(RegisterResponse {
            success: true,
            name,
        }),
    )
        .into_response()
}

pub fn create_router(state: ServerState) -> Router {
    let protected_routes = Router::new()
        .route("/send", post(send_handler))
        .route("/register", post(register_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/health", get(health_handler))
        .merge(protected_routes)
        .with_state(state)
}

/// Starts the HTTP server on 127.0.0.1:47312 (or fallback port)
/// Returns the bound port number
pub async fn start_server(
    state: ServerState,
    preferred_port: u16,
) -> Result<u16, std::io::Error> {
    let app = create_router(state);

    // Try preferred port first, then fallback to any available port
    let addr = SocketAddr::from(([127, 0, 0, 1], preferred_port));

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let bound_port = listener.local_addr()?.port();
            tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Ok(bound_port)
        }
        Err(_) => {
            // Fallback: bind to any available port
            let fallback_addr = SocketAddr::from(([127, 0, 0, 1], 0));
            let listener = TcpListener::bind(fallback_addr).await?;
            let bound_port = listener.local_addr()?.port();
            tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Ok(bound_port)
        }
    }
}

/// Writes the port file to ~/.postlane/port with 0600 permissions
pub fn write_port_file(port: u16) -> std::io::Result<()> {
    let port_path = crate::init::postlane_dir().join("port");
    let content = port.to_string();

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&port_path)?;

        file.write_all(content.as_bytes())?;

        // Explicitly set permissions (handles case where file already existed)
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&port_path, perms)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&port_path, content)?;
    }

    Ok(())
}

/// Generates a random session token and writes it to ~/.postlane/session.token with 0600 permissions
pub fn generate_and_write_token() -> std::io::Result<String> {
    use rand::Rng;

    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    let token_path = crate::init::postlane_dir().join("session.token");

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)?;

        file.write_all(token.as_bytes())?;

        // Explicitly set permissions (handles case where file already existed)
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&token_path, perms)?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&token_path, &token)?;
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint_returns_ok() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token".to_string(),
            repos,
        };

        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_server_binds_to_preferred_port() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token".to_string(),
            repos,
        };

        // Use a high port unlikely to be in use
        let test_port = 57312u16;
        let bound_port = start_server(state, test_port).await.unwrap();

        assert_eq!(bound_port, test_port);
    }

    #[test]
    fn test_write_port_file() {
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let port = 47312u16;
        write_port_file(port).expect("Failed to write port file");

        let port_path = crate::init::postlane_dir().join("port");
        assert!(port_path.exists());

        let content = fs::read_to_string(&port_path).expect("Failed to read port file");
        assert_eq!(content, "47312");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&port_path).expect("Failed to get metadata");
            let permissions = metadata.permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }

        // Cleanup
        let _ = fs::remove_file(&port_path);
    }

    #[test]
    fn test_generate_and_write_token() {
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let token = generate_and_write_token().expect("Failed to generate token");

        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_alphanumeric()));

        let token_path = crate::init::postlane_dir().join("session.token");
        assert!(token_path.exists());

        let content = fs::read_to_string(&token_path).expect("Failed to read token file");
        assert_eq!(content, token);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&token_path).expect("Failed to get metadata");
            let permissions = metadata.permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }

        // Cleanup
        let _ = fs::remove_file(&token_path);
    }

    #[tokio::test]
    async fn test_send_endpoint_requires_auth() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token-123".to_string(),
            repos,
        };

        let app = create_router(state);

        // Request without auth header
        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/send")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(
                        r#"{"repo_path": "/test", "post_folder": "test-post"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_send_endpoint_rejects_wrong_token() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "correct-token".to_string(),
            repos,
        };

        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/send")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer wrong-token")
                    .body(axum::body::Body::from(
                        r#"{"repo_path": "/test", "post_folder": "test-post"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_send_endpoint_rejects_unregistered_path() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token".to_string(),
            repos,
        };

        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/send")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer test-token")
                    .body(axum::body::Body::from(
                        r#"{"repo_path": "/nonexistent", "post_folder": "test-post"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn test_register_endpoint_requires_auth() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token".to_string(),
            repos,
        };

        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/register")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(r#"{"path": "/test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_health_endpoint_no_auth_required() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        }));

        let state = ServerState {
            token: "test-token".to_string(),
            repos,
        };

        let app = create_router(state);

        let response = app
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
