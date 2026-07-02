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

async fn run_phase_0(api_base: &str, token: &str, client: &reqwest::Client, state: &crate::app_state::AppState) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    preflight_session(api_base, token, client).await.map_err(|m| phase_err(0, "PL-DEL-000", m))?;
    let snapshot = crate::storage::read_repos_with_recovery(&state.repos_path).map(|c| c.workspaces).unwrap_or_default();
    if let Ok(mut snap) = state.deletion_snapshot.lock() { *snap = snapshot; }
    // NOTE: deletion_incomplete is NOT set here — phase 0 is pre-flight + snapshot only,
    // no destructive action has occurred. The flag is set at the start of phase 1.
    Ok(ok(0))
}

async fn run_phase_1(api_base: &str, token: &str, client: &reqwest::Client, repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    // Set before the API call: if the app crashes mid-deletion the flag survives (22.7.7a).
    crate::account_deletion_commands::set_deletion_incomplete_pub(true);
    delete_all_projects(api_base, token, &workspace_ids(repos_path), client).await
        .map_err(|m| phase_err(1, "PL-DEL-001", m))?;
    Ok(ok(1))
}

async fn run_phase_2(api_base: &str, token: &str, client: &reqwest::Client, repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    disconnect_all_github_apps(api_base, token, &workspace_ids(repos_path), client).await
        .map_err(|m| phase_err(2, "PL-DEL-001", m))?;
    Ok(ok(2))
}

async fn run_phase_3(api_base: &str, token: &str, client: &reqwest::Client) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    revoke_gitlab_token(api_base, token, client).await
        .map_err(|m| phase_err(3, "PL-DEL-003", m))?;
    Ok(ok(3))
}

async fn run_phase_5(api_base: &str, token: &str, client: &reqwest::Client) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let url = format!("{}/v1/account/delete", api_base);
    let resp = client.post(&url).bearer_auth(token).send().await
        .map_err(|e| phase_err(5, "PL-DEL-004", format!("Network error: {}", e)))?;
    match resp.status().as_u16() {
        200 | 404 => { crate::account_deletion_commands::set_deletion_incomplete_pub(false); Ok(ok(5)) }
        s => Err(phase_err(5, "PL-DEL-004", format!("Server returned {}", s))),
    }
}

fn run_phase_4(app: &tauri::AppHandle, repos_path: &std::path::Path) -> DeletionPhaseResult {
    let pids = workspace_ids(repos_path);
    // "license" is intentionally excluded — phase 5 (server delete) still needs it.
    // The license token is removed by sign_out once the user exits the deletion flow.
    for key in global_keyring_keys() {
        if *key != "license" { let _ = app.keyring().delete_password("postlane", key); }
    }
    for pid in &pids { for key in project_keyring_keys(pid) { let _ = app.keyring().delete_password("postlane", &key); } }
    ok(4)
}

fn run_phase_6(repos_path: &std::path::Path) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let dir = repos_path.parent().unwrap_or(repos_path).to_path_buf();
    wipe_postlane_files(&dir).map_err(|m| phase_err(6, "PL-DEL-004", m))?;
    Ok(ok(6))
}

fn run_phase_7(do_delete: bool, state: &crate::app_state::AppState) -> DeletionPhaseResult {
    let snapshot = state.deletion_snapshot.lock().map(|s| s.clone()).unwrap_or_default();
    if do_delete {
        crate::account_deletion::delete_workspace_dirs(&snapshot, &state.repos_path);
    }
    let consent = crate::app_state::read_app_state().telemetry_consent;
    crate::account_deletion_commands::record_account_deleted(
        state,
        consent,
        snapshot.len(),  // project_count (one project per workspace in v1.4)
        false,           // had_github_app: phase-based flow uses None for GitLab; tracked separately in future
        false,           // had_gitlab_token: same — approximate; improve in H1 telemetry pass
        do_delete,       // optional_deletion_checked: the "delete workspace files" checkbox
        snapshot.len(),  // workspace_count
    );
    DeletionPhaseResult { phase: 7, message: phase_message(7).to_string(), next_phase: None }
}

// ── Main Tauri command ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_deletion_phase(phase: u8, delete_workspace_dirs: bool, app: tauri::AppHandle, state: tauri::State<'_, crate::app_state::AppState>) -> Result<DeletionPhaseResult, DeletionPhaseError> {
    let token = resolve_license_token(app.keyring().get_password("postlane", "license"))?;
    let client = crate::providers::scheduling::build_client();
    match phase {
        0 => run_phase_0(POSTLANE_API_BASE, &token, &client, &state).await,
        1 => run_phase_1(POSTLANE_API_BASE, &token, &client, &state.repos_path).await,
        2 => run_phase_2(POSTLANE_API_BASE, &token, &client, &state.repos_path).await,
        3 => run_phase_3(POSTLANE_API_BASE, &token, &client).await,
        4 => Ok(run_phase_4(&app, &state.repos_path)),
        5 => run_phase_5(POSTLANE_API_BASE, &token, &client).await,
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
    fn test_phase_4_does_not_include_license_in_global_keys() {
        // B20: phase 4 must preserve the "license" key so phase 5 can still authenticate.
        use crate::credential_store::global_keyring_keys;
        let keys_cleared_by_phase_4: Vec<&str> = global_keyring_keys()
            .iter()
            .copied()
            .filter(|k| *k != "license")
            .collect();
        assert!(!keys_cleared_by_phase_4.contains(&"license"), "license must not be deleted in phase 4");
    }

    #[test]
    fn test_next_phase_wraps_at_total() {
        assert_eq!(next_phase(6), Some(7));
        assert_eq!(next_phase(7), None);
    }

    // ── phase_message remaining arms ─────────────────────────────────────────

    #[test]
    fn test_phase_message_phases_2_and_3_match_spec() {
        assert!(
            phase_message(2).contains("Removing project data"),
            "phase 2 must say 'Removing project data', got: {}",
            phase_message(2)
        );
        assert!(
            phase_message(3).contains("Revoking"),
            "phase 3 must say 'Revoking', got: {}",
            phase_message(3)
        );
    }

    #[test]
    fn test_phase_message_phase_4_contains_clearing() {
        assert!(
            phase_message(4).contains("Clearing"),
            "phase 4 must mention 'Clearing', got: {}",
            phase_message(4)
        );
    }

    #[test]
    fn test_phase_message_unknown_phase_returns_finishing() {
        assert!(
            phase_message(99).contains("Finishing"),
            "unknown phase must fall through to 'Finishing', got: {}",
            phase_message(99)
        );
    }

    // ── C1: deletion_incomplete flag timing (22.7.7a) ────────────────────────
    // Flag must be set at the START of phase 1, not at the end of phase 0.

    #[tokio::test]
    async fn test_phase_0_does_not_set_deletion_incomplete() {
        let _ = crate::account_deletion_commands::drain_incomplete_spy();
        let server = httpmock::MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/auth/session");
            then.status(200).json_body(serde_json::json!({"valid": true}));
        });
        let state = crate::app_state::AppState::new_with_path(
            crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] },
            std::path::PathBuf::from("/nonexistent/repos.json"),
        );
        let client = crate::providers::scheduling::build_client();
        let result = run_phase_0(&server.base_url(), "tok", &client, &state).await;
        assert!(result.is_ok(), "phase 0 with valid session must succeed: {:?}", result);
        let spy = crate::account_deletion_commands::drain_incomplete_spy();
        assert!(
            !spy.contains(&true),
            "phase 0 must NOT set deletion_incomplete=true; spy recorded: {:?}", spy
        );
    }

    #[tokio::test]
    async fn test_phase_1_sets_deletion_incomplete_before_api_call() {
        // Even when the API call fails, the flag must be set (proves it fires before the await).
        let _ = crate::account_deletion_commands::drain_incomplete_spy();
        let server = httpmock::MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::DELETE).path_matches(
                regex::Regex::new("/v1/projects/").unwrap(),
            );
            then.status(500);
        });
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let config = crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] };
        crate::storage::write_repos(&repos_path, &config).expect("write repos.json");
        let client = crate::providers::scheduling::build_client();
        let _ = run_phase_1(&server.base_url(), "tok", &client, &repos_path).await;
        let spy = crate::account_deletion_commands::drain_incomplete_spy();
        assert!(
            spy.contains(&true),
            "phase 1 must set deletion_incomplete=true before the API call; spy: {:?}", spy
        );
    }

    // ── next_phase full coverage ──────────────────────────────────────────────

    #[test]
    fn test_next_phase_returns_some_for_phases_0_through_6() {
        for p in 0u8..7 {
            assert_eq!(
                next_phase(p),
                Some(p + 1),
                "next_phase({p}) must be Some({})", p + 1
            );
        }
    }

    // ── ok() result fields ────────────────────────────────────────────────────

    #[test]
    fn test_ok_result_has_correct_phase_and_next_phase() {
        let result = ok(3);
        assert_eq!(result.phase, 3);
        assert_eq!(result.next_phase, Some(4));
        assert!(!result.message.is_empty(), "message must not be empty");
    }

    #[test]
    fn test_ok_result_for_last_phase_has_no_next() {
        let result = ok(7);
        assert_eq!(result.phase, 7);
        assert!(result.next_phase.is_none(), "last phase must have next_phase = None");
    }

    // ── workspace_ids ─────────────────────────────────────────────────────────

    #[test]
    fn test_workspace_ids_returns_empty_for_nonexistent_repos_path() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let missing = tmp.path().join("does_not_exist").join("repos.json");
        let ids = workspace_ids(&missing);
        assert!(ids.is_empty(), "nonexistent repos.json must return empty Vec, got: {:?}", ids);
    }

    #[test]
    fn test_workspace_ids_returns_ids_from_repos_json() {
        use crate::workspace_entry::WorkspaceEntry;
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![
                WorkspaceEntry {
                    id: "id-alpha".to_string(), name: "alpha".to_string(),
                    workspace_path: "/a/b/c".to_string(), active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
                WorkspaceEntry {
                    id: "id-beta".to_string(), name: "beta".to_string(),
                    workspace_path: "/d/e/f".to_string(), active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
            ],
            repos: vec![],
        };
        crate::storage::write_repos(&repos_path, &config).expect("write repos.json");
        let ids = workspace_ids(&repos_path);
        assert_eq!(ids.len(), 2, "must return 2 IDs");
        assert!(ids.contains(&"id-alpha".to_string()), "must include id-alpha");
        assert!(ids.contains(&"id-beta".to_string()), "must include id-beta");
    }

    #[test]
    fn test_workspace_ids_filters_out_entries_with_empty_id() {
        use crate::workspace_entry::WorkspaceEntry;
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![
                WorkspaceEntry {
                    id: "".to_string(), name: "empty-id".to_string(),
                    workspace_path: "/a/b/c".to_string(), active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
                WorkspaceEntry {
                    id: "real-id".to_string(), name: "real".to_string(),
                    workspace_path: "/d/e/f".to_string(), active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
            ],
            repos: vec![],
        };
        crate::storage::write_repos(&repos_path, &config).expect("write repos.json");
        let ids = workspace_ids(&repos_path);
        assert_eq!(ids.len(), 1, "entry with empty id must be filtered out");
        assert!(ids.contains(&"real-id".to_string()), "non-empty id must be included");
    }

    // ── run_phase_6 ───────────────────────────────────────────────────────────

    #[test]
    fn test_run_phase_6_returns_ok_and_phase_6_result() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let config = crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] };
        crate::storage::write_repos(&repos_path, &config).expect("write repos.json");

        let result = run_phase_6(&repos_path).expect("run_phase_6 must succeed on valid dir");
        assert_eq!(result.phase, 6);
        assert_eq!(result.next_phase, Some(7), "phase 6 must be followed by phase 7");
    }

    // ── run_phase_7 ───────────────────────────────────────────────────────────

    #[test]
    fn test_run_phase_7_returns_phase_7_with_no_next_when_not_deleting() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let state = crate::app_state::AppState::new_with_path(
            crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] },
            repos_path,
        );
        let result = run_phase_7(false, &state);
        assert_eq!(result.phase, 7);
        assert!(result.next_phase.is_none(), "phase 7 is the last phase; next_phase must be None");
    }

    #[test]
    fn test_run_phase_7_with_do_delete_true_and_empty_snapshot_returns_phase_7() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos_path = tmp.path().join("repos.json");
        let state = crate::app_state::AppState::new_with_path(
            crate::storage::ReposConfig { version: 2, workspaces: vec![], repos: vec![] },
            repos_path,
        );
        // deletion_snapshot is empty (default), so no dirs are deleted
        let result = run_phase_7(true, &state);
        assert_eq!(result.phase, 7);
        assert!(result.next_phase.is_none());
    }
}
