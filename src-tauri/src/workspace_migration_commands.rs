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
use crate::workspace_migration_execute::RecoveryStats;

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
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    use crate::app_state::app_state_path;
    let legacy_repo_count = find_qualifying_legacy_repos(&state.repos_path).len();
    let path = app_state_path()?;
    set_migration_dismissed(&path)?;
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_migration_dismissed_event(&state, consent, legacy_repo_count);
    Ok(())
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
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_migration_completed_event(&state, consent, &project_id, result.results.len());
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
    let stats = crate::workspace_migration_execute::resume_migration_journal(&ws_path, &state.repos_path)?;
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_migration_recovered_event(&state, consent, &workspace_id, &stats);
    Ok(())
}

#[tauri::command]
pub fn dismiss_workspace_journal_session(
    workspace_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<(), String> {
    let ws_path = find_workspace_path(&workspace_id, &state.repos_path)?;
    dismiss_migration_journal_session(&ws_path)
}

// ── Telemetry helpers (22.5.12, 22.9.11) ─────────────────────────────────────

pub(crate) fn record_migration_shown_event(
    state: &crate::app_state::AppState, consent: bool, workspace_id: &str, repo_count: usize,
) {
    state.telemetry.record(consent, "workspace_migration_shown", serde_json::json!({
        "workspace_id": workspace_id,
        "repo_count": repo_count,
    }));
}

pub(crate) fn record_migration_completed_event(
    state: &crate::app_state::AppState, consent: bool, workspace_id: &str, repo_count: usize,
) {
    state.telemetry.record(consent, "workspace_migration_completed", serde_json::json!({
        "workspace_id": workspace_id,
        "repo_count": repo_count,
    }));
}

pub(crate) fn record_migration_dismissed_event(
    state: &crate::app_state::AppState, consent: bool, legacy_repo_count: usize,
) {
    state.telemetry.record(consent, "workspace_migration_dismissed", serde_json::json!({ "legacy_repo_count": legacy_repo_count }));
}

pub(crate) fn record_migration_recovered_event(
    state: &crate::app_state::AppState, consent: bool, workspace_id: &str, stats: &RecoveryStats,
) {
    state.telemetry.record(consent, "workspace_migration_recovered", serde_json::json!({
        "workspace_id": workspace_id,
        "repos_cleaned": stats.repos_cleaned,
        "repos_skipped": stats.repos_skipped,
    }));
}

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

/// Records the `workspace_migration_shown` telemetry event (22.9.11).
/// Called by MigrationBanner on first render.
#[tauri::command]
pub fn record_migration_shown(
    workspace_id: String,
    repo_count: usize,
    state: tauri::State<'_, crate::app_state::AppState>,
) {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_migration_shown_event(&state, consent, &workspace_id, repo_count);
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

    // ── 22.9.11: migration telemetry events ───────────────────────────────────

    #[test]
    fn test_migration_shown_records_workspace_id_and_repo_count() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_migration_shown_event(&state, true, "ws-1", 4);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_migration_shown");
        assert_eq!(ev.properties["workspace_id"], "ws-1");
        assert_eq!(ev.properties["repo_count"], 4);
    }

    #[test]
    fn test_migration_completed_records_workspace_id_and_repo_count() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_migration_completed_event(&state, true, "ws-1", 3);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_migration_completed");
        assert_eq!(ev.properties["workspace_id"], "ws-1");
        assert_eq!(ev.properties["repo_count"], 3);
    }

    #[test]
    fn test_migration_dismissed_records_event_with_legacy_count() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_migration_dismissed_event(&state, true, 3);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_migration_dismissed");
        assert_eq!(ev.properties["legacy_repo_count"], 3);
    }

    #[test]
    fn test_migration_recovered_records_workspace_id() {
        let state = crate::test_fixtures::make_state(vec![]);
        let stats = RecoveryStats { repos_cleaned: 2, repos_skipped: 1 };
        record_migration_recovered_event(&state, true, "ws-2", &stats);
        assert_eq!(state.telemetry.queue_len(), 1);
        let ev = &state.telemetry.peek_queue()[0];
        assert_eq!(ev.name, "workspace_migration_recovered");
        assert_eq!(ev.properties["workspace_id"], "ws-2");
        assert_eq!(ev.properties["repos_cleaned"], 2);
        assert_eq!(ev.properties["repos_skipped"], 1);
    }

    #[test]
    fn test_migration_telemetry_no_event_without_consent() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_migration_shown_event(&state, false, "ws-1", 0);
        record_migration_completed_event(&state, false, "ws-1", 0);
        record_migration_dismissed_event(&state, false, 0);
        assert_eq!(state.telemetry.queue_len(), 0, "no events without consent");
    }
}
