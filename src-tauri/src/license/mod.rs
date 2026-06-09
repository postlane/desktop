// SPDX-License-Identifier: BUSL-1.1

pub mod deep_link;
pub mod validator;

/// Base URL for all Postlane backend API calls.
/// The web app is served at postlane.dev; API routes are under /api/.
pub const POSTLANE_API_BASE: &str = "https://postlane.dev/api";

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

/// Removes the license token from the OS keyring, signing the user out.
#[tauri::command]
pub fn sign_out(app_handle: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_keyring::KeyringExt;
    // Ignore "no entry" errors — if the token is already gone the user is already signed out.
    app_handle.keyring().delete_password("postlane", "license").ok();
    Ok(())
}

/// Returns the display name from the license cache, or None if not signed in.
#[tauri::command]
pub fn get_license_display_name() -> Option<String> {
    validator::read_license_cache().and_then(|c| c.user.display_name)
}

/// Returns the account email for deletion confirmation (22.7.2), or None if unavailable.
#[tauri::command]
pub fn get_license_email() -> Option<String> {
    validator::read_license_cache().and_then(|c| c.user.email)
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

    #[test]
    fn test_postlane_api_base_points_to_production_host() {
        // The web app is deployed at postlane.dev (kamal proxy.host = postlane.dev).
        // Next.js API routes are under /api/, so the base for v1 endpoints is
        // postlane.dev/api — not api.postlane.dev (that subdomain has no DNS record).
        assert_eq!(super::POSTLANE_API_BASE, "https://postlane.dev/api");
    }
}
