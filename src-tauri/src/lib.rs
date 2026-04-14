// SPDX-License-Identifier: BUSL-1.1

pub mod app_state;
pub mod engagement_cache;
pub mod http_server;
pub mod init;
pub mod parser;
pub mod storage;
pub mod types;
pub mod watcher;

use std::sync::Arc;

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
        .setup(|_app| {
            // Load repos from disk
            let repos_path = init::postlane_dir().join("repos.json");
            let repos_config = storage::read_repos_with_recovery(&repos_path)
                .map_err(|e| format!("Failed to load repos: {:?}", e))?;

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
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Checks if another instance is already running
/// Returns Err if instance is running, Ok if not
fn check_single_instance() -> Result<(), String> {
    let port_path = init::postlane_dir().join("port");

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
