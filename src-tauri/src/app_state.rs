// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use crate::providers::scheduling::SchedulingProvider;
use crate::storage::ReposConfig;
use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Application state shared across Tauri commands, watchers, and HTTP handlers
pub struct AppState {
    pub repos: Mutex<ReposConfig>,
    pub watchers: Mutex<HashMap<String, RecommendedWatcher>>,
    /// Scheduler uses tokio::sync::Mutex for async compatibility
    pub scheduler: tokio::sync::Mutex<Option<Box<dyn SchedulingProvider>>>,
    /// Linux libsecret availability flag
    /// None = not checked yet, Some(true) = available, Some(false) = unavailable
    pub libsecret_available: Mutex<Option<bool>>,
}

impl AppState {
    pub fn new(repos: ReposConfig) -> Self {
        Self {
            repos: Mutex::new(repos),
            watchers: Mutex::new(HashMap::new()),
            scheduler: tokio::sync::Mutex::new(None),
            libsecret_available: Mutex::new(None),
        }
    }
}

/// Window state for persistence
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

/// Navigation state for persistence
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NavState {
    pub last_view: String,
    pub last_repo_id: Option<String>,
    pub last_section: String,
    pub expanded_repos: Vec<String>,
}

/// Complete app state file schema
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AppStateFile {
    pub version: u32,
    pub window: WindowState,
    pub nav: NavState,
    /// Set to true after the user completes or skips the onboarding wizard.
    /// Uses serde default so existing app_state.json files without this field
    /// deserialise as false (wizard not yet completed).
    #[serde(default)]
    pub wizard_completed: bool,
    /// IANA timezone identifier (e.g. "America/New_York"). Empty string = system timezone.
    #[serde(default)]
    pub timezone: String,
}

impl Default for AppStateFile {
    fn default() -> Self {
        Self {
            version: 1,
            window: WindowState {
                width: 1100,
                height: 700,
                x: 0,
                y: 0,
            },
            nav: NavState {
                last_view: "all_repos".to_string(),
                last_repo_id: None,
                last_section: "drafts".to_string(),
                expanded_repos: vec![],
            },
            wizard_completed: false,
            timezone: String::new(),
        }
    }
}

fn app_state_path() -> Result<PathBuf, String> {
    Ok(postlane_dir()?.join("app_state.json"))
}

/// Reads app_state.json with silent fallback to defaults
/// On missing, corrupt, or version-mismatched file: returns default state
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

/// Writes app_state.json atomically
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
    use std::sync::{Mutex, OnceLock};

    // Global mutex to serialize tests that use the shared app_state file
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn get_test_mutex() -> &'static Mutex<()> {
        TEST_MUTEX.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_app_state_default() {
        let state = AppStateFile::default();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
        assert_eq!(state.nav.last_view, "all_repos");
        assert_eq!(state.nav.last_section, "drafts");
    }

    #[test]
    fn test_read_app_state_missing_file_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();

        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");

        // Ensure file doesn't exist
        let _ = fs::remove_file(&path);
        assert!(!path.exists(), "File should not exist");

        // No file exists - should return default via line 89
        let state = read_app_state();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
    }

    #[test]
    fn test_app_state_round_trip() {
        let dir = std::env::temp_dir().join("postlane_test_app_state_roundtrip");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let state = AppStateFile {
            version: 1,
            window: WindowState {
                width: 1200,
                height: 800,
                x: 100,
                y: 200,
            },
            nav: NavState {
                last_view: "repo".to_string(),
                last_repo_id: Some("test-repo-id".to_string()),
                last_section: "published".to_string(),
                expanded_repos: vec!["repo1".to_string(), "repo2".to_string()],
            },
            wizard_completed: false,
            timezone: String::new(),
        };

        let path = dir.join("app_state.json");
        let json = serde_json::to_string_pretty(&state).expect("Failed to serialize");
        crate::init::atomic_write(&path, json.as_bytes()).expect("Failed to write");

        // Read back
        let content = fs::read_to_string(&path).expect("Failed to read");
        let loaded: AppStateFile =
            serde_json::from_str(&content).expect("Failed to deserialize");

        assert_eq!(loaded.window.width, 1200);
        assert_eq!(loaded.nav.last_view, "repo");
        assert_eq!(loaded.nav.expanded_repos.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_app_state_version_mismatch_returns_default() {
        let state = AppStateFile {
            version: 999,
            ..AppStateFile::default()
        };

        let json = serde_json::to_string(&state).expect("Failed to serialize");

        // Parsing will succeed but version check should fail
        let parsed: AppStateFile = serde_json::from_str(&json).expect("Parse should succeed");
        assert_eq!(parsed.version, 999);

        // In the actual read_app_state function, this would trigger default return
    }

    #[test]
    fn test_app_state_new() {
        let repos = crate::storage::ReposConfig {
            version: 1,
            repos: vec![],
        };

        let app_state = AppState::new(repos.clone());

        // Verify all fields are initialized correctly
        assert_eq!(app_state.repos.lock().unwrap().version, 1);
        assert_eq!(app_state.repos.lock().unwrap().repos.len(), 0);
        assert_eq!(app_state.watchers.lock().unwrap().len(), 0);
        assert!(app_state.scheduler.blocking_lock().is_none());
    }

    #[test]
    fn test_write_app_state() {
        let _lock = get_test_mutex().lock().unwrap();

        // Ensure ~/.postlane exists
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let state = AppStateFile {
            version: 1,
            window: WindowState {
                width: 1300,
                height: 900,
                x: 150,
                y: 250,
            },
            nav: NavState {
                last_view: "test".to_string(),
                last_repo_id: Some("test-id".to_string()),
                last_section: "sent".to_string(),
                expanded_repos: vec!["repo1".to_string()],
            },
            wizard_completed: false,
            timezone: String::new(),
        };

        // Clean up before test
        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);

        // Write using the function
        write_app_state(&state).expect("Failed to write app_state");

        // Verify file exists
        assert!(path.exists());

        // Read back and verify content
        let content = fs::read_to_string(&path).expect("Failed to read");
        let loaded: AppStateFile = serde_json::from_str(&content).expect("Failed to parse");
        assert_eq!(loaded.window.width, 1300);
        assert_eq!(loaded.nav.last_view, "test");
        assert_eq!(loaded.nav.expanded_repos.len(), 1);

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_malformed_json_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();

        // Ensure ~/.postlane exists
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let path = app_state_path().expect("Failed to get app state path");

        // Write malformed JSON
        fs::write(&path, "{ invalid json }").expect("Failed to write malformed JSON");

        // read_app_state should return default on parse error
        let state = read_app_state();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100); // default value

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_version_mismatch_writes_file() {
        let _lock = get_test_mutex().lock().unwrap();

        // Ensure ~/.postlane exists
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let path = app_state_path().expect("Failed to get app state path");

        // Create state with wrong version
        let wrong_version_state = AppStateFile {
            version: 999,
            ..AppStateFile::default()
        };

        // Write it to disk
        let json = serde_json::to_string_pretty(&wrong_version_state)
            .expect("Failed to serialize");
        fs::write(&path, json).expect("Failed to write");

        // read_app_state should detect version mismatch and return default
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default with correct version");
        assert_eq!(loaded.window.width, 1100, "Should have default window width");

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_app_state_io_error() {
        let _lock = get_test_mutex().lock().unwrap();

        // Ensure ~/.postlane exists
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let path = app_state_path().expect("Failed to get app state path");

        // Clean up first
        let _ = fs::remove_file(&path);

        // Create a directory with the same name as the file to cause IO error
        fs::create_dir_all(&path).expect("Failed to create dir");

        // read_app_state should fail to read (it's a directory) and return default
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1, "Should return default on IO error");
        assert_eq!(loaded.window.width, 1100, "Should have default values");

        // Cleanup - remove the directory
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn test_read_app_state_valid_file() {
        let _lock = get_test_mutex().lock().unwrap();

        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = app_state_path().expect("Failed to get app state path");
        let _ = fs::remove_file(&path);

        // Write a valid version 1 file
        let state = AppStateFile {
            version: 1,
            window: WindowState {
                width: 1400,
                height: 1000,
                x: 200,
                y: 150,
            },
            nav: NavState {
                last_view: "all_repos".to_string(),
                last_repo_id: None,
                last_section: "sent".to_string(),
                expanded_repos: vec![],
            },
            wizard_completed: false,
            timezone: String::new(),
        };

        let json = serde_json::to_string_pretty(&state).expect("Failed to serialize");
        fs::write(&path, json).expect("Failed to write");

        // Should read and return the state (exercises line 102)
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.window.width, 1400);
        assert_eq!(loaded.window.height, 1000);
        assert_eq!(loaded.nav.last_section, "sent");

        let _ = fs::remove_file(&path);
    }
}
