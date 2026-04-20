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
