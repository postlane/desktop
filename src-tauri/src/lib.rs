// SPDX-License-Identifier: BUSL-1.1

pub mod app_state;
pub mod commands;
pub mod engagement_cache;
pub mod http_server;
pub mod init;
pub mod nav_commands;
pub mod parser;
pub mod providers;
pub mod storage;
pub mod types;
pub mod watcher;

use std::sync::Arc;
use app_state::AppState;
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Check for existing instance before starting
    if let Err(e) = check_single_instance() {
        eprintln!("{}", e);
        show_alert_and_exit(&e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load repos from disk
            let repos_path = init::postlane_dir()?.join("repos.json");
            let repos_config = storage::read_repos_with_recovery(&repos_path)
                .map_err(|e| format!("Failed to load repos: {:?}", e))?;

            // Check libsecret availability (Linux only)
            let libsecret_available = commands::check_libsecret_availability(Some(app.handle().clone()));

            // Create AppState
            let app_state = AppState::new(repos_config.clone());

            // Set libsecret availability flag
            {
                let mut flag = app_state.libsecret_available.lock()
                    .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
                *flag = Some(libsecret_available);
            }

            // Manage AppState
            app.manage(app_state);

            // Eagerly instantiate provider if credentials exist
            // This eliminates first-send delay when user already has credentials configured
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Get AppState from app handle
                let state: tauri::State<AppState> = app_handle.state();
                if let Err(e) = commands::eager_init_provider_if_configured(&state, Some(&app_handle)).await {
                    log::warn!("Eager provider initialization failed: {}", e);
                }
            });

            // Generate session token
            let token = http_server::generate_and_write_token()?;

            // Start HTTP server
            let repos_arc = Arc::new(tokio::sync::Mutex::new(repos_config));
            let server_state = http_server::ServerState {
                token,
                repos: repos_arc.clone(),
            };

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
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            nav_commands::get_repos,
            nav_commands::get_all_drafts,
            nav_commands::get_repo_published,
            nav_commands::get_all_published,
            nav_commands::get_model_stats,
            nav_commands::read_app_state_command,
            nav_commands::save_app_state_command,
            commands::get_drafts,
            commands::approve_post,
            commands::dismiss_post,
            commands::retry_post,
            commands::add_repo,
            commands::remove_repo,
            commands::set_repo_active,
            commands::check_repo_health,
            commands::get_libsecret_status,
            commands::save_scheduler_credential,
            commands::get_scheduler_credential,
            commands::delete_scheduler_credential,
            commands::test_scheduler,
            commands::cancel_post_command,
            commands::get_queue_command,
            commands::export_history_csv,
            commands::update_repo_path,
        ])
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
