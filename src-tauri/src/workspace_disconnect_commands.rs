// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for §22.6 — workspace disconnect and hard-delete.

use std::path::PathBuf;
use crate::workspace_disconnect::{
    clear_project_keyring, delete_project_api, migration_journal_exists,
    remove_workspace_entry, safelist_validate_delete_path, workspace_path_from_repos,
};

fn reload_repos(state: &tauri::State<'_, crate::app_state::AppState>) {
    if let Ok(new_repos) = crate::storage::read_repos_with_recovery(&state.repos_path) {
        if let Ok(mut lock) = state.repos.lock() {
            *lock = new_repos;
        }
    }
}

fn license_token(app: &tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_keyring::KeyringExt;
    app.keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())
}

pub(crate) fn record_workspace_disconnected_event(
    workspace_id: &str,
    consent: bool,
    state: &crate::app_state::AppState,
) {
    state.telemetry.record(consent, "workspace_disconnected", serde_json::json!({
        "workspace_id": workspace_id,
    }));
}

pub(crate) fn record_workspace_deleted_event(
    workspace_id: &str,
    optional_deletion_checked: bool,
    consent: bool,
    state: &crate::app_state::AppState,
) {
    state.telemetry.record(consent, "workspace_deleted", serde_json::json!({
        "workspace_id": workspace_id,
        "optional_deletion_checked": optional_deletion_checked,
    }));
}

// ── Soft-remove ───────────────────────────────────────────────────────────────

/// Soft-remove: detaches the workspace from Postlane without touching files on disk.
/// Returns `true` if other workspaces remain (navigate to dashboard), `false` (navigate to wizard).
#[tauri::command]
pub async fn disconnect_workspace(
    workspace_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<bool, String> {
    let rp = state.repos_path.clone();

    crate::watcher::stop_watcher(&workspace_id, &state.watchers);

    if !workspace_id.is_empty() {
        let token = license_token(&app)?;
        delete_project_api(crate::license::POSTLANE_API_BASE, &workspace_id, &token).await?;
        let _ = crate::github_app::disconnect_github_app_impl(
            crate::license::POSTLANE_API_BASE,
            &workspace_id,
            &token,
        )
        .await;
    }

    let remaining = remove_workspace_entry(&rp, &workspace_id)?;
    reload_repos(&state);

    if !workspace_id.is_empty() {
        clear_project_keyring(&workspace_id, &app);
    }

    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_workspace_disconnected_event(&workspace_id, consent, &state);
    Ok(remaining > 0)
}

// ── Hard-delete ───────────────────────────────────────────────────────────────

/// Hard-delete: runs all soft-remove steps then deletes the workspace directory.
/// Returns `true` if other workspaces remain after deletion.
#[tauri::command]
pub async fn delete_workspace(
    workspace_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<bool, String> {
    let rp = state.repos_path.clone();

    // Validate and stop watchers only when a local workspace directory exists.
    // Cloud-only projects (no local workspace) skip file operations but still
    // complete the API + keyring steps.
    let canonical_opt = match workspace_path_from_repos(&rp, &workspace_id) {
        Some(ws_path) => {
            let canonical = safelist_validate_delete_path(&ws_path, &rp)?;
            crate::watcher::stop_watcher(&workspace_id, &state.watchers);
            Some(canonical)
        }
        None => None,
    };

    if !workspace_id.is_empty() {
        let token = license_token(&app)?;
        delete_project_api(crate::license::POSTLANE_API_BASE, &workspace_id, &token).await?;
        let _ = crate::github_app::disconnect_github_app_impl(
            crate::license::POSTLANE_API_BASE,
            &workspace_id,
            &token,
        )
        .await;
    }

    let remaining = remove_workspace_entry(&rp, &workspace_id)?;
    reload_repos(&state);

    if !workspace_id.is_empty() {
        clear_project_keyring(&workspace_id, &app);
    }

    let had_local_dir = canonical_opt.is_some();
    if let Some(canonical) = canonical_opt {
        std::fs::remove_dir_all(&canonical)
            .map_err(|e| format!("Failed to delete workspace directory: {}", e))?;
    }

    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_workspace_deleted_event(&workspace_id, had_local_dir, consent, &state);
    Ok(remaining > 0)
}

// ── Journal check (22.6.12a) ──────────────────────────────────────────────────

/// Returns `true` if a migration journal exists for this workspace (22.6.12a).
#[tauri::command]
pub fn check_workspace_journal(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<bool, String> {
    let rp = state.repos_path.clone();
    match workspace_path_from_repos(&rp, &workspace_id) {
        Some(ws_path) => Ok(migration_journal_exists(&ws_path)),
        None => Ok(false),
    }
}

// ── Helper: resolve workspace path from id ────────────────────────────────────

/// Returns the workspace path for `workspace_id`, used by the frontend
/// to derive the workspace basename for hard-delete confirmation (22.6.11).
#[tauri::command]
pub fn get_workspace_info(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<WorkspaceInfo, String> {
    let rp = state.repos_path.clone();
    let config = crate::storage::read_repos_with_recovery(&rp)
        .map_err(|e| format!("{:?}", e))?;
    let entry = config
        .workspaces
        .into_iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| format!("Workspace '{}' not found", workspace_id))?;
    Ok(WorkspaceInfo {
        workspace_path: entry.workspace_path.clone(),
        name: PathBuf::from(&entry.workspace_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&entry.name)
            .to_string(),
    })
}

#[derive(serde::Serialize)]
pub struct WorkspaceInfo {
    pub workspace_path: String,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> crate::app_state::AppState {
        crate::test_fixtures::make_state(vec![])
    }

    #[test]
    fn test_workspace_disconnected_records_workspace_id() {
        let state = make_state();
        record_workspace_disconnected_event("ws-abc", true, &state);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_disconnected");
        assert_eq!(ev.properties["workspace_id"], "ws-abc");
    }

    #[test]
    fn test_workspace_deleted_records_workspace_id_and_optional_checked() {
        let state = make_state();
        record_workspace_deleted_event("ws-def", true, true, &state);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_deleted");
        assert_eq!(ev.properties["workspace_id"], "ws-def");
        assert_eq!(ev.properties["optional_deletion_checked"], true);
    }

    #[test]
    fn test_workspace_deleted_optional_checked_false_for_cloud_only() {
        let state = make_state();
        record_workspace_deleted_event("ws-cloud", false, true, &state);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.properties["optional_deletion_checked"], false);
    }
}
