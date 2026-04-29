// SPDX-License-Identifier: BUSL-1.1

pub use crate::post_approval::{approve_post, approve_post_impl};
pub use crate::post_export::{export_history_csv, export_history_csv_impl};
pub use crate::post_ops::{
    delete_post, delete_post_impl, dismiss_post, dismiss_post_impl, get_drafts, get_drafts_impl,
    get_post_content, get_post_content_impl, retry_post, retry_post_impl,
};
pub use crate::repo_mgmt::{
    add_repo, add_repo_impl, check_repo_health, check_repo_health_impl, remove_repo,
    remove_repo_impl, set_repo_active, set_repo_active_impl, update_repo_path,
};
pub use crate::scheduler_credentials::{
    check_libsecret_availability, check_libsecret_before_save, delete_scheduler_credential,
    delete_scheduler_credential_impl, get_credential_keyring_key, get_libsecret_status,
    get_scheduler_credential, get_scheduler_credential_impl, has_scheduler_configured,
    has_scheduler_configured_impl, mask_credential, save_scheduler_credential,
    save_scheduler_credential_impl,
};

use crate::app_state::AppState;
use tauri::State;

#[tauri::command]
pub fn test_scheduler(provider: String, _state: State<AppState>) -> Result<bool, String> {
    let valid_providers = ["zernio", "buffer", "ayrshare"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(true)
}

#[tauri::command]
pub fn cancel_post_command(
    _repo_path: String,
    _post_folder: String,
    _post_id: String,
    _platform: String,
    _state: State<AppState>,
) -> Result<(), String> {
    Err("Cancel not implemented in Milestone 3 (deferred to M4)".to_string())
}

#[tauri::command]
pub fn get_queue_command(
    _state: State<AppState>,
) -> Result<Vec<crate::types::QueuedPost>, String> {
    Ok(Vec::new())
}
