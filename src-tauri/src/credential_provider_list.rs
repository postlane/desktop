// SPDX-License-Identifier: BUSL-1.1

//! Functions for listing which scheduler providers have credentials configured.

use crate::scheduler_credentials::{get_credential_keyring_key, VALID_PROVIDERS};
use tauri_plugin_keyring::KeyringExt;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler_credentials::{get_credential_keyring_key, VALID_PROVIDERS};

    #[test]
    fn test_is_keyring_not_found_recognises_no_entry_messages() {
        assert!(is_keyring_not_found("No such entry exists"), "platform: Linux libsecret");
        assert!(is_keyring_not_found("The specified item could not be found in the keychain"), "platform: macOS");
        assert!(is_keyring_not_found("no entry found"), "generic");
        assert!(!is_keyring_not_found("Keychain locked — user must unlock"), "genuine error must not match");
        assert!(!is_keyring_not_found("Access denied"), "access denied is not a not-found");
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
}
