// SPDX-License-Identifier: BUSL-1.1

pub use crate::post_approval::{approve_post, approve_post_impl};
pub use crate::post_export::{export_history_csv, export_history_csv_impl};
pub use crate::post_dismiss::{delete_post, delete_post_impl, dismiss_post, dismiss_post_impl};
pub use crate::post_queries::{get_drafts, get_drafts_impl, get_post_content, get_post_content_impl};
pub use crate::post_retry::{retry_post, retry_post_impl};
pub use crate::repo_mgmt::{
    add_repo, add_repo_impl, check_repo_health, check_repo_health_impl, remove_repo,
    remove_repo_impl, set_repo_active, set_repo_active_impl, update_repo_path,
};
pub use crate::scheduler_credentials::{
    check_libsecret_availability, check_libsecret_before_save, delete_scheduler_credential,
    delete_scheduler_credential_impl, get_credential_keyring_key, get_libsecret_status,
    mask_credential, save_scheduler_credential, save_scheduler_credential_impl,
};

use tauri::State;
use crate::app_state::AppState;

#[tauri::command]
pub fn cancel_post_command(
    _repo_path: String,
    _post_folder: String,
    _post_id: String,
    _platform: String,
    _state: State<AppState>,
) -> Result<(), String> {
    cancel_post_impl()
}

pub(crate) fn cancel_post_impl() -> Result<(), String> {
    Err("Cancel not implemented in Milestone 3 (deferred to M4)".to_string())
}

#[tauri::command]
pub fn get_queue_command(
    _state: State<AppState>,
) -> Result<Vec<crate::types::QueuedPost>, String> {
    get_queue_impl()
}

pub(crate) fn get_queue_impl() -> Result<Vec<crate::types::QueuedPost>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_post_impl_returns_not_implemented_error() {
        let result = cancel_post_impl();
        assert!(result.is_err(), "cancel must return Err");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Cancel not implemented"),
            "error message must contain 'Cancel not implemented', got: {}",
            msg
        );
    }

    #[test]
    fn test_get_queue_impl_returns_empty_vec() {
        let result = get_queue_impl();
        assert!(result.is_ok(), "get_queue must return Ok");
        assert!(result.unwrap().is_empty(), "queue must be empty");
    }
}
