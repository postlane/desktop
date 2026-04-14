// SPDX-License-Identifier: BUSL-1.1

use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
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
    pub path: String,
    pub slug: String,
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

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn send_handler(
    State(_state): State<ServerState>,
    Json(_payload): Json<SendRequest>,
) -> Result<Json<SendResponse>, StatusCode> {
    // TODO: Implement in section 3.6
    Ok(Json(SendResponse { success: true }))
}

async fn register_handler(
    State(_state): State<ServerState>,
    Json(_payload): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, StatusCode> {
    // TODO: Implement in section 3.6
    Ok(Json(RegisterResponse {
        success: true,
        name: "test".to_string(),
    }))
}

pub fn create_router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/send", post(send_handler))
        .route("/register", post(register_handler))
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
        use std::io::Write;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&port_path)?;

        file.write_all(content.as_bytes())?;
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
        use std::io::Write;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)?;

        file.write_all(token.as_bytes())?;
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
}
