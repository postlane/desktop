// SPDX-License-Identifier: BUSL-1.1

pub mod github_project_config;
pub mod routes;
mod activate_route;
mod projects_route;
mod register_route;
mod register_workspace_route;
mod send_route;

use axum::{
    http::{HeaderName, HeaderValue, Response},
    middleware::{self, Next},
    routing::{get, post},
    Router,
};
use crate::project_registry::ProjectSummary;

async fn add_cors_null_origin(
    request: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Response<axum::body::Body> {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        HeaderName::from_static("access-control-allow-origin"),
        HeaderValue::from_static("null"),
    );
    response
}
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Clone)]
pub struct ServerState {
    pub token: String,
    pub repos: Arc<tokio::sync::Mutex<crate::storage::ReposConfig>>,
    /// Path to repos.json. Injected so tests can use a temp path instead of ~/.postlane/repos.json.
    pub repos_path: std::path::PathBuf,
    /// Sends (token, new_link) to the activation receiver task.
    /// `None` in tests and before the server is fully initialised.
    pub activation_tx: Option<tokio::sync::mpsc::Sender<(String, bool)>>,
    /// Sends (repo_id, repo_path) when a repo is registered mid-session so the
    /// Tauri side can start a file watcher immediately. `None` in tests.
    pub watcher_tx: Option<tokio::sync::mpsc::Sender<(String, String)>>,
    /// Cached project list used by `/github-project-config`. Updated on sign-in.
    pub projects: Arc<tokio::sync::RwLock<Vec<ProjectSummary>>>,
    /// Used to update AppState.repos in-memory when a workspace or repo is registered
    /// mid-session so `get_all_drafts` finds it without an app restart. `None` in tests.
    pub app_handle: Option<tauri::AppHandle>,
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

pub(crate) fn error_response(status: axum::http::StatusCode, message: String) -> axum::response::Response {
    use axum::response::IntoResponse;
    (status, axum::response::Json(ErrorResponse { error: message })).into_response()
}

pub fn create_router(state: ServerState) -> Router {
    let protected_routes = Router::new()
        .route("/send", post(send_route::send_handler))
        .route("/register", post(register_route::register_handler))
        .route("/register-workspace", post(register_workspace_route::register_workspace_handler))
        .route("/api/v1/projects", get(projects_route::projects_handler))
        .route("/github-project-config", get(github_project_config::github_project_config_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            routes::auth_middleware,
        ));

    let activation_routes = Router::new()
        .route("/activate", get(activate_route::activate_handler));

    // Restrict cross-origin requests to the null origin only.
    // This blocks browser extensions from calling the local API while still
    // allowing requests from the Tauri webview (which has no origin).
    // /activate is intentionally open to browser navigations (no CORS restriction needed
    // since it's top-level navigation, not XHR) — token validity is the security check.
    Router::new()
        .route("/health", get(routes::health_handler))
        .merge(activation_routes)
        .merge(protected_routes)
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)) // 1MB limit
        .layer(middleware::from_fn(add_cors_null_origin))
        .with_state(state)
}

/// Binds a TCP listener on 127.0.0.1 synchronously.
/// Tries `preferred_port` first; falls back to OS-assigned port if unavailable.
/// Sets non-blocking mode so the listener can be handed to tokio without blocking.
pub fn bind_listener(preferred_port: u16) -> Result<std::net::TcpListener, std::io::Error> {
    let listener = std::net::TcpListener::bind(format!("127.0.0.1:{}", preferred_port))
        .or_else(|_| std::net::TcpListener::bind("127.0.0.1:0"))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Starts serving on a pre-bound listener.
/// Converts the `std::net` listener to tokio and spawns the axum server.
/// Starts serving on a pre-bound listener.
/// Converts the `std::net` listener to tokio and spawns the axum server.
/// Returns an error if the listener is not bound to the loopback interface.
pub async fn serve_on_listener(
    state: ServerState,
    listener: std::net::TcpListener,
) -> Result<(), std::io::Error> {
    let local_addr = listener.local_addr()?;
    if !local_addr.ip().is_loopback() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("HTTP server must bind to loopback only, got {}", local_addr.ip()),
        ));
    }
    let tokio_listener = tokio::net::TcpListener::from_std(listener)?;
    let app = create_router(state);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(tokio_listener, app).await {
            log::error!("HTTP server error: {}", e);
        }
    });
    Ok(())
}

/// Starts the HTTP server on 127.0.0.1:47312 (or fallback port).
/// Returns the bound port number.
pub async fn start_server(
    state: ServerState,
    preferred_port: u16,
) -> Result<u16, std::io::Error> {
    let app = create_router(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], preferred_port));

    match TcpListener::bind(addr).await {
        Ok(listener) => {
            let bound_port = listener.local_addr()?.port();
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    log::error!("HTTP server error: {}", e);
                }
            });
            Ok(bound_port)
        }
        Err(_) => {
            let fallback_addr = SocketAddr::from(([127, 0, 0, 1], 0));
            let listener = TcpListener::bind(fallback_addr).await?;
            let bound_port = listener.local_addr()?.port();
            tokio::spawn(async move {
                if let Err(e) = axum::serve(listener, app).await {
                    log::error!("HTTP server error: {}", e);
                }
            });
            Ok(bound_port)
        }
    }
}

/// Writes the port file to ~/.postlane/port with 0600 permissions.
pub fn write_port_file(port: u16) -> Result<(), String> {
    let port_path = crate::init::postlane_dir()?.join("port");
    let content = port.to_string();

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&port_path)
            .map_err(|e| format!("Failed to open port file: {}", e))?;

        file.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write port file: {}", e))?;

        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&port_path, perms)
            .map_err(|e| format!("Failed to set port file permissions: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&port_path, content)
            .map_err(|e| format!("Failed to write port file: {}", e))?;
    }

    Ok(())
}

/// Writes `token` to `~/.postlane/local.token` with 0600 permissions.
/// The CLI reads this file for workspace registration (22.4.8a).
pub fn write_local_token(token: &str) -> Result<(), String> {
    let token_path = crate::init::postlane_dir()?.join("local.token");

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)
            .map_err(|e| format!("Failed to open local.token: {}", e))?;

        file.write_all(token.as_bytes())
            .map_err(|e| format!("Failed to write local.token: {}", e))?;

        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&token_path, perms)
            .map_err(|e| format!("Failed to set local.token permissions: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&token_path, token)
            .map_err(|e| format!("Failed to write local.token: {}", e))?;
    }

    Ok(())
}

/// Generates a random session token and writes it to ~/.postlane/session.token with 0600 permissions.
/// Token is transmitted over HTTP on loopback — this is safe because 127.0.0.1 is not routable and
/// cannot be intercepted from the network (RFC 5735 §3). No TLS is needed for loopback-only IPC.
pub fn generate_and_write_token() -> Result<String, String> {
    use rand::Rng;

    let token: String = rand::rng()
        .sample_iter(rand::distr::Alphanumeric)
        .take(43)
        .map(char::from)
        .collect();

    let token_path = crate::init::postlane_dir()?.join("session.token");

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&token_path)
            .map_err(|e| format!("Failed to open token file: {}", e))?;

        file.write_all(token.as_bytes())
            .map_err(|e| format!("Failed to write token file: {}", e))?;

        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&token_path, perms)
            .map_err(|e| format!("Failed to set token file permissions: {}", e))?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&token_path, &token)
            .map_err(|e| format!("Failed to write token file: {}", e))?;
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// CORS header must be `null` on health endpoint to block browser-extension abuse.
    #[tokio::test]
    async fn test_health_cors_header_restricts_origin() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![],
        }));
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState { token: "test-token".to_string(), repos, repos_path, activation_tx: None, watcher_tx: None, app_handle: None, projects: Arc::new(tokio::sync::RwLock::new(vec![])) };
        let port = start_server(state, 0).await.expect("server start failed");
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .header("Origin", "https://evil.example.com")
            .send()
            .await
            .expect("request failed");
        let cors = resp.headers().get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cors, "null", "CORS must restrict to null origin to block browser extensions");
    }

    /// HTTP is used without TLS. This is safe only because the server binds
    /// exclusively to 127.0.0.1 (loopback). Loopback traffic never leaves the host
    /// and is not accessible from the network (RFC 5735 §3). No TLS is needed.
    #[tokio::test]
    async fn test_server_only_accessible_on_loopback() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![],
        }));
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState { token: "tok".to_string(), repos, repos_path, activation_tx: None, watcher_tx: None, app_handle: None, projects: Arc::new(tokio::sync::RwLock::new(vec![])) };
        let port = start_server(state, 0).await.expect("server start failed");
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{}/health", port))
            .send()
            .await
            .expect("loopback request failed");
        assert!(resp.status().is_success(), "server must be reachable on loopback");
        let local_addr: std::net::SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        assert!(local_addr.ip().is_loopback(), "bound address must be loopback");
    }

    #[tokio::test]
    async fn test_server_binds_to_preferred_port() {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![],
        }));
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = ServerState { token: "test-token".to_string(), repos, repos_path, activation_tx: None, watcher_tx: None, app_handle: None, projects: Arc::new(tokio::sync::RwLock::new(vec![])) };
        let test_port = 57312u16;
        let bound_port = start_server(state, test_port).await.unwrap();
        assert_eq!(bound_port, test_port);
    }

    #[test]
    fn test_write_port_file() {
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let port = 47312u16;
        write_port_file(port).expect("Failed to write port file");
        let port_path = crate::init::postlane_dir()
            .expect("Failed to get postlane dir")
            .join("port");
        assert!(port_path.exists());
        let content = fs::read_to_string(&port_path).expect("Failed to read port file");
        assert_eq!(content, "47312");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&port_path).expect("Failed to get metadata");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        let _ = fs::remove_file(&port_path);
    }

    /// 22.4.8a: write_local_token writes local.token with 0600 permissions.
    #[test]
    fn test_write_local_token() {
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        write_local_token("test-local-token-value").expect("Failed to write local.token");
        let token_path = crate::init::postlane_dir()
            .expect("Failed to get postlane dir")
            .join("local.token");
        assert!(token_path.exists());
        let content = fs::read_to_string(&token_path).expect("Failed to read local.token");
        assert_eq!(content, "test-local-token-value");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&token_path).expect("Failed to get metadata");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        let _ = fs::remove_file(&token_path);
    }

    #[test]
    fn test_generate_and_write_token() {
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let token = generate_and_write_token().expect("Failed to generate token");
        assert_eq!(token.len(), 43);
        assert!(token.chars().all(|c| c.is_alphanumeric()));
        let token_path = crate::init::postlane_dir()
            .expect("Failed to get postlane dir")
            .join("session.token");
        assert!(token_path.exists());
        let content = fs::read_to_string(&token_path).expect("Failed to read token file");
        assert_eq!(content, token);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&token_path).expect("Failed to get metadata");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        let _ = fs::remove_file(&token_path);
    }

    #[test]
    fn test_bind_listener_binds_to_loopback() {
        let listener = bind_listener(0).expect("bind_listener failed");
        let addr = listener.local_addr().expect("local_addr failed");
        assert!(addr.ip().is_loopback(), "must bind to 127.0.0.1");
        assert_ne!(addr.port(), 0);
    }

    fn make_minimal_state(repos_path: std::path::PathBuf) -> ServerState {
        let repos = Arc::new(tokio::sync::Mutex::new(crate::storage::ReposConfig {
            version: 1,
            workspaces: vec![],
            repos: vec![],
        }));
        ServerState {
            token: "tok".to_string(),
            repos,
            repos_path,
            activation_tx: None,
            watcher_tx: None,
            app_handle: None,
            projects: Arc::new(tokio::sync::RwLock::new(vec![])),
        }
    }

    // mod.rs lines 130, 134-148: serve_on_listener rejects non-loopback addresses.
    // Binding to 0.0.0.0 (wildcard) is not loopback — the function must reject it.
    #[tokio::test]
    async fn test_serve_on_listener_rejects_non_loopback_address() {
        let listener = std::net::TcpListener::bind("0.0.0.0:0")
            .expect("bind wildcard listener");
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = make_minimal_state(repos_path);
        let result = serve_on_listener(state, listener).await;
        assert!(result.is_err(), "non-loopback listener must be rejected");
        let err = result.expect_err("checked above");
        assert!(
            err.to_string().contains("loopback"),
            "error must mention loopback restriction: {}",
            err
        );
    }

    // mod.rs lines 130, 134-148: serve_on_listener accepts a loopback listener.
    #[tokio::test]
    async fn test_serve_on_listener_accepts_loopback_address() {
        // bind_listener already sets non-blocking, which tokio::net::TcpListener::from_std requires.
        let listener = bind_listener(0).expect("bind loopback listener");
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        std::mem::forget(tmp);
        let state = make_minimal_state(repos_path);
        let result = serve_on_listener(state, listener).await;
        assert!(result.is_ok(), "loopback listener must be accepted: {:?}", result);
    }
}
