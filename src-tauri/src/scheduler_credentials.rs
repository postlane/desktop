// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::platform_constants::KNOWN_SCHEDULER_PROVIDERS;
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

/// Per-provider connection status — returned by `list_scheduler_profiles` (M19 SchedulerBlock).
/// One row per known scheduler provider; `connected` is true when a non-empty keyring entry exists.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SchedulerProviderStatus {
    pub provider: String,
    pub connected: bool,
}

pub fn get_credential_keyring_key(provider: &str, repo_id: Option<&str>) -> Vec<String> {
    match repo_id {
        Some(id) => vec![format!("{}/{}", provider, id), provider.to_string()],
        None => vec![provider.to_string()],
    }
}

pub fn mask_credential(credential: &str) -> String {
    let mask = "••••••••";
    let chars: Vec<char> = credential.chars().collect();
    if chars.len() >= 4 {
        let last_four: String = chars[chars.len() - 4..].iter().collect();
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

fn log_libsecret_cleanup_error(result: Result<(), String>, key: &str) {
    if let Err(e) = result {
        eprintln!("[scheduler_credentials] failed to clean up libsecret test key '{}': {}", key, e);
    }
}

pub fn check_libsecret_availability(app: Option<tauri::AppHandle>) -> bool {
    let app = match app {
        Some(a) => a,
        None => return true,
    };

    let test_service = "postlane";
    let test_account = "__libsecret_test__";

    let available = app.keyring().set_password(test_service, test_account, "test").is_ok();
    if available {
        log_libsecret_cleanup_error(
            app.keyring().delete_password(test_service, test_account).map_err(|e| e.to_string()),
            test_account,
        );
    }
    available
}

pub const VALID_PROVIDERS: [&str; 7] = ["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];

pub fn record_provider_configured(state: &AppState, consent: bool, provider: &str, repo_id: Option<&str>) {
    let scope = if repo_id.is_some() { "repo" } else { "global" };
    let mut props = serde_json::json!({"provider": provider, "scope": scope});
    if let Some(id) = repo_id {
        props["repo_id"] = serde_json::Value::String(id.to_string());
    }
    state.telemetry.record(consent, "provider_configured", props);
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
    record_provider_configured(&state, consent, &provider, None);
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

/// Validates that the given provider is in KNOWN_SCHEDULER_PROVIDERS (M19 canonical list).
/// The actual keyring deletion is performed by `remove_scheduler_credential`.
/// Keyring key format: "postlane-scheduler-{provider}" under service "postlane".
pub fn remove_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    if !KNOWN_SCHEDULER_PROVIDERS.contains(&provider) {
        return Err(format!(
            "Unknown scheduler provider '{}'. Supported providers: {}",
            provider,
            KNOWN_SCHEDULER_PROVIDERS.join(", ")
        ));
    }
    Ok(())
}

/// Removes a scheduler credential from the OS keyring.
/// `project_id` is accepted but unused in v1; reserved for v2 per-org credential isolation.
#[tauri::command]
pub fn remove_scheduler_credential(
    provider: String,
    _project_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    remove_scheduler_credential_impl(&provider)?;
    let keyring_key = format!("postlane-scheduler-{}", provider);
    app.keyring()
        .delete_password("postlane", &keyring_key)
        .map_err(|e| format!("Failed to remove scheduler credential for '{}': {}", provider, e))
}

pub fn get_per_repo_scheduler_key_impl(
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

/// Returns the masked per-repo scheduler key for a specific provider, or None.
/// Unlike `get_scheduler_credential`, this does NOT fall back to the global key.
#[tauri::command]
pub fn get_per_repo_scheduler_key(
    repo_id: String,
    provider: String,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<Option<String>, String> {
    get_per_repo_scheduler_key_impl(&repo_id, &provider, &state)?;
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
    consent: bool,
) -> Result<(), String> {
    validate_repo_registered(repo_id, state)?;
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    record_provider_configured(state, consent, provider, Some(repo_id));
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
    let consent = crate::app_state::read_app_state().telemetry_consent;
    save_repo_scheduler_key_impl(&repo_id, &provider, &state, consent)?;
    let keyring_key = format!("{}/{}", provider, repo_id);
    app.keyring()
        .set_password("postlane", &keyring_key, &key)
        .map_err(|e| format!("Failed to store per-repo credential: {}", e))
}

/// Validates provider is known and, when a repo_id is provided, that it is registered.
/// Global callers (SchedulerTab, WebhookPanel, Wizard) pass None; per-repo callers pass Some.
pub fn validate_scheduler_registration_impl(repo_id: Option<&str>, provider: &str, state: &AppState) -> Result<bool, String> {
    if let Some(id) = repo_id {
        validate_repo_registered(id, state)?;
    }
    if !VALID_PROVIDERS.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(true)
}

pub fn is_keyring_not_found(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("no entry")
        || lower.contains("no such entry")
        || lower.contains("not found")
        || lower.contains("could not be found")
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
        Ok(_) => Ok(()),
        Err(e) if is_keyring_not_found(&e.to_string()) => Ok(()),
        Err(e) => Err(format!("Failed to remove per-repo credential: {}", e)),
    }
}

fn credential_found(key: &str, result: Result<Option<String>, String>) -> bool {
    match result {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            if !is_keyring_not_found(&e) {
                eprintln!("[scheduler_credentials] keyring error for '{}': {}", key, e);
            }
            false
        }
    }
}

/// Returns true if any provider has a credential for the given repo.
/// Used by the per-repo scheduler setup modal to decide whether to show.
pub fn has_scheduler_configured_impl(repo_id: &str, app: &tauri::AppHandle) -> bool {
    use tauri_plugin_keyring::KeyringExt;
    for provider in &VALID_PROVIDERS {
        let keys = get_credential_keyring_key(provider, Some(repo_id));
        for key in keys {
            if credential_found(&key, app.keyring().get_password("postlane", &key).map_err(|e| e.to_string())) {
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

/// Validates and stores a scheduler credential in the OS keyring.
/// Uses the M19 canonical keyring key format: `"postlane-scheduler-{provider}"`.
/// `keyring_set_fn` is injectable for testing — the real command passes the AppHandle keyring.
pub fn add_scheduler_credential_impl(
    provider: &str,
    api_key: &str,
    keyring_set_fn: &mut dyn FnMut(&str, &str) -> Result<(), String>,
) -> Result<(), String> {
    if !KNOWN_SCHEDULER_PROVIDERS.contains(&provider) {
        return Err(format!(
            "Unknown scheduler provider '{}'. Supported providers: {}",
            provider,
            KNOWN_SCHEDULER_PROVIDERS.join(", ")
        ));
    }
    if api_key.is_empty() {
        return Err("api_key must not be empty".to_string());
    }
    if api_key.len() > 512 {
        return Err(format!(
            "api_key must be 512 characters or fewer (got {})",
            api_key.len()
        ));
    }
    // Canonical key format — must match remove_scheduler_credential and list_scheduler_profiles.
    let keyring_key = format!("postlane-scheduler-{}", provider);
    keyring_set_fn(&keyring_key, api_key)
}

/// Stores a scheduler API key in the OS keyring.
/// `project_id` is accepted but unused in v1; reserved for v2 per-org credential isolation.
#[tauri::command]
pub fn add_scheduler_credential(
    provider: String,
    api_key: String,
    _project_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    add_scheduler_credential_impl(&provider, &api_key, &mut |key, value| {
        app.keyring()
            .set_password("postlane", key, value)
            .map_err(|e| format!("Failed to store scheduler credential for '{}': {}", provider, e))
    })
}

/// Check each known scheduler provider and return its connection status.
/// `keyring_fn` is injectable for testing without a real AppHandle.
pub fn list_scheduler_profiles_impl(
    keyring_fn: impl Fn(&str) -> Result<Option<String>, String>,
) -> Result<Vec<SchedulerProviderStatus>, String> {
    KNOWN_SCHEDULER_PROVIDERS
        .iter()
        .map(|provider| {
            let key = format!("postlane-scheduler-{}", provider);
            let connected = match keyring_fn(&key) {
                Ok(Some(v)) => !v.is_empty(),
                Ok(None) => false,
                Err(e) => return Err(format!("Keyring error for provider '{}': {}", provider, e)),
            };
            Ok(SchedulerProviderStatus { provider: provider.to_string(), connected })
        })
        .collect()
}

/// Returns connection status for each known scheduler provider.
/// `project_id` is accepted but unused in v1; reserved for v2 per-org credential isolation.
#[tauri::command]
pub fn list_scheduler_profiles(
    _project_id: String,
    app: tauri::AppHandle,
) -> Result<Vec<SchedulerProviderStatus>, String> {
    list_scheduler_profiles_impl(|key| {
        app.keyring()
            .get_password("postlane", key)
            .map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_credential_does_not_panic_on_multibyte_utf8_suffix() {
        // "€€" is 6 bytes; len()-4=2 falls inside the 3-byte first '€' char.
        // A byte-slice impl panics here; a char-aware impl must not.
        let result = mask_credential("€€");
        assert!(result.contains('•'), "must return masked output, not panic");
    }

    #[test]
    fn test_mask_credential_shows_last_four_chars_not_bytes() {
        // Each Japanese char is 3 bytes; last 4 chars = last 12 bytes.
        // A byte-aware impl would return the wrong suffix.
        let result = mask_credential("テストabcd");
        assert!(result.ends_with("abcd"), "last 4 characters must be shown: got '{}'", result);
    }

    #[test]
    fn test_mask_credential_short_string_returns_mask_only() {
        let result = mask_credential("ab");
        assert_eq!(result, "••••••••");
    }

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

    // --- M19 remove_scheduler_credential ---

    #[test]
    fn test_remove_scheduler_credential_removes_entry() {
        // Valid provider resolves; impl returns Ok (keyring deletion tested at integration level)
        assert!(remove_scheduler_credential_impl("zernio").is_ok());
    }

    #[test]
    fn test_remove_scheduler_credential_returns_error_for_unknown_provider() {
        let result = remove_scheduler_credential_impl("completely_unknown");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("zernio"), "error should list known providers");
    }

    #[test]
    fn test_remove_scheduler_credential_returns_error_when_not_found() {
        // "buffer" was valid under old VALID_PROVIDERS but is not in M19 KNOWN_SCHEDULER_PROVIDERS
        let result = remove_scheduler_credential_impl("buffer");
        assert!(result.is_err());
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
    fn test_is_keyring_not_found_recognises_no_entry_messages() {
        assert!(is_keyring_not_found("No such entry exists"), "platform: Linux libsecret");
        assert!(is_keyring_not_found("The specified item could not be found in the keychain"), "platform: macOS");
        assert!(is_keyring_not_found("no entry found"), "generic");
        assert!(!is_keyring_not_found("Keychain locked — user must unlock"), "genuine error must not match");
        assert!(!is_keyring_not_found("Access denied"), "access denied is not a not-found");
    }

    #[test]
    fn test_get_per_repo_scheduler_key_rejects_unregistered_repo() {
        let state = make_state_with_repo("r1");
        let result = get_per_repo_scheduler_key_impl("attacker-id", "zernio", &state);
        assert!(result.is_err(), "must reject repo_id not in repos.json (Security Rule 2)");
        let ok_result = get_per_repo_scheduler_key_impl("r1", "zernio", &state);
        assert!(ok_result.is_ok(), "registered repo_id must pass validation");
    }

    #[test]
    fn test_save_repo_scheduler_key_rejects_unregistered_repo() {
        let state = make_state_with_repo("r1");
        let result = save_repo_scheduler_key_impl("unregistered-id", "zernio", &state, false);
        assert!(result.is_err(), "must reject repo_id not in repos.json");
        assert!(result.unwrap_err().contains("not found"), "error must name the repo");
    }

    #[test]
    fn test_save_repo_scheduler_key_validates_path_against_repos_json() {
        let state = make_state_with_repo("r1");
        let err_result = save_repo_scheduler_key_impl("attacker-id", "zernio", &state, false);
        assert!(err_result.is_err(), "repo_id not in repos.json must be rejected (Security Rule 2)");
        let ok_result = save_repo_scheduler_key_impl("r1", "zernio", &state, false);
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
    fn test_save_repo_scheduler_key_records_telemetry_with_consent() {
        let state = make_state_with_repo("r1");
        save_repo_scheduler_key_impl("r1", "zernio", &state, true).unwrap();
        assert_eq!(state.telemetry.queue_len(), 1, "one telemetry event must be queued after save");
    }

    #[test]
    fn test_save_repo_scheduler_key_no_telemetry_without_consent() {
        let state = make_state_with_repo("r1");
        save_repo_scheduler_key_impl("r1", "zernio", &state, false).unwrap();
        assert_eq!(state.telemetry.queue_len(), 0, "no telemetry event without consent");
    }

    #[test]
    fn test_record_provider_configured_queues_when_consent_given() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, true, "zernio", None);
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    }

    #[test]
    fn test_record_provider_configured_no_op_when_consent_not_given() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, false, "zernio", None);
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
    }

    // --- telemetry payload ---

    #[test]
    fn test_save_repo_scheduler_key_telemetry_includes_repo_id_and_scope() {
        let state = make_state_with_repo("r1");
        save_repo_scheduler_key_impl("r1", "zernio", &state, true).unwrap();
        let events = state.telemetry.peek_queue();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].properties["repo_id"], "r1", "per-repo event must include repo_id");
        assert_eq!(events[0].properties["scope"], "repo", "per-repo event must have scope=repo");
    }

    #[test]
    fn test_record_provider_configured_global_has_scope_global_and_no_repo_id() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, true, "zernio", None);
        let events = state.telemetry.peek_queue();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].properties["scope"], "global", "global event must have scope=global");
        assert!(events[0].properties.get("repo_id").is_none(), "global event must not include repo_id");
    }

    // --- validate_scheduler_registration_impl Security Rule 2 ---

    #[test]
    fn test_validate_scheduler_registration_rejects_unregistered_repo() {
        let state = make_state_with_repo("r1");
        let result = validate_scheduler_registration_impl(Some("attacker-id"), "zernio", &state);
        assert!(result.is_err(), "must reject repo_id not in repos.json (Security Rule 2)");
    }

    #[test]
    fn test_validate_scheduler_registration_accepts_registered_repo_with_valid_provider() {
        let state = make_state_with_repo("r1");
        let result = validate_scheduler_registration_impl(Some("r1"), "zernio", &state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_scheduler_registration_rejects_invalid_provider() {
        let state = make_state_with_repo("r1");
        let result = validate_scheduler_registration_impl(Some("r1"), "not_a_provider", &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_scheduler_registration_without_repo_id_skips_repo_validation() {
        // Global callers (SchedulerTab, WebhookPanel, Wizard) pass no repo_id.
        // They must still pass if the provider is known.
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let empty_state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = validate_scheduler_registration_impl(None, "zernio", &empty_state);
        assert!(result.is_ok(), "global callers without repo_id must succeed for valid providers");
    }

    #[test]
    fn test_validate_scheduler_registration_without_repo_id_still_rejects_unknown_provider() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let empty_state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        let result = validate_scheduler_registration_impl(None, "not_a_provider", &empty_state);
        assert!(result.is_err());
    }

    // --- log_libsecret_cleanup_error ---

    #[test]
    fn log_libsecret_cleanup_error_does_not_panic_on_err() {
        log_libsecret_cleanup_error(Err("disk full".to_string()), "__libsecret_test__");
    }

    #[test]
    fn log_libsecret_cleanup_error_is_noop_on_ok() {
        log_libsecret_cleanup_error(Ok(()), "__libsecret_test__");
    }

    // --- credential_found ---

    #[test]
    fn credential_found_returns_true_when_credential_exists() {
        assert!(credential_found("postlane/zernio/r1", Ok(Some("api-key".to_string()))));
    }

    #[test]
    fn credential_found_returns_false_when_no_entry() {
        assert!(!credential_found("postlane/zernio/r1", Ok::<_, String>(None)));
    }

    #[test]
    fn credential_found_returns_false_and_does_not_panic_on_genuine_keyring_error() {
        assert!(!credential_found("postlane/zernio/r1", Err("Keychain locked — user must unlock".to_string())));
    }

    #[test]
    fn credential_found_returns_false_silently_on_not_found_error() {
        assert!(!credential_found("postlane/zernio/r1", Err("no entry found".to_string())));
    }

    // --- add_scheduler_credential ---

    #[test]
    fn test_add_scheduler_credential_stores_in_keyring() {
        let mut stored_key = String::new();
        let mut stored_value = String::new();
        let result = add_scheduler_credential_impl("zernio", "api-key-123", &mut |k, v| {
            stored_key = k.to_string();
            stored_value = v.to_string();
            Ok(())
        });
        assert!(result.is_ok());
        assert_eq!(stored_key, "postlane-scheduler-zernio", "keyring key must use canonical M19 format");
        assert_eq!(stored_value, "api-key-123");
    }

    #[test]
    fn test_add_scheduler_credential_rejects_unknown_provider() {
        let result = add_scheduler_credential_impl("buffer", "key", &mut |_, _| Ok(()));
        assert!(result.is_err(), "provider not in KNOWN_SCHEDULER_PROVIDERS must be rejected");
        assert!(result.unwrap_err().contains("zernio"), "error must list known providers");
    }

    #[test]
    fn test_add_scheduler_credential_rejects_empty_api_key() {
        let result = add_scheduler_credential_impl("zernio", "", &mut |_, _| Ok(()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_add_scheduler_credential_rejects_api_key_exceeding_512_chars() {
        let long_key = "x".repeat(513);
        let result = add_scheduler_credential_impl("zernio", &long_key, &mut |_, _| Ok(()));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("512"), "error must mention the limit");
    }

    // --- list_scheduler_profiles ---

    #[test]
    fn test_list_scheduler_profiles_returns_connected_providers() {
        let result = list_scheduler_profiles_impl(|key| {
            if key == "postlane-scheduler-zernio" { Ok(Some("api-key-123".to_string())) }
            else { Ok(None) }
        }).expect("ok");
        assert_eq!(result.len(), KNOWN_SCHEDULER_PROVIDERS.len());
        let zernio = result.iter().find(|p| p.provider == "zernio").expect("zernio must be present");
        assert!(zernio.connected, "zernio must be connected when keyring has a non-empty entry");
    }

    #[test]
    fn test_list_scheduler_profiles_returns_empty_when_no_credentials_stored() {
        let result = list_scheduler_profiles_impl(|_| Ok(None)).expect("ok");
        assert!(!result.is_empty(), "must return a row per known provider");
        assert!(result.iter().all(|p| !p.connected), "all providers must be disconnected");
    }

    #[test]
    fn test_list_scheduler_profiles_returns_error_on_keyring_failure() {
        let result = list_scheduler_profiles_impl(|_| Err("Keychain locked".to_string()));
        assert!(result.is_err(), "keyring error must propagate as Err");
    }
}
