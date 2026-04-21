// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{AppStateFile, read_app_state, write_app_state};
use serde::{Deserialize, Serialize};

pub use crate::account_config::{
    get_account_ids, get_repo_config_impl, list_profiles_for_repo, save_account_id,
    save_account_id_impl,
};
pub use crate::draft_queries::{DraftPost, get_all_drafts, get_all_drafts_impl};
pub use crate::model_stats::{ModelStatRow, get_model_stats, get_model_stats_impl};
pub use crate::published_queries::{
    PublishedPost, get_all_published, get_all_published_impl, get_repo_published,
    get_repo_published_impl,
};
pub use crate::repo_queries::{RepoWithStatus, get_repos, get_repos_impl, scan_post_statuses};

/// Payload emitted on the "meta-changed" Tauri event
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetaChangedPayload {
    pub repo_id: String,
    pub post_folder: String,
}

#[tauri::command]
pub fn read_app_state_command() -> AppStateFile {
    read_app_state()
}

#[tauri::command]
pub fn save_app_state_command(state: AppStateFile) -> Result<(), String> {
    write_app_state(&state)
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_autostart_enabled() -> bool {
    false
}

pub fn attribution_config_path() -> Result<std::path::PathBuf, String> {
    crate::init::postlane_dir().map(|d| d.join("config.json"))
}

pub fn read_attribution(config_path: &std::path::Path) -> bool {
    if !config_path.exists() {
        return true;
    }
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return true,
    };
    v.get("attribution").and_then(|a| a.as_bool()).unwrap_or(true)
}

pub fn write_attribution(config_path: &std::path::Path, enabled: bool) -> Result<(), String> {
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read global config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse global config: {}", e))?
    } else {
        serde_json::json!({})
    };

    config["attribution"] = serde_json::Value::Bool(enabled);

    let json = serde_json::to_vec_pretty(&config)
        .map_err(|e| format!("Failed to serialize global config: {}", e))?;
    crate::init::atomic_write(config_path, &json)
        .map_err(|e| format!("Failed to write global config: {}", e))
}

#[tauri::command]
pub fn get_attribution() -> bool {
    attribution_config_path()
        .map(|p| read_attribution(&p))
        .unwrap_or(true)
}

#[tauri::command]
pub fn set_attribution(enabled: bool) -> Result<(), String> {
    let path = attribution_config_path()?;
    write_attribution(&path, enabled)
}
