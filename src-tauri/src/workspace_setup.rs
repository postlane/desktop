// SPDX-License-Identifier: BUSL-1.1

//! `setup_workspace` Tauri command (checklist 24.3.3) — the final step of the
//! new `WorkspaceSetupWizard`: writes `{workspace}/config.json` and
//! `config.local.json`, copies skill files into every discovered child repo,
//! writes the scheduler API key to the OS keyring (never to any file), and
//! registers the workspace and its child repos.

use crate::child_repo_discovery::ChildRepo;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceConfig {
    pub project_id: String,
    pub base_url: Option<String>,
    pub platforms: Vec<String>,
    pub mastodon_instance: Option<String>,
    pub llm_provider: String,
    pub llm_model: String,
    pub author: String,
    pub style: String,
    pub utm_campaign: Option<String>,
    /// `true` = default (append attribution), `false` = user opted out.
    pub attribution: bool,
    pub scheduler_provider: String,
    pub scheduler_api_key: String,
    pub scheduler_profile_id: Option<String>,
}

fn build_config_json(config: &WorkspaceConfig) -> serde_json::Value {
    let mut json = serde_json::json!({
        "version": 1,
        "project_id": config.project_id,
        "base_url": config.base_url.clone().unwrap_or_else(|| crate::repo_init_config::BASE_URL.to_string()),
        "platforms": config.platforms,
        "llm": { "provider": config.llm_provider, "model": config.llm_model },
        "author": config.author,
        "style": config.style,
    });
    if let Some(instance) = config.mastodon_instance.as_deref().filter(|s| !s.is_empty()) {
        json["mastodon_instance"] = serde_json::json!(instance);
    }
    if let Some(utm) = config.utm_campaign.as_deref().filter(|s| !s.is_empty()) {
        json["utm_campaign"] = serde_json::json!(utm);
    }
    if !config.attribution {
        json["attribution"] = serde_json::json!(false);
    }
    json
}

fn build_config_local_json(config: &WorkspaceConfig) -> serde_json::Value {
    let mut json = serde_json::json!({ "scheduler": { "provider": config.scheduler_provider } });
    if let Some(profile_id) = config.scheduler_profile_id.as_deref().filter(|s| !s.is_empty()) {
        json["profile_id"] = serde_json::json!(profile_id);
    }
    json
}

fn write_config_files(workspace_path: &Path, config: &WorkspaceConfig) -> Result<(), String> {
    let config_bytes = serde_json::to_vec_pretty(&build_config_json(config))
        .map_err(|e| format!("failed to serialise config.json: {}", e))?;
    crate::init::atomic_write(&workspace_path.join("config.json"), &config_bytes)
        .map_err(|e| format!("failed to write config.json: {}", e))?;

    let local_json = serde_json::to_string_pretty(&build_config_local_json(config))
        .map_err(|e| format!("failed to serialise config.local.json: {}", e))?;
    crate::config_local_write::write_workspace_local_config(workspace_path, &local_json)?;
    crate::config_local_write::append_config_local_to_gitignore(workspace_path)
}

fn write_scheduler_credential(
    config: &WorkspaceConfig,
    set_keyring: &dyn Fn(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    let key = crate::scheduler_credentials::get_credential_keyring_key(&config.scheduler_provider, &config.project_id);
    set_keyring(&key, &config.scheduler_api_key)
}

fn copy_skill_files_to_child_repos(child_repos: &[ChildRepo]) -> Result<(), String> {
    for child in child_repos {
        crate::bundle_skills::copy_to_repo(Path::new(&child.path))?;
    }
    Ok(())
}

fn register_child_repos(workspace_path: &Path, child_repos: &[ChildRepo]) -> Result<(), String> {
    let entries: Vec<crate::workspace_repos::RepoEntry> = child_repos
        .iter()
        .map(|c| crate::workspace_repos::RepoEntry {
            id: uuid::Uuid::new_v4().to_string(),
            name: c.name.clone(),
            path: c.path.clone(),
            posts_dir: c.posts_dir.clone(),
            active: true,
            added_at: chrono::Utc::now().to_rfc3339(),
        })
        .collect();
    let repos_config = crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: entries };
    crate::workspace_repos::write_workspace_repos(&workspace_path.join("repos.json"), &repos_config)?;
    crate::workspace_repos::create_workspace_dirs(workspace_path)
}

/// Pure implementation of the "Set up workspace" step (24.3.6). Testable
/// without Tauri. `set_keyring` is injected so tests never need a live OS
/// keyring -- same discipline as `scheduler_credential_writer.rs`'s
/// `CredentialEnv`, but this call site needs only the one write, not that
/// module's heavier account-sync machinery (none of that data exists yet
/// for a workspace still being created in this same call).
pub fn setup_workspace_impl(
    workspace_path: &Path,
    repos_path: &Path,
    config: &WorkspaceConfig,
    child_repos: &[ChildRepo],
    set_keyring: &dyn Fn(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    write_config_files(workspace_path, config)?;
    copy_skill_files_to_child_repos(child_repos)?;
    write_scheduler_credential(config, set_keyring)?;
    register_child_repos(workspace_path, child_repos)?;

    let workspace_name = workspace_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    let workspace_path_str = workspace_path
        .to_str()
        .ok_or_else(|| format!("non-UTF-8 workspace path: {}", workspace_path.display()))?
        .to_string();
    crate::workspace_add::register_workspace_globally(repos_path, &config.project_id, &workspace_name, &workspace_path_str)
}

#[tauri::command]
pub fn setup_workspace(
    path: String,
    config: WorkspaceConfig,
    child_repos: Vec<ChildRepo>,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_keyring::KeyringExt;
    let workspace_path = Path::new(&path);
    let set_keyring = |key: &str, val: &str| -> Result<(), String> {
        app.keyring().set_password("postlane", key, val).map_err(|e| e.to_string())
    };
    setup_workspace_impl(workspace_path, &state.repos_path, &config, &child_repos, &set_keyring)
}

#[cfg(test)]
#[path = "workspace_setup_tests.rs"]
mod tests;
