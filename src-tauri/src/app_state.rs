// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use crate::project_registry::ProjectSummary;
use crate::storage::ReposConfig;
use crate::telemetry::client::TelemetryClient;
use notify::RecommendedWatcher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::RwLock;

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
    /// Project list cache shared with the HTTP server's /github-project-config endpoint.
    /// Populated after sign-in and updated after project creation.
    pub projects_cache: Arc<RwLock<Vec<ProjectSummary>>>,
}

impl AppState {
    /// Creates a new `AppState` with the given repos config.
    /// The repos path is resolved from `postlane_dir()` at construction time.
    ///
    /// # Panics (test builds only)
    /// Panics in `#[cfg(test)]` builds. Use `AppState::new_with_path()` in all
    /// tests to point at an isolated temp path and never touch `~/.postlane`.
    pub fn new(repos: ReposConfig) -> Self {
        #[cfg(test)]
        panic!("Use AppState::new_with_path() in tests — AppState::new() writes to ~/.postlane");
        let repos_path = postlane_dir()
            .map(|d| d.join("repos.json"))
            .unwrap_or_else(|_| PathBuf::from("/dev/null"));
        Self::new_with_path(repos, repos_path)
    }

    pub fn lock_repos(&self) -> Result<MutexGuard<'_, ReposConfig>, String> {
        self.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))
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
            projects_cache: Arc::new(RwLock::new(vec![])),
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
    use crate::test_fixtures::AppStateGuard;
    use std::fs;

    #[test]
    fn test_read_app_state_missing_file_returns_default() {
        let _guard = AppStateGuard::acquire();
        assert!(!_guard.path.exists(), "File should not exist");
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
            credential_migration_v1: false,
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
    fn test_app_state_new_panics_in_test_builds() {
        // AppState::new() must panic in test builds so accidental callers surface
        // immediately rather than silently writing fixture data to ~/.postlane.
        // Use new_with_path() in all tests instead.
        let result = std::panic::catch_unwind(|| {
            AppState::new(crate::storage::ReposConfig { version: 1, repos: vec![] })
        });
        assert!(result.is_err(), "AppState::new() must panic in #[cfg(test)] builds");
    }

    #[test]
    fn test_app_state_new_with_path_initialises_correctly() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let repos = crate::storage::ReposConfig { version: 1, repos: vec![] };
        let app_state = AppState::new_with_path(repos, tmp.path().join("repos.json"));
        assert_eq!(app_state.repos.lock().unwrap().version, 1);
        assert_eq!(app_state.repos.lock().unwrap().repos.len(), 0);
        assert_eq!(app_state.watchers.lock().unwrap().len(), 0);
    }

    #[test]
    fn test_lock_repos_returns_guard_with_correct_data() {
        let repos = crate::storage::ReposConfig { version: 1, repos: vec![] };
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(repos, tmp.path().join("repos.json"));
        let guard = state.lock_repos().expect("must acquire repos lock");
        assert_eq!(guard.version, 1);
        assert!(guard.repos.is_empty());
    }

    #[test]
    fn test_lock_repos_error_message_mentions_repos() {
        // Poison the mutex from another thread, then verify the error message.
        use std::sync::Arc;
        let repos = crate::storage::ReposConfig { version: 1, repos: vec![] };
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let state = Arc::new(AppState::new_with_path(repos, tmp.path().join("repos.json")));
        let state2 = Arc::clone(&state);
        let _ = std::thread::spawn(move || {
            let _guard = state2.repos.lock().unwrap();
            panic!("poison");
        })
        .join();
        let err = state.lock_repos().unwrap_err();
        assert!(err.contains("Failed to lock repos"), "error was: {}", err);
    }

    #[test]
    fn test_write_app_state() {
        let _guard = AppStateGuard::acquire();

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
            credential_migration_v1: false,
        };

        write_app_state(&state).expect("Failed to write app_state");
        assert!(_guard.path.exists());

        let content = fs::read_to_string(&_guard.path).expect("Failed to read");
        let loaded: AppStateFile = serde_json::from_str(&content).expect("Failed to parse");
        assert_eq!(loaded.window.width, 1300);
        assert_eq!(loaded.nav.last_view, "test");
        assert_eq!(loaded.nav.expanded_repos.len(), 1);
    }

    #[test]
    fn test_read_app_state_malformed_json_returns_default() {
        let guard = AppStateGuard::acquire();
        fs::write(&guard.path, "{ invalid json }").expect("Failed to write malformed JSON");
        let state = read_app_state();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
    }

    #[test]
    fn test_read_app_state_version_mismatch_returns_default() {
        let guard = AppStateGuard::acquire();
        let wrong = AppStateFile { version: 999, ..AppStateFile::default() };
        let json = serde_json::to_string_pretty(&wrong).expect("Failed to serialize");
        fs::write(&guard.path, json).expect("Failed to write");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default with correct version");
        assert_eq!(loaded.window.width, 1100, "Should have default window width");
    }

    #[test]
    fn test_read_app_state_io_error() {
        let guard = AppStateGuard::acquire();
        fs::create_dir_all(&guard.path).expect("Failed to create dir");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default on IO error");
        assert_eq!(loaded.window.width, 1100, "Should have default values");
    }

    #[test]
    fn test_read_app_state_valid_file() {
        let guard = AppStateGuard::acquire();

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
            credential_migration_v1: false,
        };

        let json = serde_json::to_string_pretty(&state).expect("Failed to serialize");
        fs::write(&guard.path, json).expect("Failed to write");
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.window.width, 1400);
        assert_eq!(loaded.window.height, 1000);
        assert_eq!(loaded.nav.last_section, "sent");
    }
}
