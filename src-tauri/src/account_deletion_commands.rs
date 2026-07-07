// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for §22.7 — account deletion.

use crate::credential_store::{global_keyring_keys, project_keyring_keys};

pub(crate) fn record_account_deleted(
    state: &crate::app_state::AppState,
    consent: bool,
    project_count: usize,
    had_github_app: bool,
    had_gitlab_token: bool,
    optional_deletion_checked: bool,
    workspace_count: usize,
) {
    state.telemetry.record(consent, "account_deleted", serde_json::json!({
        "project_count": project_count,
        "had_github_app": had_github_app,
        "had_gitlab_token": had_gitlab_token,
        "optional_deletion_checked": optional_deletion_checked,
        "workspace_count": workspace_count,
    }));
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

// ── Test spy ─────────────────────────────────────────────────────────────────
// Captures calls to set_deletion_incomplete_pub during unit tests so phase
// ordering can be verified without touching the real app_state.json.

#[cfg(test)]
thread_local! {
    static INCOMPLETE_SPY: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(vec![]) };
}

pub fn set_deletion_incomplete_pub(value: bool) {
    #[cfg(test)]
    INCOMPLETE_SPY.with(|v| v.borrow_mut().push(value));
    set_deletion_incomplete(value);
}

/// Drains and returns all values passed to `set_deletion_incomplete_pub` since
/// the last call. Only available in `#[cfg(test)]`.
#[cfg(test)]
pub fn drain_incomplete_spy() -> Vec<bool> {
    INCOMPLETE_SPY.with(|v| v.borrow_mut().drain(..).collect())
}

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

