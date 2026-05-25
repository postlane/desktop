// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::credential_repo_sync::{collect_matching_repo_paths, write_provider_to_matching_repos};
use tauri::{Emitter, State};
use tauri_plugin_keyring::KeyringExt;

pub fn get_credential_keyring_key(provider: &str, id: &str) -> String {
    format!("{}/{}", provider, id)
}

pub fn mask_credential(_credential: &str) -> String {
    "••••••••".to_string()
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

pub const VALID_PROVIDERS: [&str; 8] = ["zernio", "upload_post", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];

pub fn record_provider_configured(state: &AppState, consent: bool, provider: &str) {
    let props = serde_json::json!({"provider": provider, "scope": "workspace"});
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
pub async fn save_scheduler_credential(
    provider: String,
    api_key: String,
    repo_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let libsecret_available = {
        let flag = state
            .libsecret_available
            .lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag
    };

    save_scheduler_credential_impl(&provider, &api_key, libsecret_available)?;

    let keyring_key = get_credential_keyring_key(&provider, &repo_id);
    app.keyring()
        .set_password("postlane", &keyring_key, &api_key)
        .map_err(|e| format!("Failed to store credential: {}", e))?;

    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_provider_configured(&state, consent, &provider);
    write_provider_to_matching_repos(&repo_id, &provider, &state);

    let matching_paths = collect_matching_repo_paths(&repo_id, &state);
    crate::account_config::sync_accounts_for_provider(&provider, &api_key, matching_paths.clone()).await;

    for repo_path in &matching_paths {
        let config_path = std::path::PathBuf::from(repo_path).join(".postlane/config.json");
        let project_id = crate::connected_platforms::read_project_id_from_config(&config_path);
        let mastodon_active = project_id.map(|pid| {
            use crate::mastodon_connection::{active_instance_key, KEYRING_SERVICE};
            app.keyring().get_password(KEYRING_SERVICE, &active_instance_key(&pid))
                .unwrap_or(None).is_some()
        }).unwrap_or(false);
        let _ = crate::project_config_ops::sync_connected_platforms_to_config_impl(
            &config_path, &repo_id, mastodon_active,
            &|key| app.keyring().get_password("postlane", key).unwrap_or(None).is_some(),
        );
    }

    let _ = app.emit("platform-connected", ());
    Ok(())
}

#[tauri::command]
pub fn delete_scheduler_credential(
    provider: String,
    repo_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_scheduler_credential_impl(&provider)?;

    let keyring_key = get_credential_keyring_key(&provider, &repo_id);

    app.keyring()
        .delete_password("postlane", &keyring_key)
        .map_err(|e| format!("Failed to delete credential: {}", e))?;

    for repo_path in collect_matching_repo_paths(&repo_id, &state) {
        if let Err(e) =
            crate::config_merge::remove_scheduler_provider_from_local_config(&repo_path, &provider)
        {
            log::warn!(
                "[delete_scheduler_credential] clear provider from {}: {}",
                repo_path.display(),
                e
            );
        }
    }

    Ok(())
}

pub fn is_keyring_not_found(error_msg: &str) -> bool {
    let lower = error_msg.to_lowercase();
    lower.contains("no entry")
        || lower.contains("no such entry")
        || lower.contains("not found")
        || lower.contains("could not be found")
}

pub fn list_connected_providers_impl<F>(repo_id: &str, has_cred: F) -> Vec<String>
where
    F: Fn(&str, &str) -> bool,
{
    VALID_PROVIDERS
        .iter()
        .filter(|&&p| has_cred(p, repo_id))
        .map(|&p| p.to_string())
        .collect()
}

/// Returns the names of all providers that have a credential stored.
/// Safe to call from the frontend — does not expose credential values.
#[tauri::command]
pub fn list_connected_providers(repo_id: String, app: tauri::AppHandle) -> Vec<String> {
    list_connected_providers_impl(&repo_id, |provider, rid| {
        let key = get_credential_keyring_key(provider, rid);
        matches!(app.keyring().get_password("postlane", &key), Ok(Some(_)))
    })
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
    fn test_mask_credential_returns_fixed_mask() {
        assert_eq!(mask_credential("any-api-key-value"), "••••••••");
    }

    #[test]
    fn test_mask_credential_does_not_expose_suffix() {
        let result = mask_credential("テストabcd");
        assert!(!result.contains("abcd"), "must not expose any part of the credential");
        assert_eq!(result, "••••••••");
    }

    #[test]
    fn test_mask_credential_short_string_returns_mask_only() {
        assert_eq!(mask_credential("ab"), "••••••••");
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
    fn test_get_accepts_all_providers() {
        for provider in VALID_PROVIDERS {
            assert!(get_scheduler_credential_impl(provider).is_ok(), "failed for {}", provider);
        }
    }

    #[test]
    fn test_has_provider_credential_rejects_unknown_provider() {
        assert!(!super::VALID_PROVIDERS.contains(&"unknown_xyz"));
    }

    #[test]
    fn test_delete_accepts_all_providers() {
        for provider in VALID_PROVIDERS {
            assert!(delete_scheduler_credential_impl(provider).is_ok(), "failed for {}", provider);
        }
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

    #[test]
    fn test_record_provider_configured_has_scope_workspace() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let state = AppState::new(ReposConfig { version: 1, repos: vec![] });
        record_provider_configured(&state, true, "zernio");
        let events = state.telemetry.peek_queue();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].properties["scope"], "workspace");
    }

    #[test]
    fn log_libsecret_cleanup_error_does_not_panic_on_err() {
        log_libsecret_cleanup_error(Err("disk full".to_string()), "__libsecret_test__");
    }

    #[test]
    fn log_libsecret_cleanup_error_is_noop_on_ok() {
        log_libsecret_cleanup_error(Ok(()), "__libsecret_test__");
    }

    #[test]
    fn test_get_credential_keyring_key_format() {
        assert_eq!(get_credential_keyring_key("zernio", "proj-abc"), "zernio/proj-abc");
        assert_eq!(get_credential_keyring_key("upload_post", "proj-999"), "upload_post/proj-999");
    }

    #[test]
    fn test_list_connected_providers_empty_when_none_configured() {
        let result = list_connected_providers_impl("ws-1", |_, _| false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_connected_providers_returns_single_provider() {
        let result = list_connected_providers_impl("ws-1", |p, _| p == "zernio");
        assert_eq!(result, vec!["zernio"]);
    }

    #[test]
    fn test_list_connected_providers_returns_multiple_providers() {
        let result = list_connected_providers_impl("ws-1", |p, _| p == "zernio" || p == "upload_post");
        assert!(result.contains(&"zernio".to_string()));
        assert!(result.contains(&"upload_post".to_string()));
    }

    #[test]
    fn test_list_connected_providers_passes_workspace_id_to_closure() {
        let result = list_connected_providers_impl("ws-42", |_, rid| rid == "ws-42");
        assert_eq!(result.len(), VALID_PROVIDERS.len(), "all providers should match when repo_id matches");
    }

    #[test]
    fn test_list_connected_providers_excludes_unconfigured_providers() {
        let result = list_connected_providers_impl("ws-1", |p, _| p == "zernio");
        assert!(!result.contains(&"upload_post".to_string()));
        assert!(!result.contains(&"buffer".to_string()));
    }

    #[test]
    fn test_list_connected_providers_finds_workspace_scoped_credential() {
        let result = list_connected_providers_impl("ws-123", |provider, rid| {
            let key = get_credential_keyring_key(provider, rid);
            key == "zernio/ws-123" && provider == "zernio"
        });
        assert_eq!(result, vec!["zernio"]);
    }

    #[test]
    fn test_list_connected_providers_impl_returns_empty_when_no_providers_have_credentials() {
        let result = list_connected_providers_impl("repo-xyz", |_, _| false);
        assert!(result.is_empty(), "closure returning false must yield empty vec");
    }

    #[test]
    fn test_list_connected_providers_impl_returns_only_providers_with_credentials() {
        let result = list_connected_providers_impl("repo-xyz", |p, _| p == "zernio");
        assert_eq!(result, vec!["zernio"], "only zernio should be returned");
        assert!(!result.contains(&"upload_post".to_string()));
        assert!(!result.contains(&"buffer".to_string()));
    }

    #[test]
    fn test_list_connected_providers_impl_returns_all_when_all_have_credentials() {
        let result = list_connected_providers_impl("repo-xyz", |_, _| true);
        assert_eq!(result.len(), VALID_PROVIDERS.len(), "all VALID_PROVIDERS must be returned");
        for provider in VALID_PROVIDERS {
            assert!(result.contains(&provider.to_string()), "missing provider: {}", provider);
        }
    }

    #[test]
    fn test_check_libsecret_before_save_returns_err_when_libsecret_unavailable() {
        let result = check_libsecret_before_save(Some(false));
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("libsecret"), "error must mention libsecret");
        assert!(msg.contains("apt"), "error must include install hint");
    }

    #[test]
    fn test_check_libsecret_before_save_returns_ok_when_available() {
        assert!(check_libsecret_before_save(Some(true)).is_ok());
    }

    #[test]
    fn test_save_rejects_when_libsecret_unavailable() {
        let result = save_scheduler_credential_impl("zernio", "key", Some(false));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("libsecret"));
    }

    #[test]
    fn test_delete_rejects_unknown_provider() {
        let result = delete_scheduler_credential_impl("not_a_real_provider");
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("Unknown provider"), "got: {}", msg);
    }

    // 21.5.9 — two different project IDs produce distinct keyring keys (credential isolation)
    #[test]
    fn test_credential_key_is_scoped_per_project() {
        let key_a = get_credential_keyring_key("zernio", "proj-alpha");
        let key_b = get_credential_keyring_key("zernio", "proj-beta");
        assert_ne!(key_a, key_b, "different projects must produce different keyring keys");
        assert!(key_a.contains("proj-alpha"), "key must embed the project id");
        assert!(key_b.contains("proj-beta"), "key must embed the project id");
    }

}
