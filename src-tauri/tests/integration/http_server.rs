// SPDX-License-Identifier: BUSL-1.1

use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_send_with_correct_token_and_registered_path() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();

    let config_dir = repo_path.join(".postlane");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), "{}").unwrap();

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

    let canonical_path = fs::canonicalize(&repo_path).unwrap();

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

    let token = "test-token-12345678901234567890";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(repos_config)),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

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

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_send_with_path_traversal_returns_403() {
    let temp_dir = TempDir::new().unwrap();
    let token = "test-token-12345678901234567890";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

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

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_send_with_wrong_token_returns_401() {
    let temp_dir = TempDir::new().unwrap();
    let token = "correct-token-12345678901234567890";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

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

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_register_with_valid_path() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();
    fs::create_dir_all(repo_path.join(".postlane")).unwrap();
    fs::write(repo_path.join(".postlane/config.json"), "{}").unwrap();

    let canonical_path = fs::canonicalize(&repo_path).unwrap();
    let token = "test-token-12345678901234567890";

    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "path": canonical_path.to_str().unwrap() }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["name"], "test-repo");
}

#[tokio::test]
async fn test_register_with_invalid_path_returns_403() {
    let temp_dir = TempDir::new().unwrap();
    let token = "test-token-12345678901234567890";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "path": "/nonexistent/path/that/does/not/exist" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn test_register_with_wrong_token_returns_401() {
    let temp_dir = TempDir::new().unwrap();
    let token = "correct-token-12345678901234567890";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", "Bearer wrong-token")
        .json(&serde_json::json!({ "path": "/tmp/test" }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn test_register_actually_adds_repo_to_repos_json() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_path).unwrap();
    fs::create_dir_all(repo_path.join(".git")).unwrap();
    fs::create_dir_all(repo_path.join(".postlane")).unwrap();
    fs::write(repo_path.join(".postlane/config.json"), "{}").unwrap();

    let canonical_path = fs::canonicalize(&repo_path).unwrap();
    let token = "test-token-12345678901234567890";
    let repos_arc = Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
        version: 1, repos: vec![],
    }));

    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: repos_arc.clone(),
        repos_path: temp_dir.path().join("repos.json"),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "path": canonical_path.to_str().unwrap() }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["success"], true);
    assert_eq!(body["name"], "test-repo");

    let repos = repos_arc.lock().await;
    assert_eq!(repos.repos.len(), 1);
    assert_eq!(repos.repos[0].name, "test-repo");
    assert_eq!(repos.repos[0].path, canonical_path.to_str().unwrap());
    assert_eq!(repos.repos[0].active, true);
    assert!(!repos.repos[0].id.is_empty(), "ID should be generated");
}

#[tokio::test]
async fn test_register_writes_to_state_repos_path_not_real_postlane_dir() {
    // Verifies that /register writes to ServerState.repos_path, NOT to ~/.postlane/repos.json.
    let temp_dir = TempDir::new().unwrap();
    let isolated_repos_path = temp_dir.path().join("repos.json");

    let repo_dir = temp_dir.path().join("test-repo");
    fs::create_dir_all(&repo_dir).unwrap();
    fs::create_dir_all(repo_dir.join(".git")).unwrap();
    fs::create_dir_all(repo_dir.join(".postlane")).unwrap();
    fs::write(repo_dir.join(".postlane/config.json"), "{}").unwrap();
    let canonical_repo = fs::canonicalize(&repo_dir).unwrap();

    let token = "test-token-isolation-check";
    let server_state = postlane_desktop_lib::http_server::ServerState {
        token: token.to_string(),
        repos: Arc::new(Mutex::new(postlane_desktop_lib::storage::ReposConfig {
            version: 1, repos: vec![],
        })),
        repos_path: isolated_repos_path.clone(),
        activation_tx: None,
        projects: std::sync::Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    let port = postlane_desktop_lib::http_server::start_server(server_state, 0)
        .await
        .unwrap();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://127.0.0.1:{}/register", port))
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({ "path": canonical_repo.to_str().unwrap() }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    assert!(isolated_repos_path.exists(), "repos.json must be written to the isolated path");

    let real_repos_path = postlane_desktop_lib::init::postlane_dir()
        .unwrap()
        .join("repos.json");
    if real_repos_path.exists() {
        let real_content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&real_repos_path).unwrap()).unwrap();
        let real_repos = real_content["repos"].as_array().unwrap();
        assert!(
            real_repos.iter().all(|r| r["name"] != "test-repo"),
            "real repos.json must not contain the test repo"
        );
    }
}
