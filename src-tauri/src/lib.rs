// SPDX-License-Identifier: BUSL-1.1

pub mod account_config;
pub mod app_lifecycle;
pub mod project_api;
pub mod config_merge;
pub mod instance_guard;
pub mod connect_repo;
pub mod analytics;
pub mod app_state;
pub mod commands;
pub mod deep_link_routing;
pub mod github_app;
pub mod draft_edits;
pub mod draft_output;
pub mod draft_queries;
pub mod draft_schedule;
pub mod engagement_cache;
pub mod http_server;
pub mod init;
pub mod license;
pub mod mastodon_oauth;
pub mod model_stats;
pub mod nav_commands;
pub mod og_image;
pub mod org_published;
pub mod parser;
pub mod platform_constants;
pub mod post_approval;
pub mod post_meta;
pub mod post_editor;
pub mod post_io;
pub mod post_mutations;
pub mod post_schedule;
pub mod post_export;
pub mod post_ops;
pub mod project_cache;
pub mod project_registry;
pub mod provider_orgs;
pub mod project_validation;
pub mod providers;
pub mod published_queries;
pub mod repo_mgmt;
pub mod repo_project_filter;
pub mod repo_queries;
pub mod scheduler_credentials;
pub mod scheduler_profiles;
pub mod security;
pub mod scheduling;
pub mod storage;
pub mod telemetry;
pub mod telemetry_commands;
pub mod tray;
pub mod types;
pub mod voice_guide_versions;
pub mod watcher;
pub mod poll_routing;
pub mod webhook_poller;
pub mod workspace;

#[cfg(test)]
pub mod test_fixtures;

use app_state::AppState;
use tauri::Manager;

mod scheduling_commands {
    use crate::scheduling::usage_tracker::{get_known_limit, get_usage, UsageRecord};
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct UsageResponse {
        pub provider: String,
        pub count: u32,
        pub month: u32,
        pub year: u32,
        pub limit: Option<u32>,
    }

    /// Returns the current usage and known limit for a scheduler provider.
    /// Used by Settings → Scheduler to display usage inline.
    #[tauri::command]
    pub fn get_scheduler_usage(provider: String) -> Result<UsageResponse, String> {
        let record: UsageRecord = get_usage(&provider)?;
        Ok(UsageResponse {
            provider: record.provider,
            count: record.count,
            month: record.month,
            year: record.year,
            limit: get_known_limit(&provider),
        })
    }
}

/// Initialises `AppState`, tray, close-to-tray behaviour, and starts background
/// tasks (provider init + HTTP server). Called inside `tauri::Builder::setup`.
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let repos_path = init::postlane_dir()?.join("repos.json");
    let (repos_config, repos_was_corrupted) = storage::read_repos_checked(&repos_path)
        .map_err(|e| format!("Failed to load repos: {:?}", e))?;

    let libsecret_available = commands::check_libsecret_availability(Some(app.handle().clone()));

    let app_state = AppState::new(repos_config.clone());
    {
        let mut flag = app_state.libsecret_available.lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag = Some(libsecret_available);
    }
    app.manage(app_state);

    if repos_was_corrupted {
        use tauri::Emitter;
        let _ = app.emit("repos-config-corrupted", ());
    }

    tray::setup_tray(app.handle())
        .map_err(|e| format!("Failed to set up tray: {}", e))?;

    register_close_to_tray(app);
    register_deep_link_handler(app.handle().clone());
    app_lifecycle::spawn_http_server(app.handle().clone(), repos_config)?;
    app_lifecycle::spawn_daily_engagement_sync(app.handle().clone());
    app_lifecycle::spawn_telemetry_flush(app.handle().clone());
    app_lifecycle::spawn_license_revalidation(app.handle().clone());

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

/// Registers the `postlane://` deep link handler.
/// Emits `github:app-installed` or `github:install-error` based on the OAuth callback URL.
fn handle_oauth_callback(url: &str, handle: &tauri::AppHandle) {
    use tauri::Emitter;
    log::info!("Deep link: {}", deep_link_routing::log_safe_url(url));
    match deep_link_routing::installation_id_from_url(url) {
        Some(id) => {
            let _ = handle.emit("github:app-installed", serde_json::json!({ "installation_id": id }));
        }
        None => {
            log::warn!("OAuth callback missing valid installation_id");
            let _ = handle.emit("github:install-error", serde_json::json!({ "message": "Installation ID missing or invalid in callback URL." }));
        }
    }
}

/// Routes by host+path via `deep_link_routing::classify` — query strings are never logged.
/// `postlane://activate` → license activation. Stubs logged at `info!`. Unknown → `warn!`.
fn register_deep_link_handler(app_handle: tauri::AppHandle) {
    use deep_link_routing::DeepLinkPath;
    use tauri::Emitter;
    use tauri_plugin_deep_link::DeepLinkExt;
    use tauri_plugin_keyring::KeyringExt;

    app_handle.clone().deep_link().on_open_url(move |event| {
        for url in event.urls() {
            let url_str = url.to_string();
            let handle = app_handle.clone();
            match deep_link_routing::classify(&url_str) {
                DeepLinkPath::Draft => {
                    log::info!("Deep link: {}", deep_link_routing::log_safe_url(&url_str));
                }
                DeepLinkPath::OauthCallback => {
                    handle_oauth_callback(&url_str, &handle);
                }
                DeepLinkPath::Unknown { path } => {
                    log::warn!("Unknown deep link path: {}", path);
                }
                DeepLinkPath::Activate => {
                    tauri::async_runtime::spawn(async move {
                        let token = match license::deep_link::parse_activate_url(&url_str) {
                            Ok(t) => t,
                            Err(e) => {
                                log::warn!("License deep link rejected: {}", e);
                                let _ = handle.emit("license:error", serde_json::json!({ "message": app_lifecycle::user_facing_activation_error(&e) }));
                                return;
                            }
                        };
                        let client = providers::scheduling::build_client();
                        let keyring_handle = handle.clone();
                        let result = license::deep_link::handle_activate(
                            &token,
                            &client,
                            license::POSTLANE_API_BASE,
                            move |t| keyring_handle.keyring().set_password("postlane", "license", t)
                                .map_err(|e| e.to_string()),
                            license::validator::write_license_cache,
                        )
                        .await;
                        match result {
                            Ok(display_name) => {
                                log::info!("License activated for {}", display_name);
                                let _ = handle.emit("license:activated", serde_json::json!({ "display_name": display_name }));
                            }
                            Err(e) => {
                                log::warn!("License activation failed: {}", e);
                                let _ = handle.emit("license:error", serde_json::json!({ "message": app_lifecycle::user_facing_activation_error(&e) }));
                            }
                        }
                    });
                }
            }
        }
    });
}

/// Core logic for resolving the local HTTP server port.
/// Tries the port file first; falls back to `in_memory` if the file is
/// absent or unparseable (e.g. deleted during a hot-reload cycle).
pub fn get_local_server_port_impl(port_path: &std::path::Path, in_memory: Option<u16>) -> Result<u16, String> {
    if let Ok(content) = std::fs::read_to_string(port_path) {
        if let Ok(port) = content.trim().parse::<u16>() {
            return Ok(port);
        }
    }
    in_memory.ok_or_else(|| "HTTP server port not available — server may not have started yet".to_string())
}

/// Reads the local HTTP server port, falling back to the in-memory copy
/// stored in AppState if the port file is absent or corrupt.
#[tauri::command]
fn get_local_server_port(state: tauri::State<crate::app_state::AppState>) -> Result<u16, String> {
    let port_path = init::postlane_dir()?.join("port");
    let in_memory = state.http_port.lock().ok().and_then(|g| *g);
    get_local_server_port_impl(&port_path, in_memory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_from_file_when_present() {
        let dir = std::env::temp_dir().join("postlane_port_test_file");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("port");
        std::fs::write(&path, "47312").unwrap();
        assert_eq!(get_local_server_port_impl(&path, None).unwrap(), 47312);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_port_from_file_takes_priority_over_in_memory() {
        let dir = std::env::temp_dir().join("postlane_port_test_priority");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("port");
        std::fs::write(&path, "47312").unwrap();
        let result = get_local_server_port_impl(&path, Some(9999));
        assert_eq!(result.unwrap(), 47312, "file port must win over in-memory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_port_falls_back_to_in_memory_when_file_missing() {
        let dir = std::env::temp_dir().join("postlane_port_test_fallback");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("port"); // does not exist
        let result = get_local_server_port_impl(&path, Some(47312));
        assert_eq!(result.unwrap(), 47312);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_port_falls_back_to_in_memory_when_file_corrupt() {
        let dir = std::env::temp_dir().join("postlane_port_test_corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("port");
        std::fs::write(&path, "not_a_port_number").unwrap();
        let result = get_local_server_port_impl(&path, Some(47312));
        assert_eq!(result.unwrap(), 47312, "corrupt file must fall back to in-memory");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_port_returns_error_when_file_missing_and_no_in_memory() {
        let dir = std::env::temp_dir().join("postlane_port_test_nofile_nomem");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("port"); // does not exist
        let result = get_local_server_port_impl(&path, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn add_plugins(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| tray::handle_menu_event(app, event.id.0.as_str()))
        .setup(|app| setup_app(app))
}

fn build_tauri_app() -> tauri::Builder<tauri::Wry> {
    add_plugins(tauri::Builder::default()).invoke_handler(tauri::generate_handler![
        repo_queries::get_repos,
        draft_queries::get_all_drafts,
        published_queries::get_repo_published, published_queries::get_all_published,
        org_published::get_org_published,
        model_stats::get_model_stats,
        nav_commands::get_app_version, nav_commands::get_autostart_enabled,
        nav_commands::get_attribution, nav_commands::set_attribution,
        account_config::list_profiles_for_repo, account_config::save_account_id,
        account_config::get_account_ids,
        nav_commands::read_app_state_command, nav_commands::save_app_state_command,
        nav_commands::get_app_state,
        app_state::set_wizard_completed,
        app_state::set_default_post_time,
        post_ops::get_drafts, post_approval::approve_post,
        post_ops::get_post_content, post_ops::dismiss_post, post_ops::delete_post,
        post_ops::retry_post, post_ops::queue_redraft, post_ops::cancel_redraft,
        repo_mgmt::add_repo, repo_mgmt::remove_repo, repo_mgmt::set_repo_active,
        repo_project_filter::list_repos_for_project, repo_project_filter::unregister_repo,
        repo_mgmt::check_repo_health, repo_mgmt::update_repo_path,
        repo_mgmt::update_scheduler_config,
        scheduler_credentials::get_libsecret_status, scheduler_credentials::has_scheduler_configured,
        scheduler_credentials::has_provider_credential, scheduler_credentials::list_connected_providers,
        scheduler_credentials::save_scheduler_credential,
        scheduler_credentials::get_scheduler_credential, scheduler_credentials::delete_scheduler_credential,
        scheduler_profiles::remove_scheduler_credential, scheduler_profiles::list_scheduler_profiles,
        scheduler_profiles::add_scheduler_credential,
        scheduler_credentials::save_repo_scheduler_key, scheduler_credentials::remove_repo_scheduler_key,
        scheduler_credentials::get_per_repo_scheduler_key,
        commands::test_scheduler, commands::cancel_post_command, commands::get_queue_command,
        post_export::export_history_csv,
        post_editor::update_post_content, post_editor::update_post_image,
        og_image::fetch_og_image, og_image::validate_url_safe,
        provider_orgs::fetch_avatar_bytes, provider_orgs::list_provider_orgs,
        github_app::check_github_app_installed,
        post_schedule::update_post_schedule,
        mastodon_oauth::get_mastodon_char_limit, mastodon_oauth::get_mastodon_connected_instance,
        mastodon_oauth::register_mastodon_app, mastodon_oauth::exchange_mastodon_code,
        mastodon_oauth::disconnect_mastodon,
        analytics::client::get_site_token, analytics::client::get_post_analytics,
        telemetry_commands::get_telemetry_consent, telemetry_commands::set_telemetry_consent,
        scheduling_commands::get_scheduler_usage,
        license::get_license_signed_in,
        license::sign_out,
        license::get_license_display_name,
        nav_commands::get_watcher_status,
        get_local_server_port,
        project_registry::check_project_status, project_registry::check_billing_gate,
        project_registry::create_project, project_registry::update_project_org_login,
        project_registry::write_project_id_to_config,
        project_registry::register_repo_with_project, project_registry::save_project_voice_guide,
        project_registry::get_project_voice_guide,
        project_registry::get_repo_remote_name, project_registry::read_project_id_from_path,
        project_registry::list_projects, project_registry::delete_project,
        connect_repo::connect_repo_from_desktop,
        draft_edits::save_post_draft,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = instance_guard::check_single_instance() {
        eprintln!("{}", e);
        instance_guard::show_alert_and_exit(&e);
    }
    build_tauri_app()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

