// SPDX-License-Identifier: BUSL-1.1

use crate::platform_constants::KNOWN_SCHEDULER_PROVIDERS;
use serde::{Deserialize, Serialize};
use tauri_plugin_keyring::KeyringExt;

/// Per-provider connection status — returned by `list_scheduler_profiles` (M19 SchedulerBlock).
/// One row per known scheduler provider; `connected` is true when a non-empty keyring entry exists.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SchedulerProviderStatus {
    pub provider: String,
    pub connected: bool,
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

    // ── remove_scheduler_credential ──────────────────────────────────────────

    #[test]
    fn test_remove_scheduler_credential_removes_entry() {
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
        let result = remove_scheduler_credential_impl("buffer");
        assert!(result.is_err());
    }

    // ── add_scheduler_credential ─────────────────────────────────────────────

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

    // ── list_scheduler_profiles ──────────────────────────────────────────────

    #[test]
    fn test_list_scheduler_profiles_returns_connected_providers() {
        let result = list_scheduler_profiles_impl(|key| {
            if key == "postlane-scheduler-zernio" { Ok(Some("api-key-123".to_string())) }
            else { Ok(None) }
        })
        .expect("ok");
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
