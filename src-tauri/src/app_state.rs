// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use crate::storage::ReposConfig;
use crate::telemetry::client::TelemetryClient;
use notify::RecommendedWatcher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};

pub use crate::app_state_types::{AppStateFile, DefaultPostTime, NavState, WindowState};

/// Application state shared across Tauri commands, watchers, and HTTP handlers
pub struct AppState {
    pub repos: Mutex<ReposConfig>,
    pub watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    /// Linux libsecret availability flag
    /// None = not checked yet, Some(true) = available, Some(false) = unavailable
    pub libsecret_available: Mutex<Option<bool>>,
    /// Opt-in product telemetry queue
    pub telemetry: Arc<TelemetryClient>,
    /// Tracks how many post-approval sends are currently in flight.
    /// Graceful shutdown polls this to zero before calling app.exit().
    pub in_flight_sends: Arc<AtomicUsize>,
    /// Port the local HTTP server is bound to. Set by spawn_http_server so
    /// get_local_server_port can fall back here if the port file is deleted.
    pub http_port: Mutex<Option<u16>>,
    /// Path to repos.json. Production code uses the real ~/.postlane/repos.json;
    /// tests use an isolated temp path so they cannot corrupt user data.
    pub repos_path: PathBuf,
}

impl AppState {
    /// Creates a new `AppState` with the given repos config.
    /// The repos path is resolved from `postlane_dir()` at construction time.
    pub fn new(repos: ReposConfig) -> Self {
        let repos_path = postlane_dir()
            .map(|d| d.join("repos.json"))
            .unwrap_or_else(|_| PathBuf::from("/dev/null"));
        Self::new_with_path(repos, repos_path)
    }

    /// Creates a new `AppState` with an explicit repos path.
    /// Use this in tests to point at an isolated temp file.
    pub fn new_with_path(repos: ReposConfig, repos_path: PathBuf) -> Self {
        Self {
            repos: Mutex::new(repos),
            watchers: Mutex::new(HashMap::new()),
            libsecret_available: Mutex::new(None),
            telemetry: Arc::new(TelemetryClient::new()),
            in_flight_sends: Arc::new(AtomicUsize::new(0)),
            http_port: Mutex::new(None),
            repos_path,
        }
    }
}

/// Returns the path to `app_state.json` in the postlane config directory.
pub fn app_state_path() -> Result<PathBuf, String> {
    Ok(postlane_dir()?.join("app_state.json"))
}

/// Reads app_state.json with silent fallback to defaults.
/// On missing, corrupt, or version-mismatched file: returns default state.
pub fn read_app_state() -> AppStateFile {
    let path = match app_state_path() {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Failed to get app state path: {}. Using defaults.", e);
            return AppStateFile::default();
        }
    };

    if !path.exists() {
        return AppStateFile::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<AppStateFile>(&content) {
            Ok(state) => {
                if state.version != 1 {
                    log::warn!(
                        "app_state.json version mismatch: found {}, expected 1. Using defaults.",
                        state.version
                    );
                    return AppStateFile::default();
                }
                state
            }
            Err(e) => {
                log::warn!("Failed to parse app_state.json: {}. Using defaults.", e);
                AppStateFile::default()
            }
        },
        Err(e) => {
            log::warn!("Failed to read app_state.json: {}. Using defaults.", e);
            AppStateFile::default()
        }
    }
}

/// Writes app_state.json atomically.
pub fn write_app_state(state: &AppStateFile) -> Result<(), String> {
    let path = app_state_path()?;
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize app state: {}", e))?;
    crate::init::atomic_write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write app state: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn get_test_mutex() -> &'static std::sync::Mutex<()> {
        crate::test_fixtures::app_state_mutex()
    }

    #[test]
    fn test_read_app_state_missing_file_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);
        assert!(!path.exists(), "File should not exist");
        let state = read_app_state();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
    }

    #[test]
    fn test_app_state_round_trip() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let state = AppStateFile {
            version: 1,
            window: WindowState { width: 1200, height: 800, x: 100, y: 200 },
            nav: NavState {
                last_view: "repo".to_string(),
                last_repo_id: Some("test-repo-id".to_string()),
                last_section: "published".to_string(),
                expanded_repos: vec!["repo1".to_string(), "repo2".to_string()],
            },
            wizard_completed: false,
            timezone: String::new(),
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
        };

        let path = dir.path().join("app_state.json");
        let json = serde_json::to_string_pretty(&state).expect("Failed to serialize");
        crate::init::atomic_write(&path, json.as_bytes()).expect("Failed to write");

        let content = fs::read_to_string(&path).expect("Failed to read");
        let loaded: AppStateFile = serde_json::from_str(&content).expect("Failed to deserialize");
        assert_eq!(loaded.window.width, 1200);
        assert_eq!(loaded.nav.last_view, "repo");
        assert_eq!(loaded.nav.expanded_repos.len(), 2);
    }

    #[test]
    fn test_app_state_new() {
        let repos = crate::storage::ReposConfig { version: 1, repos: vec![] };
        let app_state = AppState::new(repos);
        assert_eq!(app_state.repos.lock().unwrap().version, 1);
        assert_eq!(app_state.repos.lock().unwrap().repos.len(), 0);
        assert_eq!(app_state.watchers.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_write_app_state() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let state = AppStateFile {
            version: 1,
            window: WindowState { width: 1300, height: 900, x: 150, y: 250 },
            nav: NavState {
                last_view: "test".to_string(),
                last_repo_id: Some("test-id".to_string()),
                last_section: "sent".to_string(),
                expanded_repos: vec!["repo1".to_string()],
            },
            wizard_completed: false,
            timezone: String::new(),
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
        };

        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);
        write_app_state(&state).expect("Failed to write app_state");
        assert!(path.exists());

        let content = fs::read_to_string(&path).expect("Failed to read");
        let loaded: AppStateFile = serde_json::from_str(&content).expect("Failed to parse");
        assert_eq!(loaded.window.width, 1300);
        assert_eq!(loaded.nav.last_view, "test");
        assert_eq!(loaded.nav.expanded_repos.len(), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_malformed_json_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        fs::write(&path, "{ invalid json }").expect("Failed to write malformed JSON");
        let state = read_app_state();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_version_mismatch_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        let wrong = AppStateFile { version: 999, ..AppStateFile::default() };
        let json = serde_json::to_string_pretty(&wrong).expect("Failed to serialize");
        fs::write(&path, json).expect("Failed to write");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default with correct version");
        assert_eq!(loaded.window.width, 1100, "Should have default window width");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_io_error() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);
        fs::create_dir_all(&path).expect("Failed to create dir");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default on IO error");
        assert_eq!(loaded.window.width, 1100, "Should have default values");
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_read_app_state_valid_file() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);

        let state = AppStateFile {
            version: 1,
            window: WindowState { width: 1400, height: 1000, x: 200, y: 150 },
            nav: NavState {
                last_view: "all_repos".to_string(),
                last_repo_id: None,
                last_section: "sent".to_string(),
                expanded_repos: vec![],
            },
            wizard_completed: false,
            timezone: String::new(),
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
        };

        let json = serde_json::to_string_pretty(&state).expect("Failed to serialize");
        fs::write(&path, json).expect("Failed to write");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.window.width, 1400);
        assert_eq!(loaded.window.height, 1000);
        assert_eq!(loaded.nav.last_section, "sent");
        let _ = fs::remove_file(&path);
    }
}
