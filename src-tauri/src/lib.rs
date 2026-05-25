// SPDX-License-Identifier: BUSL-1.1

pub mod account_config;
pub mod connected_platforms;
pub mod upload_post_account;
pub mod app_lifecycle;
pub mod config_local_write;
pub mod project_api;
pub mod config_merge;
pub mod schedule_time;
pub mod instance_guard;
pub mod connect_repo;
pub mod credential_migration;
pub mod credential_repo_sync;
pub mod analytics;
pub mod app_state;
pub mod app_state_ops;
pub mod app_state_types;
pub mod commands;
pub mod deep_link_routing;
pub mod github_app;
pub mod repo_discovery;
pub mod draft_edits;
pub mod draft_output;
pub mod draft_post_scanner;
pub mod draft_queries;
pub mod draft_schedule;
pub mod engagement_cache;
pub mod http_server;
pub mod init;
pub mod license;
pub mod mastodon_app_registration;
pub mod mastodon_connection;
pub mod mastodon_token_exchange;
pub mod model_stats;
pub mod nav_commands;
pub mod og_image;
pub mod org_avatar;
pub mod org_published;
pub mod parser;
pub mod platform_constants;
pub mod post_approval;
pub mod post_meta;
pub mod post_editor;
pub mod post_image_unsplash;
pub mod post_io;
pub mod post_mutations;
pub mod post_schedule;
pub mod post_export;
pub mod post_dismiss;
pub mod post_queries;
pub mod post_redraft;
pub mod post_retry;
pub mod project_billing;
pub mod project_cache;
pub mod project_delete;
pub mod project_config_ops;
pub mod project_lifecycle;
pub mod startup_sync;
pub mod project_registry;
pub mod project_voice_guide;
pub mod provider_orgs;
pub mod project_validation;
pub mod providers;
pub mod published_queries;
pub mod repo_mgmt;
pub mod repo_project_filter;
pub mod repo_scheduler_config;
pub mod repo_queries;
pub mod scheduler_credentials;
pub mod security;
pub mod scheduling;
pub mod storage;
pub mod telemetry;
pub mod telemetry_commands;
pub mod tray;
pub mod types;
pub mod voice_guide_versions;
pub mod watcher;
pub mod wizard_state;
pub mod poll_routing;
pub mod webhook_poller;
pub mod workspace;
pub mod unsplash_search;

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

    // Run one-time credential migration (bare → project-scoped keyring keys).
    // Runs silently; errors are logged but never surfaced to the user.
    {
        let migration_state = app.state::<AppState>();
        if let Err(e) = credential_migration::run_v1(app.handle(), &migration_state) {
            log::warn!("[setup] credential migration failed: {}", e);
        }
    }

    // Sync connected_platforms for all repos from the current keyring state.
    // Credentials may have been added or removed between sessions; this ensures
    // the field in config.json reflects reality before any draft command runs.
    {
        use tauri_plugin_keyring::KeyringExt;
        let handle = app.handle().clone();
        startup_sync::sync_all_repos_on_startup(
            &repos_config.repos,
            &|project_id| {
                use mastodon_connection::{active_instance_key, KEYRING_SERVICE};
                handle.keyring()
                    .get_password(KEYRING_SERVICE, &active_instance_key(project_id))
                    .unwrap_or(None)
                    .is_some()
            },
            &|key| {
                handle.keyring()
                    .get_password("postlane", key)
                    .unwrap_or(None)
                    .is_some()
            },
        );
    }

    if repos_was_corrupted {
        use tauri::Emitter;
        let _ = app.emit("repos-config-corrupted", ());
    }

    tray::setup_tray(app.handle())
        .map_err(|e| format!("Failed to set up tray: {}", e))?;

    register_close_to_tray(app);
    register_deep_link_handler(app.handle().clone());
    app_lifecycle::spawn_http_server(app.handle().clone(), repos_config.clone())?;
    app_lifecycle::spawn_daily_engagement_sync(app.handle().clone());
    app_lifecycle::spawn_telemetry_flush(app.handle().clone());
    app_lifecycle::spawn_license_revalidation(app.handle().clone());

    // Restart watchers for all repos that were already registered before this launch.
    // Watchers are started at registration time but are not persisted across restarts,
    // so without this any post drafted after a restart would never appear in the queue.
    let handle = app.handle().clone();
    let state = app.state::<AppState>();
    for repo in repos_config.repos.iter().filter(|r| r.active) {
        repo_mgmt::start_repo_watcher(&repo.id, &repo.path, &state, handle.clone());
    }

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

/// Dispatches a single `postlane://` URL through the appropriate handler.
/// Called from both the `on_open_url` listener and the single-instance callback.
pub(crate) fn dispatch_deep_link(url: String, handle: tauri::AppHandle) {
    use deep_link_routing::DeepLinkPath;
    use tauri::Emitter;
    use tauri_plugin_keyring::KeyringExt;
    match deep_link_routing::classify(&url) {
        DeepLinkPath::Draft => {
            log::info!("Deep link: {}", deep_link_routing::log_safe_url(&url));
        }
        DeepLinkPath::OauthCallback => {
            handle_oauth_callback(&url, &handle);
        }
        DeepLinkPath::Unknown { path } => {
            log::warn!("Unknown deep link path: {}", path);
        }
        DeepLinkPath::Activate => {
            tauri::async_runtime::spawn(async move {
                let token = match license::deep_link::parse_activate_url(&url) {
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
                    &token, &client, license::POSTLANE_API_BASE,
                    move |t| keyring_handle.keyring().set_password("postlane", "license", t)
                        .map_err(|e| e.to_string()),
                    license::validator::write_license_cache,
                ).await;
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

/// Routes by host+path via `deep_link_routing::classify` — query strings are never logged.
fn register_deep_link_handler(app_handle: tauri::AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;
    app_handle.clone().deep_link().on_open_url(move |event| {
        for url in event.urls() {
            dispatch_deep_link(url.to_string(), app_handle.clone());
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
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("port");
        std::fs::write(&path, "47312").unwrap();
        assert_eq!(get_local_server_port_impl(&path, None).unwrap(), 47312);
    }

    #[test]
    fn test_port_from_file_takes_priority_over_in_memory() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("port");
        std::fs::write(&path, "47312").unwrap();
        let result = get_local_server_port_impl(&path, Some(9999));
        assert_eq!(result.unwrap(), 47312, "file port must win over in-memory");
    }

    #[test]
    fn test_port_falls_back_to_in_memory_when_file_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("port"); // does not exist
        let result = get_local_server_port_impl(&path, Some(47312));
        assert_eq!(result.unwrap(), 47312);
    }

    #[test]
    fn test_port_falls_back_to_in_memory_when_file_corrupt() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("port");
        std::fs::write(&path, "not_a_port_number").unwrap();
        let result = get_local_server_port_impl(&path, Some(47312));
        assert_eq!(result.unwrap(), 47312, "corrupt file must fall back to in-memory");
    }

    #[test]
    fn test_port_returns_error_when_file_missing_and_no_in_memory() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("port"); // does not exist
        let result = get_local_server_port_impl(&path, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }
}

fn add_plugins(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // A second instance was launched — bring the existing window to front.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            // On Windows, the deep link URL arrives as a process argument in the
            // second instance. Re-dispatch it through the existing handler.
            if let Some(url) = deep_link_routing::deep_link_from_args(&argv) {
                dispatch_deep_link(url, app.clone());
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .on_menu_event(|app, event| tray::handle_menu_event(app, event.id.0.as_str()))
        .setup(|app| setup_app(app))
}

fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        repo_queries::get_repos, repo_queries::has_active_repos, draft_queries::get_all_drafts, draft_queries::get_all_drafts_count,
        published_queries::get_repo_published, published_queries::get_all_published, org_published::get_org_published,
        model_stats::get_model_stats, nav_commands::get_app_version, nav_commands::get_autostart_enabled,
        nav_commands::get_attribution, nav_commands::set_attribution, nav_commands::get_watcher_status,
        account_config::save_account_id, account_config::get_account_ids,
        upload_post_account::validate_upload_post_username,
        connected_platforms::list_connected_platforms,
        nav_commands::read_app_state_command, nav_commands::save_app_state_command, nav_commands::get_app_state,
        app_state_ops::set_wizard_completed, app_state_ops::set_default_post_time,
        post_queries::get_drafts, post_approval::approve_post, post_queries::get_post_content,
        post_dismiss::dismiss_post, post_dismiss::delete_post, post_retry::retry_post,
        post_redraft::queue_redraft, post_redraft::cancel_redraft,
        repo_mgmt::add_repo, repo_mgmt::remove_repo, repo_mgmt::set_repo_active,
        repo_mgmt::check_repo_health, repo_mgmt::update_repo_path,
        repo_scheduler_config::update_scheduler_config,
        repo_project_filter::list_repos_for_project, repo_project_filter::unregister_repo,
        scheduler_credentials::get_libsecret_status, scheduler_credentials::list_connected_providers,
        scheduler_credentials::save_scheduler_credential, scheduler_credentials::delete_scheduler_credential,
        commands::cancel_post_command, commands::get_queue_command,
        post_export::export_history_csv, post_editor::update_post_content, post_editor::update_post_image,
        post_image_unsplash::update_post_image_unsplash,
        og_image::fetch_og_image, og_image::validate_url_safe,
        org_avatar::fetch_avatar_bytes,
        provider_orgs::list_provider_orgs, provider_orgs::list_linked_providers,
        github_app::check_github_app_installed, github_app::backfill_project_org_login,
        github_app::list_github_app_repos, github_app::disconnect_github_app,
        repo_discovery::discover_repos,
        post_schedule::update_post_schedule,
        mastodon_connection::get_mastodon_char_limit, mastodon_connection::get_mastodon_connected_instance,
        mastodon_connection::get_mastodon_connected_account, mastodon_connection::disconnect_mastodon,
        mastodon_app_registration::register_mastodon_app,
        mastodon_token_exchange::exchange_mastodon_code,
        analytics::client::get_site_token, analytics::client::get_post_analytics,
        telemetry_commands::get_telemetry_consent, telemetry_commands::set_telemetry_consent,
        scheduling_commands::get_scheduler_usage,
        license::get_license_signed_in, license::sign_out, license::get_license_display_name,
        get_local_server_port,
        project_billing::check_project_status, project_billing::check_billing_gate,
        project_lifecycle::create_project, project_lifecycle::update_project_org_login,
        project_lifecycle::register_repo_with_project, project_lifecycle::list_projects,
        project_delete::delete_project,
        project_config_ops::write_project_id_to_config, project_config_ops::get_repo_remote_name,
        project_config_ops::read_project_id_from_path,
        project_voice_guide::save_project_voice_guide, project_voice_guide::get_project_voice_guide,
        project_voice_guide::get_voice_guide_fields, project_voice_guide::sync_voice_guide_to_repos,
        connect_repo::connect_repo_from_desktop, draft_edits::save_post_draft,
        wizard_state::read_wizard_state, wizard_state::write_wizard_state, wizard_state::clear_wizard_state,
        unsplash_search::save_unsplash_key, unsplash_search::delete_unsplash_key,
        unsplash_search::has_unsplash_key, unsplash_search::search_unsplash,
        unsplash_search::trigger_unsplash_download,
    ])
}

fn build_tauri_app() -> tauri::Builder<tauri::Wry> {
    register_commands(add_plugins(tauri::Builder::default()))
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

