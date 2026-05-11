// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::init::atomic_write;
use crate::license::POSTLANE_API_BASE;
use crate::project_validation::{reject_if_symlink, validate_project_id};
use crate::providers::scheduling::build_client;
use crate::storage::ReposConfig;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use tauri::{Emitter, State};
use tauri_plugin_keyring::KeyringExt;

// ── Public types ─────────────────────────────────────────────────────────────

/// Lightweight project summary returned by `list_projects`.
#[derive(serde::Serialize, Deserialize, Clone, Debug)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub workspace_type: String,
    pub tier: String,
    pub billing_active: bool,
    pub is_owner: bool,
}

#[derive(Debug, PartialEq)]
pub enum ProjectStatus {
    Owned,
    NotFound,
    Offline,
}

#[derive(Debug, PartialEq)]
pub enum BillingGate {
    Free,
    None,
    Offline,
}

#[derive(Debug)]
pub enum CreateProjectError {
    InvalidName(String),
    InvalidWorkspaceType(String),
    NoFreeSlot,
    NoLicenseToken,
    Backend(String),
}

impl std::fmt::Display for CreateProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(msg) => write!(f, "Invalid project name: {}", msg),
            Self::InvalidWorkspaceType(t) => write!(f, "Invalid workspace type: '{}'. Must be personal, organization, or client", t),
            Self::NoFreeSlot => write!(f, "No free project slot. Subscribe at postlane.dev/billing"),
            Self::NoLicenseToken => write!(f, "No license token — sign in at postlane.dev/login"),
            Self::Backend(msg) => write!(f, "Backend error: {}", msg),
        }
    }
}

/// Canonical error returned to the frontend when any web API command receives HTTP 401.
/// The TypeScript `src/ipc/invoke.ts` wrapper detects this string and navigates to
/// AccountSettingsView. All web API commands must use this constant — never return a
/// free-form "401" string that the wrapper won't detect.
///
/// Auth token pattern (M19):
///   - Token stored in OS keyring under service "postlane", account "license"
///   - Retrieved via `app.keyring().get_password("postlane", "license")`
///   - Passed as `Bearer {token}` via `client.bearer_auth(token)`
///   - On HTTP 401: return `Err(SESSION_EXPIRED_ERROR.to_string())`
///   - No automatic refresh in v1 (no refresh token flow)
pub const SESSION_EXPIRED_ERROR: &str = "session_expired";

fn require_license_token(opt: Option<String>) -> Result<String, String> {
    opt.ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())
}

// ── Pure functions (injectable deps for testability) ─────────────────────────

#[derive(Deserialize)]
struct ProjectStatusResponse {
    status: String,
}

/// Calls `GET {base_url}/v1/projects/{project_id}` and maps the response to a `ProjectStatus`.
pub async fn check_project_status_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> ProjectStatus {
    if validate_project_id(project_id).is_err() {
        return ProjectStatus::NotFound;
    }
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let resp = client.get(&url).bearer_auth(token).send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            match r.json::<ProjectStatusResponse>().await {
                Ok(body) if body.status == "owned" => ProjectStatus::Owned,
                _ => ProjectStatus::NotFound,
            }
        }
        Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 404 => ProjectStatus::NotFound,
        _ => ProjectStatus::Offline,
    }
}

#[derive(Deserialize)]
struct BillingGateResponse {
    slot: String,
}

/// Calls `GET {base_url}/v1/projects/gate` and maps the response to a `BillingGate`.
pub async fn check_billing_gate_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> BillingGate {
    let url = format!("{}/v1/projects/gate", base_url);
    let resp = client.get(&url).bearer_auth(token).send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            match r.json::<BillingGateResponse>().await {
                Ok(body) if body.slot == "free" => BillingGate::Free,
                Ok(_) => BillingGate::None,
                Err(_) => BillingGate::Offline,
            }
        }
        _ => BillingGate::Offline,
    }
}

#[derive(Deserialize)]
struct CreateProjectResponse {
    project_id: String,
    name: String,
    workspace_type: String,
}

const VALID_WORKSPACE_TYPES: &[&str] = &["personal", "organization", "client"];

/// Calls `POST {base_url}/v1/projects` with `name` and `workspace_type`.
/// Returns `(project_id, name, workspace_type)` on success.
pub async fn create_project_with_client(
    name: &str,
    workspace_type: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(String, String, String), CreateProjectError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CreateProjectError::InvalidName("name cannot be empty".to_string()));
    }
    if trimmed.len() > 100 {
        return Err(CreateProjectError::InvalidName("name cannot exceed 100 characters".to_string()));
    }
    if !VALID_WORKSPACE_TYPES.contains(&workspace_type) {
        return Err(CreateProjectError::InvalidWorkspaceType(workspace_type.to_string()));
    }

    let url = format!("{}/v1/projects", base_url);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "name": trimmed, "workspace_type": workspace_type }))
        .send()
        .await
        .map_err(|e| CreateProjectError::Backend(e.to_string()))?;

    match resp.status().as_u16() {
        200 => {
            let body: CreateProjectResponse = resp
                .json()
                .await
                .map_err(|e| CreateProjectError::Backend(e.to_string()))?;
            Ok((body.project_id, body.name, body.workspace_type))
        }
        402 => Err(CreateProjectError::NoFreeSlot),
        _ => Err(CreateProjectError::Backend(format!("unexpected status {}", resp.status()))),
    }
}

/// Writes `project_id` into `.postlane/config.json` atomically.
/// Path must be in the registered repos list (security rule 2).
pub fn write_project_id_to_config_impl(
    repo_path: &str,
    project_id: &str,
    repos: &ReposConfig,
) -> Result<String, String> {
    let is_registered = repos.repos.iter().any(|r| r.path == repo_path);
    if !is_registered {
        return Err(format!(
            "Path '{}' is not in the registered repos list",
            repo_path
        ));
    }

    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    reject_if_symlink(&config_path)?;
    if !config_path.exists() {
        return Err(format!(
            "config.json not found at {} — run `postlane init` first",
            config_path.display()
        ));
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    config["project_id"] = serde_json::json!(project_id);

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.json: {}", e))?;

    atomic_write(&config_path, serialized.as_bytes())
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    Ok("We've added a project_id to .postlane/config.json — commit this so your team can access this project.".to_string())
}

/// Calls `POST {base_url}/v1/projects/{project_id}/repos`, then writes `project_id` to config.
pub async fn register_repo_with_project_with_client(
    project_id: &str,
    repo_path: &str,
    description: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    repos: &ReposConfig,
) -> Result<String, String> {
    validate_project_id(project_id)?;
    let is_registered = repos.repos.iter().any(|r| r.path == repo_path);
    if !is_registered {
        return Err(format!("Path '{}' is not in the registered repos list", repo_path));
    }

    let path_hash = sha256_hex(repo_path);
    let url = format!("{}/v1/projects/{}/repos", base_url, project_id);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "repo_path_hash": path_hash, "description": description }))
        .send()
        .await
        .map_err(|e| format!("Backend error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Backend returned {}", resp.status()));
    }

    write_project_id_to_config_impl(repo_path, project_id, repos)
}

use crate::project_cache::{
    get_project_voice_guide_cached, save_project_voice_guide_with_client,
    VOICE_GUIDE_CACHE_TTL_SECS,
};

#[tauri::command]
pub async fn get_project_voice_guide(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    get_project_voice_guide_cached(&project_id, &client, POSTLANE_API_BASE, &token, VOICE_GUIDE_CACHE_TTL_SECS).await
}

/// Calls `DELETE {base_url}/v1/projects/{project_id}`.
/// On 204: deregisters all repos whose config.json matches `project_id`.
/// On any other status (including 401, 403): returns `Err` without touching local state.
pub async fn delete_project_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    state: &crate::app_state::AppState,
) -> Result<(), String> {
    validate_project_id(project_id)?;
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let resp = client.delete(&url).bearer_auth(token).send().await
        .map_err(|e| format!("Network error: {}", e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(SESSION_EXPIRED_ERROR.to_string());
    }
    if status.as_u16() != 204 {
        return Err(format!("Backend returned {} — project not deleted", status));
    }
    let matching = crate::repo_project_filter::list_repos_for_project_impl(project_id, state)?;
    for repo in &matching {
        if let Err(e) = crate::repo_project_filter::unregister_repo_impl(&repo.id, state) {
            log::error!("[delete_project] failed to remove repo '{}': {}", repo.id, e);
            return Err(format!(
                "Project deleted remotely but failed to remove repo '{}' from local registry: {}",
                repo.id, e
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_project(
    project_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    delete_project_with_client(&project_id, &client, POSTLANE_API_BASE, &token, &state).await?;
    let _ = app.emit(crate::platform_constants::PROJECTS_CHANGED_EVENT, ());
    Ok(())
}

#[derive(Deserialize)]
struct ListProjectsResponse {
    projects: Vec<ProjectSummary>,
}

/// Calls `GET {base_url}/v1/projects` and returns the project list.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn list_projects_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<ProjectSummary>, String> {
    let url = format!("{}/v1/projects", base_url);
    let resp = client.get(&url).bearer_auth(token).send().await
        .map_err(|e| format!("Backend error: {}", e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(SESSION_EXPIRED_ERROR.to_string());
    }
    if !status.is_success() {
        return Err(format!("Backend returned {}", status));
    }
    resp.json::<ListProjectsResponse>().await
        .map(|r| r.projects)
        .map_err(|e| format!("Failed to parse response: {}", e))
}

#[tauri::command]
pub async fn list_projects(app: tauri::AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    list_projects_with_client(&client, POSTLANE_API_BASE, &token).await
}

fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

/// Returns the last path segment of a repo's `origin` remote URL (e.g. `"desktop"` from
/// `https://github.com/postlane/desktop.git`). Returns `None` if no remote is configured.
/// Path must be in the registered repos list (security rule 2).
pub fn get_repo_remote_name_impl(repo_path: &str, repos: &ReposConfig) -> Result<Option<String>, String> {
    let is_registered = repos.repos.iter().any(|r| r.path == repo_path);
    if !is_registered {
        return Err(format!("Path '{}' is not in the registered repos list", repo_path));
    }

    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_path)
        .output();

    let out = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(None),
    };

    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(parse_remote_name(&url))
}

fn parse_remote_name(url: &str) -> Option<String> {
    let stripped = url.trim_end_matches('/').trim_end_matches(".git");
    stripped.split('/').next_back().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Reads `.postlane/config.json` from a registered repo path and returns the `project_id` field.
/// Rejects paths not in repos.json (Security Rule 2).
/// Returns `Ok(None)` if the file doesn't exist or if `project_id` is not present.
/// Returns `Err` if the path is unregistered or the file exists but cannot be parsed.
pub fn read_project_id_from_path_impl(path: &str, repos: &ReposConfig) -> Result<Option<String>, String> {
    let is_registered = repos.repos.iter().any(|r| r.path == path);
    if !is_registered {
        return Err(format!("Path '{}' is not in the registered repos list", path));
    }
    let config_path = PathBuf::from(path).join(".postlane/config.json");
    if !config_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;
    Ok(config["project_id"].as_str().map(str::to_string))
}

#[tauri::command]
pub fn read_project_id_from_path(path: String, state: State<AppState>) -> Result<Option<String>, String> {
    let repos = state.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))?;
    read_project_id_from_path_impl(&path, &repos)
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn check_project_status(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    use tauri_plugin_keyring::KeyringExt;
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let status = check_project_status_with_client(&project_id, &client, POSTLANE_API_BASE, &token).await;
    Ok(match status {
        ProjectStatus::Owned => "owned".to_string(),
        ProjectStatus::NotFound => "not_found".to_string(),
        ProjectStatus::Offline => "offline".to_string(),
    })
}

#[tauri::command]
pub async fn check_billing_gate(app: tauri::AppHandle) -> Result<String, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let gate = check_billing_gate_with_client(&client, POSTLANE_API_BASE, &token).await;
    Ok(match gate {
        BillingGate::Free => "free".to_string(),
        BillingGate::None => "none".to_string(),
        BillingGate::Offline => "offline".to_string(),
    })
}

#[tauri::command]
pub async fn create_project(name: String, workspace_type: String, app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let (project_id, project_name, wt) =
        create_project_with_client(&name, &workspace_type, &client, POSTLANE_API_BASE, &token)
            .await
            .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "project_id": project_id, "name": project_name, "workspace_type": wt }))
}

#[tauri::command]
pub fn write_project_id_to_config(
    repo_path: String,
    project_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    write_project_id_to_config_impl(&repo_path, &project_id, &repos)
}

#[tauri::command]
pub async fn register_repo_with_project(
    project_id: String,
    repo_path: String,
    description: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?
        .clone();
    let client = build_client();
    register_repo_with_project_with_client(
        &project_id, &repo_path, &description, &client, POSTLANE_API_BASE, &token, &repos,
    )
    .await
}

#[tauri::command]
pub fn get_repo_remote_name(
    repo_path: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    get_repo_remote_name_impl(&repo_path, &repos)
}

#[tauri::command]
pub async fn save_project_voice_guide(
    project_id: String,
    voice_guide: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    save_project_voice_guide_with_client(&project_id, &voice_guide, &client, POSTLANE_API_BASE, &token).await?;
    let _ = crate::voice_guide_versions::record_version(&project_id);
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Repo, ReposConfig};
    use crate::test_fixtures::{make_state as make_state_with_repos, make_repo as make_repo_entry};
    use httpmock::prelude::*;
    use std::fs;

    fn build_test_client() -> reqwest::Client {
        build_client()
    }

    fn make_repos(paths: &[&str]) -> ReposConfig {
        ReposConfig {
            version: 1,
            repos: paths
                .iter()
                .enumerate()
                .map(|(i, p)| Repo {
                    id: format!("r{}", i),
                    name: format!("Repo{}", i),
                    path: p.to_string(),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                })
                .collect(),
        }
    }

    // ── check_project_status ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_returns_owned_for_200_owned_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/proj-123");
            then.status(200).json_body(serde_json::json!({ "status": "owned", "tier": "free" }));
        });

        let status = check_project_status_with_client("proj-123", &build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(status, ProjectStatus::Owned);
    }

    #[tokio::test]
    async fn test_returns_not_found_for_404_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/proj-456");
            then.status(404).json_body(serde_json::json!({ "id": "proj-456", "status": "not_found" }));
        });

        let status = check_project_status_with_client("proj-456", &build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(status, ProjectStatus::NotFound);
    }

    #[tokio::test]
    async fn test_returns_offline_on_network_error() {
        let status = check_project_status_with_client(
            "proj-789", &build_test_client(), "http://127.0.0.1:19998", "tok",
        ).await;
        assert_eq!(status, ProjectStatus::Offline);
    }

    // ── require_license_token ─────────────────────────────────────────────────

    #[test]
    fn test_require_license_token_returns_err_for_none() {
        let result = require_license_token(None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sign in"), "error should mention sign-in");
    }

    #[test]
    fn test_require_license_token_returns_token_for_some() {
        let result = require_license_token(Some("tok-123".to_string()));
        assert_eq!(result.expect("require_license_token should return Ok for Some"), "tok-123");
    }

    // ── check_billing_gate ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_gate_returns_free_for_new_user() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/gate");
            then.status(200).json_body(serde_json::json!({ "slot": "free" }));
        });

        let gate = check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(gate, BillingGate::Free);
    }

    #[tokio::test]
    async fn test_gate_returns_none_when_no_free_slot() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/gate");
            then.status(200).json_body(serde_json::json!({ "slot": "none" }));
        });

        let gate = check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(gate, BillingGate::None);
    }

    #[tokio::test]
    async fn test_gate_returns_offline_on_network_error() {
        let gate = check_billing_gate_with_client(&build_test_client(), "http://127.0.0.1:19997", "tok").await;
        assert_eq!(gate, BillingGate::Offline);
    }

    // ── create_project ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_creates_project_returns_id() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "project_id": "new-uuid-abc", "name": "My Project", "tier": "free",
                "workspace_type": "personal"
            }));
        });

        let result = create_project_with_client("My Project", "personal", &build_test_client(), &server.base_url(), "tok").await;
        let (id, name, _wt) = result.expect("create_project_with_client should succeed for 200 response");
        assert_eq!(id, "new-uuid-abc");
        assert_eq!(name, "My Project");
    }

    #[tokio::test]
    async fn test_create_project_rejects_empty_name_before_network_call() {
        let result = create_project_with_client("", "personal", &build_test_client(), "http://127.0.0.1:19996", "tok").await;
        assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
    }

    #[tokio::test]
    async fn test_create_project_rejects_whitespace_only_name() {
        let result = create_project_with_client("   ", "personal", &build_test_client(), "http://127.0.0.1:19996", "tok").await;
        assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
    }

    #[tokio::test]
    async fn test_create_project_returns_error_on_402() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(402).json_body(serde_json::json!({ "error": "no_free_slot" }));
        });

        let result = create_project_with_client("Second Project", "personal", &build_test_client(), &server.base_url(), "tok").await;
        assert!(matches!(result, Err(CreateProjectError::NoFreeSlot)));
    }

    // ── write_project_id_to_config ───────────────────────────────────────────

    #[test]
    fn test_writes_project_id_to_config_atomically() {
        let dir = std::env::temp_dir().join("postlane_test_write_project_id_pr");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write config");

        let repos = make_repos(&[dir.to_str().unwrap()]);
        let notice = write_project_id_to_config_impl(dir.to_str().unwrap(), "proj-uuid-xyz", &repos).expect("should succeed");

        let content = fs::read_to_string(config_dir.join("config.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-uuid-xyz"));
        assert!(notice.contains("project_id"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_project_id_rejects_path_not_in_repos() {
        let repos = make_repos(&["/some/other/path"]);
        let result = write_project_id_to_config_impl("/not/registered", "proj-abc", &repos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    #[test]
    #[cfg(unix)]
    fn test_write_config_rejects_symlinked_config_json() {
        let dir = std::env::temp_dir().join("postlane_test_write_symlink");
        let _ = fs::remove_dir_all(&dir);
        let postlane_dir = dir.join(".postlane");
        fs::create_dir_all(&postlane_dir).expect("create .postlane");
        let target = std::env::temp_dir().join("evil_write_target.json");
        fs::write(&target, "{}").expect("write target");
        std::os::unix::fs::symlink(&target, postlane_dir.join("config.json")).expect("create symlink");

        let repos = make_repos(&[dir.to_str().unwrap()]);
        let result = write_project_id_to_config_impl(dir.to_str().unwrap(), "proj-123", &repos);
        assert!(result.is_err(), "must reject symlinked config.json");
        assert!(result.unwrap_err().to_lowercase().contains("symlink"), "error must mention symlink");

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_file(&target);
    }

    // ── register_repo_with_project ───────────────────────────────────────────

    #[tokio::test]
    async fn test_registers_repo_and_writes_project_id() {
        let dir = std::env::temp_dir().join("postlane_test_register_repo_pr");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write");

        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path_matches(regex::Regex::new(r"/v1/projects/.+/repos").unwrap());
            then.status(200).json_body(serde_json::json!({ "repo_id": "repo-uuid-123" }));
        });

        let repos = make_repos(&[dir.to_str().unwrap()]);
        let notice = register_repo_with_project_with_client(
            "proj-abc", dir.to_str().unwrap(), "The desktop app",
            &build_test_client(), &server.base_url(), "tok", &repos,
        ).await.expect("should succeed");

        let content = fs::read_to_string(config_dir.join("config.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-abc"));
        assert!(notice.contains("project_id"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_register_repo_rejects_path_not_in_repos() {
        let repos = make_repos(&["/other/path"]);
        let result = register_repo_with_project_with_client(
            "proj-abc", "/not/registered", "desc",
            &build_client(), "http://127.0.0.1:19995", "tok", &repos,
        ).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    // ── read_project_id_from_path ────────────────────────────────────────────

    fn make_repos_with_path(path: &str) -> ReposConfig {
        ReposConfig {
            version: 1,
            repos: vec![crate::storage::Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        }
    }

    #[test]
    fn test_read_project_id_from_path_rejects_unregistered_path() {
        let repos = ReposConfig { version: 1, repos: vec![] };
        let result = read_project_id_from_path_impl("/unregistered/path", &repos);
        assert!(result.is_err(), "path not in repos.json must be rejected (Security Rule 2)");
    }

    #[test]
    fn test_read_project_id_from_path_returns_id() {
        let dir = std::env::temp_dir().join("postlane_test_read_project_id_present");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"project_id":"proj-abc","scheduler":{"provider":"zernio"}}"#).expect("write config");
        let repos = make_repos_with_path(dir.to_str().unwrap());

        let result = read_project_id_from_path_impl(dir.to_str().unwrap(), &repos);
        assert_eq!(result, Ok(Some("proj-abc".to_string())));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_project_id_from_path_returns_none_when_missing() {
        let dir = std::env::temp_dir().join("postlane_test_read_project_id_missing");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write config");
        let repos = make_repos_with_path(dir.to_str().unwrap());

        let result = read_project_id_from_path_impl(dir.to_str().unwrap(), &repos);
        assert_eq!(result, Ok(None));
        let _ = fs::remove_dir_all(&dir);
    }

    // ── get_repo_remote_name ─────────────────────────────────────────────────

    #[test]
    fn test_returns_remote_name_for_https_remote() {
        let dir = std::env::temp_dir().join("postlane_test_remote_https");
        std::fs::create_dir_all(&dir).expect("create dir");
        std::process::Command::new("git").args(["init"]).current_dir(&dir).output().expect("git init");
        std::process::Command::new("git")
            .args(["remote", "add", "origin", "https://github.com/postlane/desktop.git"])
            .current_dir(&dir).output().expect("git remote add");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert_eq!(result.as_deref(), Some("desktop"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_remote_name_for_ssh_remote() {
        let dir = std::env::temp_dir().join("postlane_test_remote_ssh");
        std::fs::create_dir_all(&dir).expect("create dir");
        std::process::Command::new("git").args(["init"]).current_dir(&dir).output().expect("git init");
        std::process::Command::new("git")
            .args(["remote", "add", "origin", "git@github.com:postlane/desktop.git"])
            .current_dir(&dir).output().expect("git remote add");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert_eq!(result.as_deref(), Some("desktop"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_none_for_no_remote() {
        let dir = std::env::temp_dir().join("postlane_test_remote_none");
        std::fs::create_dir_all(&dir).expect("create dir");
        std::process::Command::new("git").args(["init"]).current_dir(&dir).output().expect("git init");
        let canonical = std::fs::canonicalize(&dir).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_remote_name_rejects_path_not_in_repos() {
        let repos = make_repos(&["/other/path"]);
        let result = get_repo_remote_name_impl("/not/registered", &repos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    // ── create_project workspace_type ───────────────────────────────────────

    #[tokio::test]
    async fn test_create_project_passes_workspace_type() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects")
                .body_contains("\"workspace_type\":\"organization\"");
            then.status(200).json_body(serde_json::json!({
                "project_id": "org-uuid-abc", "name": "Acme", "tier": "free",
                "workspace_type": "organization"
            }));
        });

        let result = create_project_with_client("Acme", "organization", &build_test_client(), &server.base_url(), "tok").await;
        let (id, name, wt) = result.expect("create_project with organization workspace_type should succeed");
        assert_eq!(id, "org-uuid-abc");
        assert_eq!(name, "Acme");
        assert_eq!(wt, "organization");
    }

    #[tokio::test]
    async fn test_create_project_rejects_invalid_workspace_type() {
        let result = create_project_with_client("Acme", "enterprise", &build_test_client(), "http://127.0.0.1:19994", "tok").await;
        assert!(matches!(result, Err(CreateProjectError::InvalidWorkspaceType(_))));
    }

    // ── sha256_hex ────────────────────────────────────────────────────────────

    #[test]
    fn test_sha256_hex_produces_64_char_hex() {
        let h = sha256_hex("test-input");
        assert_eq!(h.len(), 64, "expected 64-char SHA-256 hex, got {} chars: {}", h.len(), h);
    }

    #[test]
    fn test_sha256_hex_is_deterministic() {
        assert_eq!(sha256_hex("/users/hugo/repos/desktop"), sha256_hex("/users/hugo/repos/desktop"));
    }

    #[test]
    fn test_sha256_hex_different_inputs_differ() {
        assert_ne!(sha256_hex("/path/one"), sha256_hex("/path/two"));
    }

    // ── integration: full wizard path ─────────────────────────────────────────

    /// Exercises the full create → register → read_config chain in one test.
    /// Catches contract breaks (e.g. field rename) that unit tests miss because
    /// they test each layer in isolation.
    #[tokio::test]
    async fn test_full_wizard_path_create_register_read_config() {
        let dir = std::env::temp_dir().join("postlane_test_full_wizard_path");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane");
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
            .expect("write initial config");

        let server = MockServer::start();
        let project_id = "wizard-integration-uuid";

        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "project_id": project_id,
                "name": "Integration Workspace",
                "tier": "free",
                "workspace_type": "personal",
            }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path(format!("/v1/projects/{}/repos", project_id));
            then.status(200).json_body(serde_json::json!({ "repo_id": "repo-int-001" }));
        });

        let client = build_test_client();
        let repos = make_repos(&[dir.to_str().unwrap()]);

        // Phase 1: create project
        let (returned_id, returned_name, workspace_type) =
            create_project_with_client("Integration Workspace", "personal", &client, &server.base_url(), "tok")
                .await
                .expect("create_project should succeed");

        assert_eq!(returned_id, project_id);
        assert_eq!(returned_name, "Integration Workspace");
        assert_eq!(workspace_type, "personal");

        // Phase 2: register repo (writes project_id to config)
        register_repo_with_project_with_client(
            &returned_id, dir.to_str().unwrap(), "Integration test repo",
            &client, &server.base_url(), "tok", &repos,
        )
        .await
        .expect("register_repo should succeed");

        // Phase 3: read config back — the chain is complete if this returns the same id
        let read_back = read_project_id_from_path_impl(dir.to_str().unwrap(), &repos)
            .expect("read_project_id should not error");

        assert_eq!(
            read_back.as_deref(),
            Some(project_id),
            "project_id written by register_repo must be readable by read_project_id_from_path"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // ── delete_project (19.2.1) ──────────────────────────────────────────────


    fn write_project_config(repo_dir: &std::path::Path, project_id: &str) {
        let pl_dir = repo_dir.join(".postlane");
        fs::create_dir_all(&pl_dir).expect("create .postlane");
        fs::write(
            pl_dir.join("config.json"),
            format!(r#"{{"project_id":"{}"}}"#, project_id),
        ).expect("write config.json");
    }

    #[tokio::test]
    async fn test_delete_project_succeeds_on_204() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_del_proj_204");
        write_project_config(&dir, "proj-to-delete");

        let state = make_state_with_repos(vec![make_repo_entry("r1", dir.to_str().unwrap())]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-to-delete");
            then.status(204);
        });

        let result = delete_project_with_client("proj-to-delete", &build_test_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_ok(), "204 must return Ok: {:?}", result);

        let repos = state.repos.lock().expect("lock");
        assert!(!repos.repos.iter().any(|r| r.id == "r1"), "r1 must be deregistered after 204");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_delete_project_deregisters_local_repos_only_after_204() {
        let home = dirs::home_dir().expect("home dir");
        let dir = home.join("postlane_test_del_proj_403_noderegister");
        write_project_config(&dir, "proj-protected");

        let state = make_state_with_repos(vec![make_repo_entry("r1", dir.to_str().unwrap())]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-protected");
            then.status(403);
        });

        let result = delete_project_with_client("proj-protected", &build_test_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_err(), "403 must return Err");

        let repos = state.repos.lock().expect("lock");
        assert!(repos.repos.iter().any(|r| r.id == "r1"), "r1 must NOT be deregistered on 403");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_delete_project_returns_error_on_403() {
        let state = make_state_with_repos(vec![]);
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path("/v1/projects/proj-abc");
            then.status(403);
        });

        let result = delete_project_with_client("proj-abc", &build_test_client(), &server.base_url(), "tok", &state).await;
        assert!(result.is_err(), "403 must return Err");
    }

    #[tokio::test]
    async fn test_delete_project_returns_error_on_network_failure() {
        let state = make_state_with_repos(vec![]);
        let result = delete_project_with_client("proj-abc", &build_test_client(), "http://127.0.0.1:19993", "tok", &state).await;
        assert!(result.is_err(), "network failure must return Err");
    }

    // ── list_projects (19.1.3) ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_projects_returns_vec_on_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "projects": [{
                    "id": "proj-1",
                    "name": "Postlane",
                    "workspace_type": "organization",
                    "tier": "free",
                    "billing_active": true,
                    "is_owner": true
                }]
            }));
        });

        let result = list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
        let projects = result.expect("list_projects_with_client should succeed for 200 response");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "proj-1");
        assert_eq!(projects[0].name, "Postlane");
        assert_eq!(projects[0].workspace_type, "organization");
        assert_eq!(projects[0].tier, "free");
        assert!(projects[0].billing_active);
        assert!(projects[0].is_owner);
    }

    #[tokio::test]
    async fn test_list_projects_returns_error_on_http_failure() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(503);
        });

        let result = list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert!(result.is_err(), "HTTP 503 must return Err");
    }

    #[tokio::test]
    async fn test_list_projects_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(401);
        });

        let result = list_projects_with_client(&build_test_client(), &server.base_url(), "expired-tok").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR, "HTTP 401 must return session_expired error");
    }
}
