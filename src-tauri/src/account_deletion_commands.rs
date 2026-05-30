// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for §22.7 — account deletion.

use tauri_plugin_keyring::KeyringExt;
use crate::account_deletion::{DeleteAccountParams, delete_account_impl};
use crate::credential_store::{global_keyring_keys, project_keyring_keys};

fn license_token(app: &tauri::AppHandle) -> Result<String, String> {
    app.keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token".to_string())
}

/// Injectable core: calls `delete_fn` for every keyring key this account owns.
/// Used by `clear_all_keyring` (production) and unit tests (captures deleted keys).
pub fn clear_all_keyring_impl(project_ids: &[String], mut delete_fn: impl FnMut(&str)) {
    for key in global_keyring_keys() {
        delete_fn(key);
    }
    for pid in project_ids {
        for key in project_keyring_keys(pid) {
            delete_fn(&key);
        }
    }
}

fn clear_all_keyring(project_ids: &[String], app: &tauri::AppHandle) {
    clear_all_keyring_impl(project_ids, |key| {
        let _ = app.keyring().delete_password("postlane", key);
    });
}

pub fn set_deletion_incomplete_pub(value: bool) { set_deletion_incomplete(value) }

fn set_deletion_incomplete(value: bool) {
    use crate::app_state::app_state_path;
    if let Ok(path) = app_state_path() {
        let mut s = crate::app_state::read_app_state();
        s.deletion_incomplete = value;
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            let _ = crate::init::atomic_write(&path, json.as_bytes());
        }
    }
}

/// Returns `true` if a previous deletion attempt failed at Step 5 (22.7.7a).
#[tauri::command]
pub fn get_deletion_incomplete() -> bool {
    crate::app_state::read_app_state().deletion_incomplete
}

/// Executes the full account deletion sequence (Steps pre-flight → 9).
/// Step 10 (clear session, navigate) is done by the React caller on success.
#[tauri::command]
pub async fn delete_account(
    delete_workspace_dirs: bool,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let token = license_token(&app)?;
    let repos_path = state.repos_path.clone();

    let config = crate::storage::read_repos_with_recovery(&repos_path)
        .map_err(|e| format!("PL-DEL-005: Cannot read workspace registry: {:?}", e))?;

    let project_ids: Vec<String> = config.workspaces.iter()
        .filter(|w| !w.id.is_empty())
        .map(|w| w.id.clone())
        .collect();

    // Mark deletion as in-progress before Step 1 (22.7.7a).
    set_deletion_incomplete(true);

    let client = crate::providers::scheduling::build_client();

    let postlane_dir = repos_path.parent()
        .unwrap_or(&repos_path)
        .to_path_buf();

    let params = DeleteAccountParams {
        postlane_dir,
        api_base: crate::license::POSTLANE_API_BASE.to_string(),
        token: token.clone(),
        project_ids: project_ids.clone(),
        project_ids_with_github_app: project_ids.clone(),
        gitlab_instance_url: None,
        delete_workspace_dirs,
    };

    let result = delete_account_impl(
        params, &client, crate::ssrf_validation::validate_ssrf_url,
    ).await;

    match result {
        Ok(_) => {
            // Step 4: clear all keyring entries.
            clear_all_keyring(&project_ids, &app);
            // Step 5 succeeded → clear deletion_incomplete flag.
            set_deletion_incomplete(false);
            Ok(())
        }
        Err(e) => Err(e),
    }
}
