// SPDX-License-Identifier: BUSL-1.1

//! Local file read/write operations for per-repo `.postlane/config.json`.
//!
//! These functions handle the local side of project configuration (reading/writing
//! the `project_id` field and deriving repo names from git remotes). They operate
//! only on paths that are already registered in `repos.json` (Security Rule 2).

use crate::app_state::AppState;
use crate::init::atomic_write;
use crate::project_validation::{reject_if_symlink, validate_project_id};
use crate::storage::ReposConfig;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

pub(crate) const BASE_URL: &str = "https://postlane.dev";
pub(crate) const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-6";

pub(crate) fn build_initial_config_json(project_id: &str) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "project_id": project_id,
        "base_url": BASE_URL,
        "llm": { "provider": "anthropic", "model": DEFAULT_LLM_MODEL }
    })
}

pub(crate) fn build_initial_config_local_json() -> serde_json::Value {
    serde_json::json!({ "scheduler": { "provider": "" } })
}

/// Writes `.postlane/config.json` and `.postlane/config.local.json` into `repo_dir`.
/// Both files are written atomically (tmp → rename). Rejects symlinks.
/// Creates `.postlane/` automatically (via atomic_write's parent-dir creation).
pub(crate) fn write_initial_config_files(repo_dir: &Path, project_id: &str) -> Result<(), String> {
    let config_path = repo_dir.join(".postlane").join("config.json");
    reject_if_symlink(&config_path)?;
    let config_bytes = serde_json::to_vec_pretty(&build_initial_config_json(project_id))
        .map_err(|e| format!("Failed to serialise config.json: {}", e))?;
    atomic_write(&config_path, &config_bytes)
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    let local_path = repo_dir.join(".postlane").join("config.local.json");
    reject_if_symlink(&local_path)?;
    let local_bytes = serde_json::to_vec_pretty(&build_initial_config_local_json())
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    atomic_write(&local_path, &local_bytes)
        .map_err(|e| format!("Failed to write config.local.json: {}", e))?;

    Ok(())
}

/// Computes the hex-encoded SHA-256 digest of `input`.
pub fn sha256_hex(input: &str) -> String {
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

/// Parses the repository name from a git remote URL.
pub fn parse_remote_name(url: &str) -> Option<String> {
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

/// Tauri command: reads the `project_id` field from a repo's `.postlane/config.json`.
#[tauri::command]
pub fn read_project_id_from_path(path: String, state: State<AppState>) -> Result<Option<String>, String> {
    let repos = state.lock_repos()?;
    read_project_id_from_path_impl(&path, &repos)
}

/// Tauri command: writes `project_id` to a repo's `.postlane/config.json`.
#[tauri::command]
pub fn write_project_id_to_config(
    repo_path: String,
    project_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    validate_project_id(&project_id)?;
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    write_project_id_to_config_impl(&repo_path, &project_id, &repos)
}

/// Tauri command: returns the last path segment of a repo's `origin` remote URL.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Repo, ReposConfig};

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

    // ── write_project_id_to_config ───────────────────────────────────────────

    #[test]
    fn test_writes_project_id_to_config_atomically() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        std::fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write config");

        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let notice = write_project_id_to_config_impl(dir.path().to_str().unwrap(), "proj-uuid-xyz", &repos).expect("should succeed");

        let content = std::fs::read_to_string(config_dir.join("config.json")).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-uuid-xyz"));
        assert!(notice.contains("project_id"));
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
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let target_dir = tempfile::TempDir::new().expect("create target temp dir");
        let postlane_dir = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane_dir).expect("create .postlane");
        let target = target_dir.path().join("evil_write_target.json");
        std::fs::write(&target, "{}").expect("write target");
        std::os::unix::fs::symlink(&target, postlane_dir.join("config.json")).expect("create symlink");

        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let result = write_project_id_to_config_impl(dir.path().to_str().unwrap(), "proj-123", &repos);
        assert!(result.is_err(), "must reject symlinked config.json");
        assert!(result.unwrap_err().to_lowercase().contains("symlink"), "error must mention symlink");
    }

    // ── read_project_id_from_path ────────────────────────────────────────────

    #[test]
    fn test_read_project_id_from_path_rejects_unregistered_path() {
        let repos = ReposConfig { version: 1, repos: vec![] };
        let result = read_project_id_from_path_impl("/unregistered/path", &repos);
        assert!(result.is_err(), "path not in repos.json must be rejected (Security Rule 2)");
    }

    #[test]
    fn test_read_project_id_from_path_returns_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        std::fs::write(config_dir.join("config.json"), r#"{"project_id":"proj-abc","scheduler":{"provider":"zernio"}}"#).expect("write config");
        let repos = make_repos_with_path(dir.path().to_str().unwrap());

        let result = read_project_id_from_path_impl(dir.path().to_str().unwrap(), &repos);
        assert_eq!(result, Ok(Some("proj-abc".to_string())));
    }

    #[test]
    fn test_read_project_id_from_path_returns_none_when_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        std::fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).expect("write config");
        let repos = make_repos_with_path(dir.path().to_str().unwrap());

        let result = read_project_id_from_path_impl(dir.path().to_str().unwrap(), &repos);
        assert_eq!(result, Ok(None));
    }

    // ── get_repo_remote_name ─────────────────────────────────────────────────

    #[test]
    fn test_returns_remote_name_for_https_remote() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().expect("git init");
        std::process::Command::new("git")
            .args(["remote", "add", "origin", "https://github.com/postlane/desktop.git"])
            .current_dir(dir.path()).output().expect("git remote add");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert_eq!(result.as_deref(), Some("desktop"));
    }

    #[test]
    fn test_returns_remote_name_for_ssh_remote() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().expect("git init");
        std::process::Command::new("git")
            .args(["remote", "add", "origin", "git@github.com:postlane/desktop.git"])
            .current_dir(dir.path()).output().expect("git remote add");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert_eq!(result.as_deref(), Some("desktop"));
    }

    #[test]
    fn test_returns_none_for_no_remote() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::process::Command::new("git").args(["init"]).current_dir(dir.path()).output().expect("git init");
        let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let repos = make_repos(&[canonical.to_str().unwrap()]);
        let result = get_repo_remote_name_impl(canonical.to_str().unwrap(), &repos).expect("should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_remote_name_rejects_path_not_in_repos() {
        let repos = make_repos(&["/other/path"]);
        let result = get_repo_remote_name_impl("/not/registered", &repos);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    // ── write_project_id_to_config — missing config.json ────────────────────

    #[test]
    fn test_write_project_id_errors_when_config_json_missing() {
        // Path is registered but .postlane/config.json does not exist yet.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let result = write_project_id_to_config_impl(dir.path().to_str().unwrap(), "proj-new", &repos);
        assert!(result.is_err(), "must Err when config.json is absent");
        let err = result.unwrap_err();
        assert!(
            err.contains("config.json not found"),
            "error must mention config.json not found, got: {}",
            err
        );
    }

    #[test]
    fn test_write_project_id_errors_on_invalid_json() {
        // config.json exists but contains invalid JSON — parse step must Err.
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        std::fs::write(config_dir.join("config.json"), b"{ bad json }").expect("write bad config");

        let repos = make_repos(&[dir.path().to_str().unwrap()]);
        let result = write_project_id_to_config_impl(dir.path().to_str().unwrap(), "proj-abc", &repos);
        assert!(result.is_err(), "must Err on unparseable config.json");
        assert!(
            result.unwrap_err().contains("parse"),
            "error must mention parse failure"
        );
    }
}
