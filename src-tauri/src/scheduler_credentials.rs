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

pub use crate::scheduler_credential_writer::{CredentialEnv, save_scheduler_credential_core};

/// Response returned by the `save_scheduler_credential` Tauri command.
/// `account_names` is the merged display-name map. `sync_warning` is `Some` when
/// non-fatal sync failures occurred (e.g. config write errors) — the credential
/// itself was saved but the user should be informed about partial success.
#[derive(serde::Serialize)]
pub struct SaveCredentialResponse {
    pub account_names: std::collections::HashMap<String, String>,
    pub sync_warning: Option<String>,
}

#[tauri::command]
pub async fn save_scheduler_credential(
    provider: String,
    api_key: String,
    repo_id: String,
    username: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SaveCredentialResponse, String> {
    let libsecret_available = {
        let flag = state
            .libsecret_available
            .lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag
    };
    save_scheduler_credential_impl(&provider, &api_key, libsecret_available)?;
    crate::scheduler_account_sync::validate_provider_credential(&provider, &api_key).await?;
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_provider_configured(&state, consent, &provider);
    write_provider_to_matching_repos(&repo_id, &provider, &state);
    let matching_paths = collect_matching_repo_paths(&repo_id, &state);
    let workspace_config_paths = crate::credential_repo_sync::collect_matching_workspace_config_paths(&repo_id, &state);
    let env = CredentialEnv {
        set_keyring: &|key, val| app.keyring().set_password("postlane", key, val).map_err(|e| e.to_string()),
        has_mastodon_active: &|pid| {
            use crate::mastodon_connection::{active_instance_key, KEYRING_SERVICE};
            matches!(app.keyring().get_password(KEYRING_SERVICE, &active_instance_key(pid)), Ok(Some(_)))
        },
        has_keyring_key: &|key| matches!(app.keyring().get_password("postlane", key), Ok(Some(_))),
    };
    let save_result = save_scheduler_credential_core(
        &provider, &api_key, &repo_id, username.as_deref(),
        &matching_paths, &workspace_config_paths, &env,
    ).await?;
    for warning in &save_result.warnings {
        log::warn!("[save_scheduler_credential] {}", warning);
    }
    let _ = app.emit("platform-connected", ());
    let sync_warning = if save_result.warnings.is_empty() {
        None
    } else {
        Some("Credential saved, but some repos could not be synced. Check logs.".to_string())
    };
    Ok(SaveCredentialResponse { account_names: save_result.account_names, sync_warning })
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

/// Re-fetches connected accounts from every configured scheduler provider and
/// updates `config.json` for all repos in the same project.
/// Returns which providers were synced and any per-provider errors.
#[tauri::command]
pub async fn refresh_scheduler_accounts(
    repo_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::scheduler_account_sync::RefreshResult, String> {
    let (repos, workspaces) = {
        let locked = state.lock_repos()?;
        (locked.repos.clone(), locked.workspaces.clone())
    };
    let result = crate::scheduler_account_sync::refresh_scheduler_accounts_impl(
        &repos,
        &workspaces,
        &|provider, project_id| {
            let key = get_credential_keyring_key(provider, project_id);
            app.keyring().get_password("postlane", &key).ok().flatten()
        },
    )
    .await;
    if !result.providers_synced.is_empty() {
        let _ = app.emit("platform-connected", ());
    }
    // Rebuild connected_platforms in config.json for repos in this project
    for repo_path in collect_matching_repo_paths(&repo_id, &state) {
        let config_path = repo_path.join(".postlane/config.json");
        let project_id = crate::connected_platforms::read_project_id_from_config(&config_path);
        let mastodon_active = project_id.as_deref().map(|pid| {
            use crate::mastodon_connection::{active_instance_key, KEYRING_SERVICE};
            matches!(app.keyring().get_password(KEYRING_SERVICE, &active_instance_key(pid)), Ok(Some(_)))
        }).unwrap_or(false);
        let _ = crate::project_config_ops::sync_connected_platforms_to_config_impl(
            &config_path,
            &repo_id,
            mastodon_active,
            &|key| matches!(app.keyring().get_password("postlane", key), Ok(Some(_))),
        );
    }
    Ok(result)
}

pub use crate::credential_provider_list::{is_keyring_not_found, list_connected_providers_impl};

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
    fn test_save_accepts_upload_post() {
        assert!(save_scheduler_credential_impl("upload_post", "sk_test_12345", None).is_ok());
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
    fn test_record_provider_configured_queues_when_consent_given() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_provider_configured(&state, true, "zernio");
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    }

    #[test]
    fn test_record_provider_configured_no_op_when_consent_not_given() {
        let state = crate::test_fixtures::make_state(vec![]);
        record_provider_configured(&state, false, "zernio");
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
    }

    #[test]
    fn test_record_provider_configured_has_scope_workspace() {
        let state = crate::test_fixtures::make_state(vec![]);
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
