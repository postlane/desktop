// SPDX-License-Identifier: BUSL-1.1

use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;
use std::sync::OnceLock;

// Mutex to ensure tests that use ~/.postlane directory run sequentially
// This prevents race conditions when tests run in parallel
static POSTLANE_DIR_MUTEX: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

fn get_postlane_dir_lock() -> &'static std::sync::Mutex<()> {
    POSTLANE_DIR_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}

#[tokio::test]
async fn test_send_with_correct_token_and_registered_path() {
    // Setup: Create temp repo with config
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();

    let config_dir = repo_path.join(".postlane");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), "{}").unwrap();

    // Create post folder with meta.json
    let posts_dir = config_dir.join("posts");
    fs::create_dir_all(&posts_dir).unwrap();
    let post_dir = posts_dir.join("test-post");
    fs::create_dir_all(&post_dir).unwrap();

    let meta = serde_json::json!({
        "status": "ready",
        "platforms": ["x"],
        "schedule": null,
        "trigger": null,
        "scheduler_ids": null,
        "platform_results": null,
        "error": null,
        "image_url": null,
        "image_source": null,
        "image_attribution": null,
        "llm_model": null,
        "created_at": "2024-01-01T00:00:00Z",
        "sent_at": null
    });
    fs::write(post_dir.join("meta.json"), serde_json::to_string(&meta).unwrap()).unwrap();

    // Canonicalize path
    let canonical_path = fs::canonicalize(&repo_path).unwrap();

    // Setup repos config with registered path
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![postlane_desktop_lib::storage::Repo {
            id: "test-id".to_string(),
            name: "Test Repo".to_string(),
            path: canonical_path.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }],
    };

    // Generate token
    let token = "test-token-12345678901234567890";

    // Start server
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /send with correct token and registered path
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/send", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "repo_path": canonical_path.to_str().unwrap(),
            "post_folder": "test-post"
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 200 with success: true
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_send_with_path_traversal_returns_403() {
    // Setup: Create repos config
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "test-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /send with path traversal attempt
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/send", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "repo_path": "../../etc/passwd",
            "post_folder": "test"
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 403 (path not in repos.json)
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_send_with_wrong_token_returns_401() {
    // Setup
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "correct-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /send with wrong token
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/send", port))
        .header("Authorization", "Bearer wrong-token")
        .json(&serde_json::json!({
            "repo_path": "/tmp/test",
            "post_folder": "test"
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 401
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_register_with_valid_path() {
    // Acquire lock to prevent race conditions with other tests using ~/.postlane
    let _lock = get_postlane_dir_lock().lock().unwrap();

    // Setup: Create temp repo with .git and config
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();

    let config_dir = repo_path.join(".postlane");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), "{}").unwrap();

    let canonical_path = fs::canonicalize(&repo_path).unwrap();

    // Initialize postlane directory (needed for /register endpoint to write repos.json)
    postlane_desktop_lib::init::init_postlane_dir().expect("Failed to init postlane dir");

    // Setup server
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "test-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /register with valid path
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "path": canonical_path.to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 200 with success: true and name
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["name"], "test-repo");
}

#[tokio::test]
async fn test_register_with_invalid_path_returns_403() {
    // Setup
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "test-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /register with path that doesn't exist
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "path": "/nonexistent/path/that/does/not/exist"
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 403 (path not found or not accessible)
    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_register_with_wrong_token_returns_401() {
    // Setup
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "correct-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc,
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /register with wrong token
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", "Bearer wrong-token")
        .json(&serde_json::json!({
            "path": "/tmp/test"
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 401
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_register_actually_adds_repo_to_repos_json() {
    // Acquire lock to prevent race conditions with other tests using ~/.postlane
    let _lock = get_postlane_dir_lock().lock().unwrap();

    // Setup: Create temp repo with .git and config
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();

    let config_dir = repo_path.join(".postlane");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), "{}").unwrap();

    let canonical_path = fs::canonicalize(&repo_path).unwrap();

    // Initialize postlane directory (needed for /register endpoint to write repos.json)
    postlane_desktop_lib::init::init_postlane_dir().expect("Failed to init postlane dir");

    // Setup server with empty repos
    let repos_config = postlane_desktop_lib::storage::ReposConfig {
        version: 1,
        repos: vec![],
    };

    let token = "test-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(repos_config));
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc.clone(),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    // Test: POST /register
    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "path": canonical_path.to_str().unwrap()
        }))
        .send()
        .await
        .unwrap();

    // Assert: Should return 200
    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["name"], "test-repo");

    // Verify: Repo was actually added to repos_arc
    let repos = repos_arc.lock().await;
    assert_eq!(repos.repos.len(), 1);
    assert_eq!(repos.repos[0].name, "test-repo");
    assert_eq!(repos.repos[0].path, canonical_path.to_str().unwrap());
    assert_eq!(repos.repos[0].active, true);
    assert!(!repos.repos[0].id.is_empty(), "ID should be generated");
}
