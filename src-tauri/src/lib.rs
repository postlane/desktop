// SPDX-License-Identifier: BUSL-1.1

pub mod account_config;
pub mod command_registry;
pub mod account_id_store;
pub mod account_name_store;
pub mod analytics;
pub mod app_lifecycle;
pub mod app_state;
pub mod app_state_ops;
pub mod app_state_types;
pub mod commands;
pub mod config_local_write;
pub mod config_merge;
pub mod connect_repo;
pub mod connected_platforms;
pub mod credential_migration;
pub mod credential_provider_list;
pub mod credential_repo_sync;
pub mod deep_link_routing;
pub mod draft_edits;
pub mod draft_output;
pub mod draft_post_scanner;
pub mod draft_queries;
pub mod draft_schedule;
pub mod engagement_cache;
pub mod folder_lookup;
pub mod git_url_parser;
pub mod github_app;
pub mod http_server;
pub mod init;
pub mod instance_guard;
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
pub mod platform_config_sync;
pub mod platform_constants;
pub mod poll_routing;
pub mod post_approval;
pub mod post_dismiss;
pub mod post_editor;
pub mod post_export;
pub mod post_image_unsplash;
pub mod post_io;
pub mod post_meta;
pub mod post_mutations;
pub mod post_queries;
pub mod post_redraft;
pub mod post_retry;
pub mod post_schedule;
pub mod project_api;
pub mod project_billing;
pub mod project_cache;
pub mod project_config_ops;
pub mod project_delete;
pub mod project_lifecycle;
pub mod project_registry;
pub mod project_validation;
pub mod project_voice_guide;
pub mod provider_orgs;
pub mod providers;
pub mod published_queries;
pub mod repo_connection_status;
pub mod repo_discovery;
pub mod repo_init_config;
pub mod repo_mgmt;
pub mod repo_project_filter;
pub mod repo_queries;
pub mod repo_scheduler_config;
pub mod schedule_time;
pub mod scheduler_account_sync;
pub mod scheduler_credential_writer;
pub mod scheduler_credentials;
pub mod scheduling;
pub mod security;
pub mod startup_sync;
pub mod storage;
pub mod telemetry;
pub mod telemetry_commands;
pub mod tray;
pub mod types;
pub mod unsplash_search;
pub mod upload_post_account;
pub mod voice_guide_versions;
pub mod watcher;
pub mod webhook_poller;
pub mod wizard_state;
pub mod repos_migration;
pub mod workspace;
pub mod workspace_add;
pub mod workspace_confirm;
pub mod workspace_entry;
pub mod workspace_history;
pub mod workspace_repos;
pub mod workspace_rescan;
pub mod account_deletion;
pub mod account_deletion_commands;
pub mod account_deletion_steps;
pub mod credential_store;
pub mod ssrf_validation;
pub mod workspace_disconnect;
pub mod workspace_disconnect_commands;
pub mod workspace_migration;
pub mod workspace_migration_commands;
pub mod workspace_migration_execute;
pub mod workspace_path_check;

#[cfg(test)]
pub mod test_fixtures;

use app_state::AppState;
use tauri::Manager;

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
fn get_local_server_port(state: tauri::State<AppState>) -> Result<u16, String> {
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

fn setup_app(app: &mut tauri::App, repos: storage::ReposConfig) -> Result<(), Box<dyn std::error::Error>> {
    instance_guard::exit_if_duplicate_instance();
    let handle = app.handle().clone();
    app_lifecycle::spawn_http_server(handle.clone(), repos.clone())?;
    let app_state: tauri::State<AppState> = app.state();
    let repos_ref = app_state.repos.lock().map_err(|e| e.to_string())?;
    let active_repos: Vec<storage::Repo> = repos_ref.repos.clone();
    drop(repos_ref);
    startup_sync::sync_all_repos_on_startup(&active_repos, &|_pid| false, &|_key| false);
    app_lifecycle::spawn_startup_account_sync(handle.clone(), active_repos);
    app_lifecycle::spawn_license_revalidation(handle.clone());
    app_lifecycle::spawn_telemetry_flush(handle.clone());
    app_lifecycle::spawn_daily_engagement_sync(handle.clone());
    tray::setup_tray(&handle)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let repos_path = init::postlane_dir()
        .map(|d| d.join("repos.json"))
        .unwrap_or_else(|_| std::path::PathBuf::from("/dev/null"));

    // Migrate repos.json from v1 → v2 schema before loading (22.1.1).
    // Must run before read_repos_with_recovery so the loaded config is always v2.
    if let Ok(postlane) = init::postlane_dir() {
        let app_state_path = postlane.join("app_state.json");
        if let Err(e) = repos_migration::migrate_repos_to_v2(&repos_path, &app_state_path) {
            log::warn!("[startup] repos.json migration failed (non-fatal): {}", e);
        }
    }

    let repos = storage::read_repos_with_recovery(&repos_path)
        .unwrap_or_else(|_| storage::ReposConfig::default());
    let state = AppState::new(repos.clone());
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(state)
        .setup(move |app| setup_app(app, repos.clone()))
        .invoke_handler(command_registry::all_commands())
        .run(tauri::generate_context!())
        .expect("error while running postlane");
}
