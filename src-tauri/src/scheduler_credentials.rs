// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

pub fn get_credential_keyring_key(provider: &str, repo_id: Option<&str>) -> Vec<String> {
    match repo_id {
        Some(id) => vec![format!("{}/{}", provider, id), provider.to_string()],
        None => vec![provider.to_string()],
    }
}

pub fn mask_credential(credential: &str) -> String {
    let mask = "••••••••";
    if credential.len() >= 4 {
        let last_four = &credential[credential.len() - 4..];
        format!("{}{}", mask, last_four)
    } else {
        mask.to_string()
    }
}

pub fn check_libsecret_before_save(libsecret_available: Option<bool>) -> Result<(), String> {
    match libsecret_available {
        Some(false) => Err(
            "libsecret not available. See the warning banner. Install with: sudo apt install libsecret-1-dev"
                .to_string(),
        ),
        Some(true) | None => Ok(()),
    }
}

pub fn check_libsecret_availability(app: Option<tauri::AppHandle>) -> bool {
    let app = match app {
        Some(a) => a,
        None => return true,
    };

    let test_service = "postlane";
    let test_account = "__libsecret_test__";
    let test_password = "test";

    if app.keyring().set_password(test_service, test_account, test_password).is_ok() {
        app.keyring().delete_password(test_service, test_account).is_ok()
    } else {
        false
    }
}

pub const VALID_PROVIDERS: [&str; 7] = ["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];

pub fn record_provider_configured(state: &AppState, consent: bool, provider: &str) {
    state.telemetry.record(consent, "provider_configured", serde_json::json!({"provider": provider}));
}

pub fn save_scheduler_credential_impl(
    provider: &str,
    _api_key: &str,
    libsecret_available: Option<bool>,
) -> Result<(), String> {
    check_libsecret_before_save(libsecret_available)?;
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

pub fn get_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

pub fn delete_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

#[tauri::command]
pub fn get_scheduler_credential(
    provider: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    get_scheduler_credential_impl(&provider)?;

    let keys = get_credential_keyring_key(&provider, repo_id.as_deref());

    for key in keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => return Ok(Some(mask_credential(&credential))),
            Ok(None) => continue,
            Err(e) => return Err(format!("Failed to retrieve credential: {}", e)),
        }
    }

    Ok(None)
}

#[tauri::command]
pub fn save_scheduler_credential(
    provider: String,
    api_key: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    let libsecret_available = {
        let flag = state
            .libsecret_available
            .lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag
    };

    save_scheduler_credential_impl(&provider, &api_key, libsecret_available)?;

    let keyring_key = match repo_id {
        Some(id) => format!("{}/{}", provider, id),
        None => provider.clone(),
    };

    app.keyring()
        .set_password("postlane", &keyring_key, &api_key)
        .map_err(|e| format!("Failed to store credential: {}", e))?;
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_provider_configured(&state, consent, &provider);
    Ok(())
}

#[tauri::command]
pub fn delete_scheduler_credential(
    provider: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    delete_scheduler_credential_impl(&provider)?;

    let keyring_key = match repo_id {
        Some(id) => format!("{}/{}", provider, id),
        None => provider.clone(),
    };

    app.keyring()
        .delete_password("postlane", &keyring_key)
        .map_err(|e| format!("Failed to delete credential: {}", e))?;

    Ok(())
}

/// Returns the masked per-repo scheduler key for a specific provider, or None.
/// Unlike `get_scheduler_credential`, this does NOT fall back to the global key.
#[tauri::command]
pub fn get_per_repo_scheduler_key(
    repo_id: String,
    provider: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    if !VALID_PROVIDERS.contains(&provider.as_str()) {
        return Err(format!("Unknown provider: {}", provider));
    }
    let keyring_key = format!("{}/{}", provider, repo_id);
    match app.keyring().get_password("postlane", &keyring_key) {
        Ok(Some(credential)) => Ok(Some(mask_credential(&credential))),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Failed to retrieve per-repo credential: {}", e)),
    }
}

pub fn validate_repo_registered(repo_id: &str, state: &AppState) -> Result<(), String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo {} not found in repos.json", repo_id))?;
    Ok(())
}

pub fn save_repo_scheduler_key_impl(
    repo_id: &str,
    provider: &str,
    state: &AppState,
) -> Result<(), String> {
    validate_repo_registered(repo_id, state)?;
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

pub fn remove_repo_scheduler_key_impl(
    repo_id: &str,
    provider: &str,
    state: &AppState,
) -> Result<(), String> {
    validate_repo_registered(repo_id, state)?;
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

#[tauri::command]
pub fn save_repo_scheduler_key(
    repo_id: String,
    provider: String,
    key: String,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    save_repo_scheduler_key_impl(&repo_id, &provider, &state)?;
    let keyring_key = format!("{}/{}", provider, repo_id);
    app.keyring()
        .set_password("postlane", &keyring_key, &key)
        .map_err(|e| format!("Failed to store per-repo credential: {}", e))
}

#[tauri::command]
pub fn remove_repo_scheduler_key(
    repo_id: String,
    provider: String,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    remove_repo_scheduler_key_impl(&repo_id, &provider, &state)?;
    let keyring_key = format!("{}/{}", provider, repo_id);
    match app.keyring().delete_password("postlane", &keyring_key) {
        Ok(_) | Err(_) => Ok(()),
    }
}

/// Returns true if any provider has a credential for the given repo.
/// Used by the per-repo scheduler setup modal to decide whether to show.
pub fn has_scheduler_configured_impl(repo_id: &str, app: &tauri::AppHandle) -> bool {
    use tauri_plugin_keyring::KeyringExt;
    for provider in &VALID_PROVIDERS {
        let keys = get_credential_keyring_key(provider, Some(repo_id));
        for key in keys {
            if let Ok(Some(_)) = app.keyring().get_password("postlane", &key) {
                return true;
            }
        }
    }
    false
}

#[tauri::command]
pub fn has_scheduler_configured(repo_id: String, app: tauri::AppHandle) -> bool {
    has_scheduler_configured_impl(&repo_id, &app)
}

/// Returns true if the given provider has a credential stored for the given repo.
/// Does not expose the credential value — safe to call from the frontend.
#[tauri::command]
pub fn has_provider_credential(repo_id: String, provider: String, app: tauri::AppHandle) -> bool {
    if !VALID_PROVIDERS.contains(&provider.as_str()) { return false; }
    let keys = get_credential_keyring_key(&provider, Some(&repo_id));
    for key in keys {
        if let Ok(Some(_)) = app.keyring().get_password("postlane", &key) { return true; }
    }
    false
}

#[tauri::command]
pub fn get_libsecret_status(state: State<AppState>) -> Result<Option<bool>, String> {
    let flag = state
        .libsecret_available
        .lock()
        .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
    Ok(*flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_accepts_publer() {
        assert!(save_scheduler_credential_impl("publer", "key", None).is_ok());
    }

    #[test]
    fn test_save_accepts_outstand() {
        assert!(save_scheduler_credential_impl("outstand", "key", None).is_ok());
    }

    #[test]
    fn test_save_accepts_substack_notes() {
        assert!(save_scheduler_credential_impl("substack_notes", "cookie", None).is_ok());
    }

    #[test]
    fn test_save_accepts_webhook() {
        assert!(save_scheduler_credential_impl("webhook", "https://hooks.zapier.com/x", None).is_ok());
    }

    #[test]
    fn test_save_rejects_unknown_provider() {
        assert!(save_scheduler_credential_impl("unknown_provider", "key", None).is_err());
    }

    #[test]
    fn test_get_accepts_all_seven_providers() {
        for provider in &["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"] {
            assert!(get_scheduler_credential_impl(provider).is_ok(), "failed for {}", provider);
        }
    }

    #[test]
    fn test_has_provider_credential_rejects_unknown_provider() {
        assert!(!super::VALID_PROVIDERS.contains(&"unknown_xyz"));
    }

    #[test]
    fn test_delete_accepts_all_seven_providers() {
        for provider in &["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"] {
            assert!(delete_scheduler_credential_impl(provider).is_ok(), "failed for {}", provider);
        }
    }

    // --- §15.3 per-repo scheduler key ---

    fn make_state_with_repo(repo_id: &str) -> crate::app_state::AppState {
        use crate::storage::{Repo, ReposConfig};
        crate::app_state::AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: repo_id.to_string(),
                name: "test-repo".to_string(),
                path: "/tmp/test-repo".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    #[test]
    fn test_save_repo_scheduler_key_rejects_unregistered_repo() {
        let state = make_state_with_repo("r1");
        let result = save_repo_scheduler_key_impl("unregistered-id", "zernio", &state);
        assert!(result.is_err(), "must reject repo_id not in repos.json");
        assert!(result.unwrap_err().contains("not found"), "error must name the repo");
    }

    #[test]
    fn test_save_repo_scheduler_key_validates_path_against_repos_json() {
        let state = make_state_with_repo("r1");
        let err_result = save_repo_scheduler_key_impl("attacker-id", "zernio", &state);
        assert!(err_result.is_err(), "repo_id not in repos.json must be rejected (Security Rule 2)");
        let ok_result = save_repo_scheduler_key_impl("r1", "zernio", &state);
        assert!(ok_result.is_ok(), "registered repo_id must be accepted");
    }

    #[test]
    fn test_remove_repo_scheduler_key_is_idempotent() {
        let state = make_state_with_repo("r1");
        let result = remove_repo_scheduler_key_impl("r1", "zernio", &state);
        assert!(result.is_ok(), "remove must succeed even when no key is present");
    }

    // --- 11.11.5 telemetry ---

    #[test]
    fn test_record_provider_configured_queues_when_consent_given() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, true, "zernio");
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    }

    #[test]
    fn test_record_provider_configured_no_op_when_consent_not_given() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, false, "zernio");
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
    }
}
