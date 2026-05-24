// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::init::atomic_write;
use crate::project_validation::{reject_if_symlink, validate_project_id};
use crate::storage::{write_repos, Repo};
use std::fs;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

const BASE_URL: &str = "https://postlane.dev";
const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-6";

fn build_config_json(project_id: &str) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "project_id": project_id,
        "base_url": BASE_URL,
        "llm": { "provider": "anthropic", "model": DEFAULT_LLM_MODEL }
    })
}

fn build_config_local_json() -> serde_json::Value {
    serde_json::json!({
        "scheduler": { "provider": "" }
    })
}

fn is_already_registered(canonical_path: &str, state: &AppState) -> Result<bool, String> {
    let repos = state.lock_repos()?;
    Ok(repos.repos.iter().any(|r| r.path == canonical_path))
}

fn register_in_repos(canonical_path: &str, name: &str, state: &AppState) -> Result<Repo, String> {
    let repo = Repo {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        path: canonical_path.to_string(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };
    let mut repos = state.lock_repos()?;
    repos.repos.push(repo.clone());
    write_repos(&state.repos_path, &repos).map_err(|e| format!("Failed to write repos.json: {:?}", e))?;
    Ok(repo)
}

pub fn connect_repo_from_desktop_impl(
    repo_path: &str,
    project_id: &str,
    state: &AppState,
    home_dir: Option<&Path>,
) -> Result<Repo, String> {
    validate_project_id(project_id)?;

    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize '{}': {}", repo_path, e))?;

    if let Some(home) = home_dir {
        if !canonical_path.starts_with(home) {
            return Err(format!(
                "PathNotAuthorised: '{}' is outside the home directory",
                canonical_path.display()
            ));
        }
    }

    let canonical_str = canonical_path.to_str().ok_or("Invalid path encoding")?;

    if !canonical_path.join(".git").exists() {
        return Err(format!("NotAGitRepo: '{}' is not a git repository", canonical_path.display()));
    }

    if is_already_registered(canonical_str, state)? {
        return Err(format!("RepoAlreadyRegistered: '{}' is already registered", canonical_str));
    }

    let config_path = canonical_path.join(".postlane").join("config.json");
    reject_if_symlink(&config_path)?;

    let config_bytes = serde_json::to_vec_pretty(&build_config_json(project_id))
        .map_err(|e| format!("Failed to serialise config: {}", e))?;
    atomic_write(&config_path, &config_bytes)
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    let local_config_path = canonical_path.join(".postlane").join("config.local.json");
    reject_if_symlink(&local_config_path)?;
    let local_config_bytes = serde_json::to_vec_pretty(&build_config_local_json())
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    atomic_write(&local_config_path, &local_config_bytes)
        .map_err(|e| format!("Failed to write config.local.json: {}", e))?;

    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid folder name")?
        .to_string();

    register_in_repos(canonical_str, &name, state)
}

#[tauri::command]
pub fn connect_repo_from_desktop(
    repo_path: String,
    project_id: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Repo, String> {
    use tauri_plugin_keyring::KeyringExt;
    let home = dirs::home_dir();
    let repo = connect_repo_from_desktop_impl(&repo_path, &project_id, &state, home.as_deref())?;
    crate::repo_mgmt::start_repo_watcher(&repo.id, &repo.path, &state, app_handle.clone());
    let repo_path_buf = std::path::PathBuf::from(&repo.path);
    let pairs = restore_scheduler_for_new_repo(
        &repo_path_buf,
        &crate::scheduler_credentials::VALID_PROVIDERS,
        |provider| {
            let key = crate::scheduler_credentials::get_credential_keyring_key(provider, &project_id);
            app_handle.keyring().get_password("postlane", &key).ok().flatten()
        },
    );
    for (provider, api_key) in pairs {
        let path = repo_path_buf.clone();
        tauri::async_runtime::spawn(async move {
            crate::account_config::sync_accounts_for_provider(&provider, &api_key, vec![path]).await;
        });
    }
    Ok(repo)
}

/// Checks each provider in `providers` via `get_api_key`, writes its name to
/// `config.local.json`, and returns `(provider, api_key)` pairs so the caller
/// can spawn async account-id sync without needing keyring access here.
pub(crate) fn restore_scheduler_for_new_repo(
    repo_path: &Path,
    providers: &[&str],
    get_api_key: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for &provider in providers {
        if let Some(api_key) = get_api_key(provider) {
            if let Err(e) =
                crate::config_merge::write_scheduler_provider_to_local_config(repo_path, provider)
            {
                log::warn!(
                    "[connect_repo] restore scheduler provider '{}': {}",
                    provider,
                    e
                );
            }
            pairs.push((provider.to_string(), api_key));
        }
    }
    pairs
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{home_tmp, make_state};
    use std::fs;

    fn make_git_repo(dir: &Path) {
        fs::create_dir_all(dir.join(".git")).expect("create .git");
    }

    #[test]
    fn test_happy_path_writes_config_and_registers_repo() {
        let dir = home_tmp("connect_repo_happy");
        let _ = fs::remove_dir_all(&dir);
        make_git_repo(&dir);
        let state = make_state(vec![]);

        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            dirs::home_dir().as_deref(),
        );
        let repo = result.expect("should succeed");

        // Shared config.json: version, project_id, base_url, llm — no scheduler
        let config_str = fs::read_to_string(dir.join(".postlane/config.json")).expect("config.json");
        let config: serde_json::Value = serde_json::from_str(&config_str).expect("valid JSON");
        assert_eq!(config["version"].as_u64(), Some(1));
        assert_eq!(config["project_id"].as_str(), Some("proj-abc"));
        assert_eq!(config["base_url"].as_str(), Some(BASE_URL));
        assert_eq!(config["llm"]["model"].as_str(), Some(DEFAULT_LLM_MODEL));
        assert!(config.get("scheduler").is_none(), "scheduler must not appear in shared config.json");

        // Per-user config.local.json: scheduler block written separately
        let local_str = fs::read_to_string(dir.join(".postlane/config.local.json"))
            .expect("config.local.json must be written by connect_repo_from_desktop");
        let local: serde_json::Value = serde_json::from_str(&local_str).expect("valid JSON");
        assert!(local["scheduler"].is_object(), "config.local.json must have a scheduler block");
        assert_eq!(local["scheduler"]["provider"].as_str(), Some(""),
            "impl writes empty provider; Tauri command layer restores from keyring");

        // Repo registered in state
        let repos = state.repos.lock().expect("lock");
        assert!(repos.repos.iter().any(|r| r.id == repo.id));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rejects_path_without_git_dir() {
        let dir = home_tmp("connect_repo_no_git");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        let state = make_state(vec![]);

        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            dirs::home_dir().as_deref(),
        );
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("NotAGitRepo"), "expected NotAGitRepo, got: {}", err);

        // No config.json written
        assert!(!dir.join(".postlane/config.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_rejects_already_registered_path() {
        let dir = home_tmp("connect_repo_already_reg");
        let _ = fs::remove_dir_all(&dir);
        make_git_repo(&dir);
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let state = make_state(vec![crate::test_fixtures::make_repo(
            "existing-id",
            canonical.to_str().unwrap(),
        )]);

        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            dirs::home_dir().as_deref(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RepoAlreadyRegistered"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_write_failure_does_not_register_repo() {
        let dir = home_tmp("connect_repo_write_fail");
        let _ = fs::remove_dir_all(&dir);
        make_git_repo(&dir);
        // Make .postlane a file so atomic_write fails (can't create parent dir)
        fs::write(dir.join(".postlane"), b"not a dir").expect("create blocking file");
        let state = make_state(vec![]);

        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            dirs::home_dir().as_deref(),
        );
        assert!(result.is_err(), "write failure should return Err");

        // Repo must NOT be registered
        let repos = state.repos.lock().expect("lock");
        assert!(repos.repos.is_empty(), "no repo should be registered on write failure");
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_rejects_symlink_on_config_local_json() {
        use std::os::unix::fs::symlink;
        let dir = home_tmp("connect_repo_symlink_local");
        let _ = fs::remove_dir_all(&dir);
        make_git_repo(&dir);
        let postlane_dir = dir.join(".postlane");
        fs::create_dir_all(&postlane_dir).expect("create .postlane");
        // Plant a symlink where config.local.json would be written
        let target = postlane_dir.join("symlink_target.txt");
        fs::write(&target, b"").expect("create target");
        symlink(&target, postlane_dir.join("config.local.json")).expect("create symlink");
        let state = make_state(vec![]);

        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            dirs::home_dir().as_deref(),
        );
        assert!(result.is_err(), "should reject symlink on config.local.json");
        assert!(result.unwrap_err().contains("symlink"), "error must mention symlink");
        let _ = fs::remove_dir_all(&dir);
    }

    // --- §restore_scheduler_for_new_repo ---

    #[test]
    fn test_restore_scheduler_writes_provider_to_config_local_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".postlane")).expect("mkdir");

        restore_scheduler_for_new_repo(dir.path(), &["zernio"], |p| {
            if p == "zernio" { Some("my-key".to_string()) } else { None }
        });

        let local_str = fs::read_to_string(dir.path().join(".postlane/config.local.json"))
            .expect("config.local.json must be written");
        let local: serde_json::Value = serde_json::from_str(&local_str).expect("valid JSON");
        assert_eq!(
            local["scheduler"]["provider"].as_str(),
            Some("zernio"),
            "provider must be written to config.local.json"
        );
    }

    #[test]
    fn test_restore_scheduler_returns_pairs_for_async_sync() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".postlane")).expect("mkdir");

        let pairs = restore_scheduler_for_new_repo(dir.path(), &["zernio", "buffer"], |p| {
            if p == "zernio" { Some("zernio-key".to_string()) } else { None }
        });

        assert_eq!(pairs, vec![("zernio".to_string(), "zernio-key".to_string())]);
    }

    #[test]
    fn test_restore_scheduler_skips_providers_without_credentials() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".postlane")).expect("mkdir");

        let pairs = restore_scheduler_for_new_repo(dir.path(), &["zernio", "buffer"], |_| None);

        assert!(pairs.is_empty(), "no credential → no pairs returned");
        assert!(
            !dir.path().join(".postlane/config.local.json").exists(),
            "config.local.json must not be created when no credentials found"
        );
    }

    #[test]
    fn test_rejects_path_outside_home_directory() {
        let dir = home_tmp("connect_repo_path_check");
        let _ = fs::remove_dir_all(&dir);
        make_git_repo(&dir);
        let state = make_state(vec![]);

        // Pass a fake home_dir that does NOT contain `dir`
        let fake_home = dirs::home_dir().unwrap().join("nonexistent_subdir_xyz");
        let result = connect_repo_from_desktop_impl(
            dir.to_str().unwrap(),
            "proj-abc",
            &state,
            Some(&fake_home),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("PathNotAuthorised"), "expected PathNotAuthorised");
        let _ = fs::remove_dir_all(&dir);
    }
}
