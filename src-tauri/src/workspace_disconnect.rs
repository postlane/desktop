// SPDX-License-Identifier: BUSL-1.1

//! Pure logic for §22.6 — workspace soft-remove (disconnect) and hard-delete.
//!
//! Ordering for soft-remove:
//!   1. Stop file watchers immediately.
//!   2. Call DELETE /api/v1/projects/{project_id}; 404 = success.
//!   3. Atomically remove the workspace entry from repos.json.
//!   4. Clear project-scoped keyring entries.
//!   5. Disconnect GitHub App installation if one exists.
//!
//! Files on disk are never touched by soft-remove.

use std::path::{Path, PathBuf};
use crate::storage::{self, ReposConfig};

// ── Keyring key patterns (22.6.5) ─────────────────────────────────────────────

/// Returns every scheduler-scoped keyring account key for `project_id`.
/// All entries use the "postlane" service.
fn scheduler_keyring_keys(project_id: &str) -> Vec<String> {
    crate::credential_store::SCHEDULER_PROVIDERS
        .iter()
        .map(|p| format!("{}/{}", p, project_id))
        .collect()
}

/// Returns every Mastodon-scoped keyring account key for `project_id`.
/// Must be called before the active instance key is deleted.
fn mastodon_keyring_keys(project_id: &str, active_instance: Option<&str>) -> Vec<String> {
    let mut keys = vec![
        crate::mastodon_connection::active_instance_key(project_id),
        crate::mastodon_connection::active_username_key(project_id),
    ];
    if let Some(instance) = active_instance {
        keys.push(crate::mastodon_connection::access_token_key(project_id, instance));
    }
    keys
}

// ── Workspace lookup ──────────────────────────────────────────────────────────

fn read_repos(repos_path: &Path) -> Result<ReposConfig, String> {
    storage::read_repos_with_recovery(repos_path).map_err(|e| format!("{:?}", e))
}

/// Returns the workspace path for `workspace_id` from repos.json, or `None`.
pub fn workspace_path_from_repos(repos_path: &Path, workspace_id: &str) -> Option<PathBuf> {
    let config = read_repos(repos_path).ok()?;
    config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .map(|w| PathBuf::from(&w.workspace_path))
}

// ── Soft-remove steps ─────────────────────────────────────────────────────────

/// Removes the workspace entry for `workspace_id` from repos.json atomically.
/// Returns the number of remaining workspaces.
pub fn remove_workspace_entry(repos_path: &Path, workspace_id: &str) -> Result<usize, String> {
    let mut config = read_repos(repos_path)?;
    config.workspaces.retain(|w| w.id != workspace_id);
    let remaining = config.workspaces.len();
    storage::write_repos(repos_path, &config).map_err(|e| format!("{:?}", e))?;
    Ok(remaining)
}

/// Calls `DELETE {api_base}/v1/projects/{project_id}`.
/// Returns `Ok(())` on 2xx or 404. Returns `Err("PL-DEL-001: …")` on other failures.
pub async fn delete_project_api(api_base: &str, project_id: &str, token: &str) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {}", e))?;
    let url = format!("{}/v1/projects/{}", api_base, project_id);
    let resp = client
        .delete(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("PL-DEL-001: network error deleting project: {}", e))?;
    match resp.status().as_u16() {
        200 | 204 | 404 => Ok(()),
        status => Err(format!("PL-DEL-001: server returned {} for project deletion", status)),
    }
}

/// Attempts to delete all project-scoped keyring entries for `project_id`.
/// Ignores individual deletion errors (best-effort).
pub fn clear_project_keyring(
    project_id: &str,
    app: &tauri::AppHandle,
) {
    use tauri_plugin_keyring::KeyringExt;
    let active_instance = app
        .keyring()
        .get_password("postlane", &crate::mastodon_connection::active_instance_key(project_id))
        .ok()
        .flatten();

    let mut keys = scheduler_keyring_keys(project_id);
    keys.extend(mastodon_keyring_keys(project_id, active_instance.as_deref()));

    for key in &keys {
        let _ = app.keyring().delete_password("postlane", key);
    }
}

// ── Hard-delete safelist validation (22.6.13) ─────────────────────────────────

/// Depth and home-dir safety checks only — no registry lookup.
/// Used by account deletion step 9, where repos.json has already been wiped.
pub fn canonicalize_deletion_target(workspace_path: &Path) -> Result<PathBuf, String> {
    let canonical = std::fs::canonicalize(workspace_path)
        .map_err(|_| "PL-DEL-002: Cannot delete this directory".to_string())?;
    if canonical.components().count() < 4 {
        return Err("PL-DEL-002: Cannot delete this directory".to_string());
    }
    let home = dirs::home_dir().ok_or_else(|| "PL-DEL-002: Cannot delete this directory".to_string())?;
    let canonical_home = std::fs::canonicalize(&home).unwrap_or(home);
    if canonical == canonical_home || canonical_home.starts_with(&canonical) {
        return Err("PL-DEL-002: Cannot delete this directory".to_string());
    }
    Ok(canonical)
}

/// Validates `workspace_path` against registry membership + safety conditions.
/// Returns the canonical path on success, or `Err("PL-DEL-002: …")` on failure.
/// Use this for workspace disconnect. For account deletion step 9, use
/// `canonicalize_deletion_target` — repos.json is wiped before step 9 runs.
pub fn safelist_validate_delete_path(
    workspace_path: &Path,
    repos_path: &Path,
) -> Result<PathBuf, String> {
    let config = read_repos(repos_path)?;
    let in_registry = config
        .workspaces
        .iter()
        .any(|w| Path::new(&w.workspace_path) == workspace_path);
    if !in_registry {
        return Err("PL-DEL-002: Cannot delete this directory".to_string());
    }
    canonicalize_deletion_target(workspace_path)
}

// ── Migration journal check (22.6.12a) ────────────────────────────────────────

/// Returns `true` if `{workspace_path}/.migration-journal.json` exists.
pub fn migration_journal_exists(workspace_path: &Path) -> bool {
    workspace_path.join(".migration-journal.json").exists()
}

#[cfg(test)]
#[path = "workspace_disconnect_tests.rs"]
mod tests;
