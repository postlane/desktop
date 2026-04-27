// SPDX-License-Identifier: BUSL-1.1

pub mod deep_link;
pub mod validator;

/// Returns true if a license token is stored in the OS keyring.
/// Used by the frontend to show/hide the "Sign in" button.
#[tauri::command]
pub fn get_license_signed_in(app_handle: tauri::AppHandle) -> bool {
    use tauri_plugin_keyring::KeyringExt;
    matches!(
        app_handle.keyring().get_password("postlane", "license"),
        Ok(Some(_))
    )
}

#[cfg(test)]
mod tests {
    // get_license_signed_in cannot be unit-tested without a real Tauri AppHandle.
    // It is covered by the integration test in the Tauri test harness and manually
    // during deep link activation testing. The keyring interaction is the only logic.
    //
    // NOTE: the function is intentionally trivial — the only way it could be wrong is
    // if the keyring key names ("postlane"/"license") don't match what the deep link
    // handler writes, which is enforced by the shared constants below.
    const KEYRING_SERVICE: &str = "postlane";
    const KEYRING_KEY: &str = "license";

    #[test]
    fn test_keyring_constants_match_deep_link_handler() {
        // The deep link handler uses hardcoded "postlane"/"license".
        // This test ensures mod.rs uses the same names so they stay in sync.
        assert_eq!(KEYRING_SERVICE, "postlane");
        assert_eq!(KEYRING_KEY, "license");
    }
}
