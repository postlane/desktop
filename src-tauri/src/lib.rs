// SPDX-License-Identifier: BUSL-1.1

pub mod account_config;
pub mod analytics;
pub mod app_state;
pub mod commands;
pub mod draft_queries;
pub mod engagement_cache;
pub mod http_server;
pub mod init;
pub mod license;
pub mod mastodon_oauth;
pub mod model_stats;
pub mod nav_commands;
pub mod parser;
pub mod post_approval;
pub mod post_editor;
pub mod post_export;
pub mod post_ops;
pub mod providers;
pub mod published_queries;
pub mod repo_mgmt;
pub mod repo_queries;
pub mod scheduler_credentials;
pub mod storage;
pub mod telemetry;
pub mod tray;
pub mod types;
pub mod watcher;

use std::sync::Arc;
use app_state::AppState;
use tauri::Manager;

mod telemetry_commands {
    use crate::app_state::AppState;
    use crate::app_state::{read_app_state, write_app_state};
    use tauri::State;

    /// Returns whether the user has given telemetry consent.
    #[tauri::command]
    pub fn get_telemetry_consent(state: State<AppState>) -> Result<bool, String> {
        let _ = state;
        Ok(read_app_state().telemetry_consent)
    }

    /// Saves the user's telemetry consent choice and marks consent_asked = true.
    #[tauri::command]
    pub fn set_telemetry_consent(consent: bool) -> Result<(), String> {
        let mut s = read_app_state();
        s.telemetry_consent = consent;
        s.consent_asked = true;
        write_app_state(&s)
    }
}

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Initialises `AppState`, tray, close-to-tray behaviour, and starts background
/// tasks (provider init + HTTP server). Called inside `tauri::Builder::setup`.
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let repos_path = init::postlane_dir()?.join("repos.json");
    let repos_config = storage::read_repos_with_recovery(&repos_path)
        .map_err(|e| format!("Failed to load repos: {:?}", e))?;

    let libsecret_available = commands::check_libsecret_availability(Some(app.handle().clone()));

    let app_state = AppState::new(repos_config.clone());
    {
        let mut flag = app_state.libsecret_available.lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag = Some(libsecret_available);
    }
    app.manage(app_state);

    tray::setup_tray(app.handle())
        .map_err(|e| format!("Failed to set up tray: {}", e))?;

    register_close_to_tray(app);
    spawn_eager_provider_init(app.handle().clone());
    spawn_http_server(app.handle().clone(), repos_config)?;
    spawn_daily_engagement_sync(app.handle().clone());
    spawn_telemetry_flush(app.handle().clone());

    Ok(())
}

/// Registers a window event handler that hides the main window on close
/// instead of quitting. The app lives in the tray.
fn register_close_to_tray(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("main") {
        let win = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = win.hide();
            }
        });
    }
}

/// Spawns an async task that eagerly initialises the scheduler provider when
/// credentials are already stored. Eliminates first-send latency.
fn spawn_eager_provider_init(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state: tauri::State<AppState> = app_handle.state();
        if let Err(e) = commands::eager_init_provider_if_configured(&state, Some(&app_handle)).await {
            log::warn!("Eager provider initialization failed: {}", e);
        }
    });
}

/// Generates the session token and starts the HTTP server on port 47312.
fn spawn_http_server(
    _app_handle: tauri::AppHandle,
    repos_config: storage::ReposConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let token = http_server::generate_and_write_token()?;
    let repos_arc = Arc::new(tokio::sync::Mutex::new(repos_config));
    let server_state = http_server::ServerState { token, repos: repos_arc };

    tauri::async_runtime::spawn(async move {
        match http_server::start_server(server_state, 47312).await {
            Ok(port) => {
                if let Err(e) = http_server::write_port_file(port) {
                    log::error!("Failed to write port file: {}", e);
                } else {
                    log::info!("HTTP server started on port {}", port);
                }
            }
            Err(e) => {
                log::error!("Failed to start HTTP server: {}", e);
            }
        }
    });

    Ok(())
}

/// Spawns a background task that flushes queued telemetry every 30 minutes.
/// No-ops if consent is false or if not signed in.
fn spawn_telemetry_flush(app_handle: tauri::AppHandle) {
    use tauri_plugin_keyring::KeyringExt;
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        interval.tick().await; // discard the immediate first tick
        loop {
            interval.tick().await;
            if !crate::app_state::read_app_state().telemetry_consent { continue; }
            let token = match app_handle.keyring().get_password("postlane", "license") {
                Ok(Some(t)) => t,
                _ => continue,
            };
            let state: tauri::State<AppState> = app_handle.state();
            state.telemetry.flush(&token).await;
        }
    });
}

/// Spawns a background task that runs engagement sync once daily at startup
/// and then every 24 hours. Errors are logged but do not crash the app.
fn spawn_daily_engagement_sync(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = analytics::engagement_sync::sync_engagement(&app_handle).await {
                log::warn!("Engagement sync failed: {}", e);
            }
            tokio::time::sleep(std::time::Duration::from_secs(86400)).await;
        }
    });
}

fn build_tauri_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| {
            tray::handle_menu_event(app, event.id.0.as_str());
        })
        .setup(|app| setup_app(app))
        .invoke_handler(tauri::generate_handler![
            greet,
            repo_queries::get_repos,
            draft_queries::get_all_drafts,
            published_queries::get_repo_published,
            published_queries::get_all_published,
            model_stats::get_model_stats,
            nav_commands::get_app_version,
            nav_commands::get_autostart_enabled,
            nav_commands::get_attribution,
            nav_commands::set_attribution,
            account_config::list_profiles_for_repo,
            account_config::save_account_id,
            account_config::get_account_ids,
            nav_commands::read_app_state_command,
            nav_commands::save_app_state_command,
            post_ops::get_drafts,
            post_approval::approve_post,
            post_ops::get_post_content,
            post_ops::dismiss_post,
            post_ops::delete_post,
            post_ops::retry_post,
            post_ops::queue_redraft,
            post_ops::cancel_redraft,
            repo_mgmt::add_repo,
            repo_mgmt::remove_repo,
            repo_mgmt::set_repo_active,
            repo_mgmt::check_repo_health,
            scheduler_credentials::get_libsecret_status,
            scheduler_credentials::save_scheduler_credential,
            scheduler_credentials::get_scheduler_credential,
            scheduler_credentials::delete_scheduler_credential,
            commands::test_scheduler,
            commands::cancel_post_command,
            commands::get_queue_command,
            post_export::export_history_csv,
            repo_mgmt::update_repo_path,
            post_editor::update_post_content,
            post_editor::update_post_image,
            post_editor::fetch_og_image,
            mastodon_oauth::get_mastodon_char_limit,
            mastodon_oauth::get_mastodon_connected_instance,
            mastodon_oauth::register_mastodon_app,
            mastodon_oauth::exchange_mastodon_code,
            mastodon_oauth::disconnect_mastodon,
            analytics::client::get_site_token,
            analytics::client::get_post_analytics,
            telemetry_commands::get_telemetry_consent,
            telemetry_commands::set_telemetry_consent,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = check_single_instance() {
        eprintln!("{}", e);
        show_alert_and_exit(&e);
    }
    build_tauri_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Checks if another instance is already running
/// Returns Err if instance is running, Ok if not
fn check_single_instance() -> Result<(), String> {
    let port_path = init::postlane_dir()?.join("port");

    if !port_path.exists() {
        return Ok(());
    }

    // Port file exists - check if instance is responsive
    let port_str = std::fs::read_to_string(&port_path)
        .map_err(|e| format!("Failed to read port file: {}", e))?;

    let port: u16 = port_str
        .trim()
        .parse()
        .map_err(|e| format!("Invalid port in port file: {}", e))?;

    // Try to connect to /health endpoint
    let url = format!("http://127.0.0.1:{}/health", port);

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;

    let health_check = rt.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(200))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Health check failed: {}", e))
    });

    match health_check {
        Ok(_) => {
            // Instance is running
            Err(format!(
                "Postlane is already running on port {}. Close the existing instance first.",
                port
            ))
        }
        Err(_) => {
            // No response - stale port file
            log::warn!("Stale port file detected, cleaning up");
            let _ = std::fs::remove_file(&port_path);
            Ok(())
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn show_alert_and_exit(message: &str) {
    eprintln!("{}", message);
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn show_alert_and_exit(message: &str) {
    use std::process::Command;

    let _ = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "display dialog \"{}\" buttons {{\"OK\"}} default button \"OK\" with icon caution",
            message
        ))
        .output();

    std::process::exit(1);
}
