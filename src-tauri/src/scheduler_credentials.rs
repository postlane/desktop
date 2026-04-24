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

pub fn save_scheduler_credential_impl(
    provider: &str,
    _api_key: &str,
    libsecret_available: Option<bool>,
) -> Result<(), String> {
    check_libsecret_before_save(libsecret_available)?;

    let valid_providers = ["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];
    if !valid_providers.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }

    Ok(())
}

pub fn get_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    let valid_providers = ["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];
    if !valid_providers.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }
    Ok(())
}

pub fn delete_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    let valid_providers = ["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"];
    if !valid_providers.contains(&provider) {
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
    fn test_delete_accepts_all_seven_providers() {
        for provider in &["zernio", "buffer", "ayrshare", "publer", "outstand", "substack_notes", "webhook"] {
            assert!(delete_scheduler_credential_impl(provider).is_ok(), "failed for {}", provider);
        }
    }
}
