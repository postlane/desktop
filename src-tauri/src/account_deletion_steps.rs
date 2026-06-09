// SPDX-License-Identifier: BUSL-1.1

//! §22.7.6/22.7.7 — Per-phase deletion step dispatcher.

use serde::Serialize;
use tauri_plugin_keyring::KeyringExt;
use crate::account_deletion::{preflight_session, delete_all_projects, disconnect_all_github_apps, revoke_gitlab_token, wipe_postlane_files};
use crate::credential_store::{global_keyring_keys, project_keyring_keys};
use crate::license::POSTLANE_API_BASE;

// ── Phase result types ────────────────────────────────────────────────────────

#[derive(Serialize, Debug, Clone)]
pub struct DeletionPhaseResult { pub phase: u8, pub message: String, pub next_phase: Option<u8> }

#[derive(Serialize, Debug, Clone)]
pub struct DeletionPhaseError { pub phase: u8, pub code: String, pub message: String, pub skippable: bool }

// ── Phase metadata ────────────────────────────────────────────────────────────

pub fn phase_message(phase: u8) -> &'static str {
    match phase {
        0 => "Verifying session\u{2026}",
        1 | 2 => "Removing project data\u{2026}",
        3 => "Revoking integrations\u{2026}",
        4 => "Clearing credentials\u{2026}",
        5 => "Removing account record\u{2026}",
        6 => "Cleaning up local files\u{2026}",
        7 => "Removing workspace files\u{2026}",
        _ => "Finishing\u{2026}",
    }
}

pub fn is_skippable(phase: u8) -> bool { phase != 0 && phase != 5 }
pub const TOTAL_PHASES: u8 = 8;

fn next_phase(phase: u8) -> Option<u8> { if phase + 1 < TOTAL_PHASES { Some(phase + 1) } else { None } }
fn ok(phase: u8) -> DeletionPhaseResult { DeletionPhaseResult { phase, message: phase_message(phase).to_string(), next_phase: next_phase(phase) } }
fn phase_err(phase: u8, code: &str, msg: String) -> DeletionPhaseError { DeletionPhaseError { phase, code: code.to_string(), message: msg, skippable: is_skippable(phase) } }

fn workspace_ids(repos_path: &std::path::Path) -> Vec<String> {
    crate::storage::read_repos_with_recovery(repos_path)
        .map(|c| c.workspaces.into_iter().filter(|w| !w.id.is_empty()).map(|w| w.id).collect())
        .unwrap_or_default()
}

// ── Per-phase helpers ─────────────────────────────────────────────────────────

async fn run_phase_0(token: &str, client: &reqwest::Client, state: &crate::app_state::AppState) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    preflight_session(POSTLANE_API_BASE, token, client).await.map_err(|m| phase_err(0, "PL-DEL-000", m))?;
    let snapshot = crate::storage::read_repos_with_recovery(&state.repos_path).map(|c| c.workspaces).unwrap_or_default();
    if let Ok(mut snap) = state.deletion_snapshot.lock() { *snap = snapshot; }
    crate::account_deletion_commands::set_deletion_incomplete_pub(true);
    Ok(ok(0))
}

async fn run_phase_1(token: &str, client: &reqwest::Client, repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    delete_all_projects(POSTLANE_API_BASE, token, &workspace_ids(repos_path), client).await
        .map_err(|m| phase_err(1, "PL-DEL-001", m))?;
    Ok(ok(1))
}

async fn run_phase_2(token: &str, client: &reqwest::Client, repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    disconnect_all_github_apps(POSTLANE_API_BASE, token, &workspace_ids(repos_path), client).await
        .map_err(|m| phase_err(2, "PL-DEL-001", m))?;
    Ok(ok(2))
}

async fn run_phase_5(token: &str, client: &reqwest::Client) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let url = format!("{}/v1/account/delete", POSTLANE_API_BASE);
    let resp = client.post(&url).bearer_auth(token).send().await
        .map_err(|e| phase_err(5, "PL-DEL-004", format!("Network error: {}", e)))?;
    match resp.status().as_u16() {
        200 | 404 => { crate::account_deletion_commands::set_deletion_incomplete_pub(false); Ok(ok(5)) }
        s => Err(phase_err(5, "PL-DEL-004", format!("Server returned {}", s))),
    }
}

fn run_phase_4(app: &tauri::AppHandle, repos_path: &std::path::Path) -> DeletionPhaseResult {
    let pids = workspace_ids(repos_path);
    for key in global_keyring_keys() { let _ = app.keyring().delete_password("postlane", key); }
    for pid in &pids { for key in project_keyring_keys(pid) { let _ = app.keyring().delete_password("postlane", &key); } }
    ok(4)
}

fn run_phase_6(repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let dir = repos_path.parent().unwrap_or(repos_path).to_path_buf();
    wipe_postlane_files(&dir).map_err(|m| phase_err(6, "PL-DEL-004", m))?;
    Ok(ok(6))
}

fn run_phase_7(do_delete: bool, state: &crate::app_state::AppState) -> DeletionPhaseResult {
    if do_delete {
        let snapshot = state.deletion_snapshot.lock().map(|s| s.clone()).unwrap_or_default();
        crate::account_deletion::delete_workspace_dirs(&snapshot, &state.repos_path);
    }
    DeletionPhaseResult { phase: 7, message: phase_message(7).to_string(), next_phase: None }
}

// ── Main Tauri command ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_deletion_phase(phase: u8, delete_workspace_dirs: bool, app: tauri::AppHandle, state: tauri::State<'_, crate::app_state::AppState>) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let token = resolve_license_token(app.keyring().get_password("postlane", "license"))?;
    let client = crate::providers::scheduling::build_client();
    match phase {
        0 => run_phase_0(&token, &client, &state).await,
        1 => run_phase_1(&token, &client, &state.repos_path).await,
        2 => run_phase_2(&token, &client, &state.repos_path).await,
        3 => { let _ = revoke_gitlab_token(None, &client, crate::ssrf_validation::validate_ssrf_url).await; Ok(ok(3)) }
        4 => Ok(run_phase_4(&app, &state.repos_path)),
        5 => run_phase_5(&token, &client).await,
        6 => run_phase_6(&state.repos_path),
        7 => Ok(run_phase_7(delete_workspace_dirs, &state)),
        p => Err(phase_err(p, "PL-DEL-000", format!("Unknown phase {p}"))),
    }
}

// ── Helper: temporary repos_path for PathBuf ──────────────────────────────────
// (needed because State<AppState> contains repos_path: PathBuf, not &Path)
impl crate::app_state::AppState {
    pub fn repos_path_ref(&self) -> &std::path::Path { &self.repos_path }
}

fn resolve_license_token<E: std::fmt::Display>(result: Result<Option<String>, E>) -> Result<String, DeletionPhaseError> {
    match result {
        Ok(Some(t)) => Ok(t),
        Ok(None) => Err(phase_err(0, "PL-DEL-000", "Your session has expired. Sign out and sign back in to continue.".to_string())),
        Err(e) => Err(phase_err(0, "PL-DEL-KEYCHAIN", format!("Keychain error: {e}. Check macOS Keychain Access and ensure Postlane has permission."))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_token_returns_token_on_success() {
        let result: Result<Option<String>, String> = Ok(Some("tok".to_string()));
        assert_eq!(resolve_license_token(result).unwrap(), "tok");
    }

    #[test]
    fn test_resolve_token_session_expired_on_no_entry() {
        let result: Result<Option<String>, String> = Ok(None);
        let err = resolve_license_token(result).unwrap_err();
        assert_eq!(err.code, "PL-DEL-000");
        assert!(err.message.to_lowercase().contains("sign"));
    }

    #[test]
    fn test_resolve_token_keychain_error_on_access_denied() {
        let result: Result<Option<String>, String> = Err("access denied".to_string());
        let err = resolve_license_token(result).unwrap_err();
        assert_eq!(err.code, "PL-DEL-KEYCHAIN");
        assert!(err.message.to_lowercase().contains("keychain"));
    }

    #[test]
    fn test_phase_5_not_skippable() { assert!(!is_skippable(5)); }

    #[test]
    fn test_phase_0_not_skippable() { assert!(!is_skippable(0)); }

    #[test]
    fn test_all_other_phases_skippable() {
        for p in [1u8, 2, 3, 4, 6, 7] { assert!(is_skippable(p), "Phase {p} must be skippable"); }
    }

    #[test]
    fn test_no_license_token_error_is_non_skippable_with_sign_in_message() {
        // phase_err(0, ...) must produce skippable:false and a message that tells the user to sign in.
        let err = phase_err(0, "PL-DEL-000", "Your session has expired. Sign out and sign back in to continue.".to_string());
        assert!(!err.skippable, "pre-flight failure must not be skippable");
        assert!(err.message.to_lowercase().contains("sign"), "message must tell user to sign in");
    }

    #[test]
    fn test_phase_message_matches_spec() {
        assert!(phase_message(0).contains("Verifying"));
        assert!(phase_message(1).contains("Removing project data"));
        assert!(phase_message(5).contains("account record"));
        assert!(phase_message(6).contains("local files"));
        assert!(phase_message(7).contains("workspace files"));
    }

    #[test]
    fn test_next_phase_wraps_at_total() {
        assert_eq!(next_phase(6), Some(7));
        assert_eq!(next_phase(7), None);
    }
}
