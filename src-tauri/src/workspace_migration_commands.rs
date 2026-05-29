// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for the workspace migration feature (22.5).

use std::path::PathBuf;
use crate::workspace_migration::{
    LegacyRepoInfo, MigrationResult, MigrationStatus, JournalStatus,
    RepoConflicts,
    find_qualifying_legacy_repos, get_migration_status_impl,
    set_migration_dismissed, check_migration_journals,
    dismiss_migration_journal_session, get_migration_conflicts_impl,
};

// ── Status and dismiss ────────────────────────────────────────────────────────

#[tauri::command]
pub fn migration_status(
    state: tauri::State<'_, crate::app_state::AppState>,
) -> MigrationStatus {
    use crate::app_state::app_state_path;
    let app_state_path = app_state_path().unwrap_or_default();
    get_migration_status_impl(&state.repos_path, &app_state_path)
}

#[tauri::command]
pub fn dismiss_migration(
    _state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    use crate::app_state::app_state_path;
    let path = app_state_path()?;
    set_migration_dismissed(&path)
}

// ── Migration execution ───────────────────────────────────────────────────────

#[tauri::command]
pub fn start_workspace_migration(
    workspace_path: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<MigrationResult, String> {
    let ws = PathBuf::from(&workspace_path);
    let project_id = workspace_project_id(&workspace_path, &state.repos_path);
    let qualifying = find_qualifying_legacy_repos(&state.repos_path);
    let result = crate::workspace_migration_execute::execute_migration(
        &qualifying, &ws, &project_id, &state.repos_path,
    );
    reload_repos(&state);
    Ok(result)
}

/// Re-attempts only the repos in `repo_paths` that previously failed (22.5.8).
#[tauri::command]
pub fn retry_workspace_migration(
    workspace_path: String,
    repo_paths: Vec<String>,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<MigrationResult, String> {
    let ws = PathBuf::from(&workspace_path);
    let project_id = workspace_project_id(&workspace_path, &state.repos_path);
    let all_qualifying = find_qualifying_legacy_repos(&state.repos_path);
    let retry_repos: Vec<LegacyRepoInfo> = all_qualifying
        .into_iter()
        .filter(|r| repo_paths.contains(&r.path))
        .collect();
    let result = crate::workspace_migration_execute::execute_migration(
        &retry_repos, &ws, &project_id, &state.repos_path,
    );
    reload_repos(&state);
    Ok(result)
}

// ── Conflict detection (22.5.6) ───────────────────────────────────────────────

#[tauri::command]
pub fn get_migration_conflicts(
    workspace_path: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Vec<RepoConflicts> {
    let ws = PathBuf::from(&workspace_path);
    let project_id = workspace_project_id(&workspace_path, &state.repos_path);
    let qualifying = find_qualifying_legacy_repos(&state.repos_path);
    get_migration_conflicts_impl(&qualifying, &ws, &project_id)
}

// ── Journal / recovery ────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_journal_statuses(
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Vec<JournalStatus> {
    check_migration_journals(&state.repos_path)
}

#[tauri::command]
pub fn resume_workspace_journal(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let ws_path = find_workspace_path(&workspace_id, &state.repos_path)?;
    crate::workspace_migration_execute::resume_migration_journal(&ws_path, &state.repos_path)
}

#[tauri::command]
pub fn dismiss_workspace_journal_session(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let ws_path = find_workspace_path(&workspace_id, &state.repos_path)?;
    dismiss_migration_journal_session(&ws_path)
}

// ── Telemetry (22.5.12) ───────────────────────────────────────────────────────

/// Records the `workspace_migration_reentered` telemetry event (22.5.12).
/// Called when the user clicks "Migrate to workspace..." in Settings.
#[tauri::command]
pub fn note_migration_reentered(
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    state.telemetry.record(consent, "workspace_migration_reentered", serde_json::json!({}));
    Ok(())
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn workspace_project_id(workspace_path: &str, repos_path: &std::path::Path) -> String {
    let global = crate::storage::read_repos_with_recovery(repos_path).unwrap_or_default();
    global.workspaces.iter()
        .find(|w| w.workspace_path == workspace_path)
        .map(|w| w.id.clone())
        .unwrap_or_default()
}

fn find_workspace_path(workspace_id: &str, repos_path: &std::path::Path) -> Result<PathBuf, String> {
    let global = crate::storage::read_repos_with_recovery(repos_path).unwrap_or_default();
    global.workspaces.iter()
        .find(|w| w.id == workspace_id)
        .map(|w| PathBuf::from(&w.workspace_path))
        .ok_or_else(|| format!("Workspace not found: {}", workspace_id))
}

fn reload_repos(state: &tauri::State<'_, crate::app_state::AppState>) {
    if let Ok(new_repos) = crate::storage::read_repos_with_recovery(&state.repos_path) {
        if let Ok(mut lock) = state.repos.lock() {
            *lock = new_repos;
        }
    }
}

// ── Tests for failed-repo retry logic ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_migration::RepoMigrationStatus;

    #[test]
    fn test_failed_repos_extracted_correctly() {
        let results = vec![
            crate::workspace_migration::RepoMigrationResult {
                repo_path: "/repo-a".to_string(),
                repo_name: "repo-a".to_string(),
                status: RepoMigrationStatus::Success { posts_dir: "repo-a".to_string() },
            },
            crate::workspace_migration::RepoMigrationResult {
                repo_path: "/repo-b".to_string(),
                repo_name: "repo-b".to_string(),
                status: RepoMigrationStatus::VerificationFailed { error: "PL-MIG-001".to_string() },
            },
        ];
        let failed: Vec<String> = results.iter()
            .filter(|r| matches!(&r.status, RepoMigrationStatus::VerificationFailed { .. }))
            .map(|r| r.repo_path.clone())
            .collect();
        assert_eq!(failed, vec!["/repo-b"]);
    }
}
