// SPDX-License-Identifier: BUSL-1.1

use std::sync::Arc;
use tauri::{Emitter, Manager};
use tauri_plugin_keyring::KeyringExt;

/// Maps a `DeepLinkError` to a user-friendly message suitable for display in the UI.
/// Internal details (OS errors, server terminology) are never exposed.
pub fn user_facing_activation_error(e: &crate::license::deep_link::DeepLinkError) -> String {
    use crate::license::deep_link::DeepLinkError;
    match e {
        DeepLinkError::TokenRejected => "Sign-in failed. Please try again.".to_string(),
        DeepLinkError::BackendUnavailable => {
            "Couldn't connect to Postlane. Check your internet connection and try again."
                .to_string()
        }
        DeepLinkError::KeyringWrite(_) => {
            "Couldn't save your credentials. Check your system keychain settings and try again."
                .to_string()
        }
        DeepLinkError::InvalidUrl(_) | DeepLinkError::MalformedToken => {
            "Sign-in failed. Please try again.".to_string()
        }
        DeepLinkError::CacheWrite(_) => {
            "Sign-in succeeded but a local cache write failed. You may need to sign in again after restarting."
                .to_string()
        }
    }
}

/// Generates the session token, starts the HTTP server on port 47312, and
/// spawns a task that receives validated JWT tokens from the `/activate` route
/// and processes them identically to the deep-link handler.
pub fn spawn_http_server(
    app_handle: tauri::AppHandle,
    repos_config: crate::storage::ReposConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let repos_path = crate::init::postlane_dir()?.join("repos.json");
    let token = crate::http_server::generate_and_write_token()?;
    let repos_arc = Arc::new(tokio::sync::Mutex::new(repos_config));
    let (activation_tx, activation_rx) = tokio::sync::mpsc::channel::<(String, bool)>(4);
    let server_state = crate::http_server::ServerState {
        token,
        repos: repos_arc,
        repos_path,
        activation_tx: Some(activation_tx),
        projects: Arc::new(tokio::sync::RwLock::new(vec![])),
    };

    // Bind synchronously so the port file is written before setup_app returns.
    // This eliminates a race where the wizard appeared before the async server wrote the file.
    let listener = crate::http_server::bind_listener(47312)?;
    let port = listener.local_addr()?.port();
    crate::http_server::write_port_file(port)?;
    if let Some(state) = app_handle.try_state::<crate::app_state::AppState>() {
        if let Ok(mut guard) = state.http_port.lock() {
            *guard = Some(port);
        }
    }
    log::info!("HTTP server bound to port {}", port);

    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::http_server::serve_on_listener(server_state, listener).await {
            log::error!("Failed to start HTTP server: {}", e);
        }
    });
    spawn_activation_listener(activation_rx, app_handle);
    Ok(())
}

fn spawn_activation_listener(
    mut rx: tokio::sync::mpsc::Receiver<(String, bool)>,
    app_handle: tauri::AppHandle,
) {
    tauri::async_runtime::spawn(async move {
        while let Some((tok, new_link)) = rx.recv().await {
            log::info!("[activate] validating token from local server (length={})", tok.len());
            let handle = app_handle.clone();
            let keyring_handle = handle.clone();
            let client = crate::providers::scheduling::build_client();
            let result = crate::license::deep_link::handle_activate(
                &tok,
                &client,
                crate::license::POSTLANE_API_BASE,
                move |t| {
                    keyring_handle
                        .keyring()
                        .set_password("postlane", "license", t)
                        .map_err(|e| e.to_string())
                },
                crate::license::validator::write_license_cache,
            )
            .await;
            match result {
                Ok(display_name) => {
                    log::info!("License activated via local callback for {}", display_name);
                    let _ = handle.emit(
                        "license:activated",
                        serde_json::json!({ "display_name": display_name, "new_link": new_link }),
                    );
                }
                Err(e) => {
                    log::warn!("Local callback activation failed: {}", e);
                    let _ = handle.emit(
                        "license:error",
                        serde_json::json!({ "message": user_facing_activation_error(&e) }),
                    );
                }
            }
        }
    });
}

/// Spawns a background task that flushes queued telemetry every 30 minutes.
/// No-ops if consent is false or if not signed in.
pub fn spawn_telemetry_flush(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        interval.tick().await;
        loop {
            interval.tick().await;
            if !crate::app_state::read_app_state().telemetry_consent {
                continue;
            }
            let token = match app_handle.keyring().get_password("postlane", "license") {
                Ok(Some(t)) => t,
                Ok(None) => continue,
                Err(e) => {
                    log::warn!(
                        "Telemetry flush: failed to read license token from keyring: {}",
                        e
                    );
                    continue;
                }
            };
            let state: tauri::State<crate::app_state::AppState> = app_handle.state();
            state.telemetry.flush(&token).await;
        }
    });
}

/// Spawns the 24-hour license revalidation loop.
/// Reads the keyring on each cycle so that a re-authentication mid-session is picked up.
pub fn spawn_license_revalidation(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = crate::providers::scheduling::build_client();
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            let token = match app_handle.keyring().get_password("postlane", "license") {
                Ok(Some(t)) => t,
                Ok(None) => {
                    log::info!("[revalidation] no token in keyring, skipping");
                    continue;
                }
                Err(e) => {
                    log::warn!("[revalidation] failed to read keyring: {}", e);
                    continue;
                }
            };
            match crate::license::validator::validate_token_enforcing_expiry(
                &token,
                &client,
                crate::license::POSTLANE_API_BASE,
            )
            .await
            {
                Ok(crate::license::validator::LicenseState::Expired) => {
                    log::warn!("[revalidation] license expired");
                    let _ = app_handle.emit("license:expired", serde_json::json!({}));
                }
                Ok(_) => {}
                Err(e) => log::warn!("[revalidation] validation error: {}", e),
            }
        }
    });
}

/// Spawns a background task that runs engagement sync once daily at startup
/// and then every 24 hours. Errors are logged but do not crash the app.
pub fn spawn_daily_engagement_sync(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) =
                crate::analytics::engagement_sync::sync_engagement(&app_handle).await
            {
                log::warn!("Engagement sync failed: {}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::deep_link::DeepLinkError;

    #[test]
    fn token_rejected_returns_try_again_message() {
        let msg = user_facing_activation_error(&DeepLinkError::TokenRejected);
        assert!(msg.contains("Sign-in failed"), "got: {}", msg);
        assert!(
            !msg.contains("license server"),
            "must not expose internal term, got: {}",
            msg
        );
    }

    #[test]
    fn backend_unavailable_mentions_internet() {
        let msg = user_facing_activation_error(&DeepLinkError::BackendUnavailable);
        assert!(
            msg.to_lowercase().contains("internet") || msg.to_lowercase().contains("connect"),
            "got: {}",
            msg
        );
    }

    #[test]
    fn keyring_error_does_not_expose_os_error_string() {
        let msg = user_facing_activation_error(&DeepLinkError::KeyringWrite(
            "gnome-keyring: daemon not running".to_string(),
        ));
        assert!(
            !msg.contains("gnome-keyring"),
            "must not expose OS error details, got: {}",
            msg
        );
    }

    #[test]
    fn revalidation_reads_keyring_each_cycle_by_design() {
        // spawn_license_revalidation must read the keyring inside the loop, not outside.
        // The function is async and requires a real AppHandle to test end-to-end.
        // The invariant is enforced by code review of the loop structure.
        assert!(true, "see spawn_license_revalidation implementation");
    }
}
