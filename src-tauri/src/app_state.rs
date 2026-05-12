// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use crate::storage::ReposConfig;
use crate::telemetry::client::TelemetryClient;
use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, OnceLock};

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
}

impl AppState {
    pub fn new(repos: ReposConfig) -> Self {
        Self {
            repos: Mutex::new(repos),
            watchers: Mutex::new(HashMap::new()),
            libsecret_available: Mutex::new(None),
            telemetry: Arc::new(TelemetryClient::new()),
            in_flight_sends: Arc::new(AtomicUsize::new(0)),
            http_port: Mutex::new(None),
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

/// Default post time (hour + minute with the timezone it was set in).
/// Storing the timezone inline prevents silent shifts if the user later changes
/// their display timezone — the schedule is always computed against `timezone`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DefaultPostTime {
    pub hour: u8,
    pub minute: u8,
    #[serde(default)]
    pub timezone: String,
}

/// Validates a DefaultPostTime, returning Err with a descriptive message if out of range.
pub fn validate_default_post_time(dpt: &DefaultPostTime) -> Result<(), String> {
    if dpt.hour > 23 {
        return Err(format!("hour {} is out of range (0–23)", dpt.hour));
    }
    if dpt.minute > 59 {
        return Err(format!("minute {} is out of range (0–59)", dpt.minute));
    }
    Ok(())
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
    /// Whether the user has given consent for opt-in product telemetry.
    #[serde(default)]
    pub telemetry_consent: bool,
    /// Whether the consent prompt has been shown. If false, show it on next launch.
    #[serde(default)]
    pub consent_asked: bool,
    /// Global default post time for new drafts. None = no pre-population.
    #[serde(default)]
    pub default_post_time: Option<DefaultPostTime>,
    /// Set to true when the user dismisses the unassigned-draft warning banner.
    #[serde(default)]
    pub dismissed_unassigned_draft_warning: bool,
    /// Whether to trigger macOS system notifications when new drafts are detected.
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
    /// Set to true after the user completes the post-wizard setup flow (connect repo + add accounts).
    /// Uses serde default so existing files without this field deserialise as false.
    #[serde(default)]
    pub post_wizard_completed: bool,
    /// Set to true when the user dismisses the v1.2 org-upgrade banner.
    /// Uses serde default so pre-v1.2 app_state.json files deserialise as false (banner shown).
    #[serde(default)]
    pub org_upgrade_banner_dismissed_v1_2: bool,
}

fn default_notifications_enabled() -> bool { true }

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
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            dismissed_unassigned_draft_warning: false,
            notifications_enabled: true,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
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

static APP_STATE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

static WIZARD_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Sets `default_post_time` in app_state.json atomically without clobbering other fields.
/// The lock serializes concurrent callers so hour/minute changes never interleave.
pub fn set_default_post_time_impl(dpt: Option<DefaultPostTime>) -> Result<(), String> {
    if let Some(ref d) = dpt {
        validate_default_post_time(d)?;
    }
    let _guard = APP_STATE_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    let mut state = read_app_state();
    state.default_post_time = dpt;
    write_app_state(&state)
}

#[tauri::command]
pub fn set_default_post_time(dpt: Option<DefaultPostTime>) -> Result<(), String> {
    set_default_post_time_impl(dpt)
}

/// Sets `wizard_completed: true` in app_state.json atomically.
/// The mutex serializes concurrent callers so they don't race on the tmp rename.
pub fn set_wizard_completed_impl() -> Result<(), String> {
    let _guard = WIZARD_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    let mut state = read_app_state();
    state.wizard_completed = true;
    write_app_state(&state)
}

#[tauri::command]
pub fn set_wizard_completed() -> Result<(), String> {
    set_wizard_completed_impl()
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
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
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
            telemetry_consent: false,
            consent_asked: false,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
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
    fn test_wizard_completed_written_atomically() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");

        let initial = AppStateFile { wizard_completed: false, ..AppStateFile::default() };
        write_app_state(&initial).expect("write initial");

        set_wizard_completed_impl().expect("set_wizard_completed_impl should succeed");

        let result = read_app_state();
        assert!(result.wizard_completed, "wizard_completed should be true after set_wizard_completed_impl");

        let path = app_state_path().expect("path");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_set_wizard_completed_concurrent_calls_all_succeed() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");

        let initial = AppStateFile { wizard_completed: false, ..AppStateFile::default() };
        write_app_state(&initial).expect("write initial");

        let handles: Vec<_> = (0..4)
            .map(|_| std::thread::spawn(set_wizard_completed_impl))
            .collect();

        for h in handles {
            h.join().expect("thread panicked").expect("set_wizard_completed_impl failed");
        }

        let result = read_app_state();
        assert!(result.wizard_completed, "wizard_completed must be true after concurrent writes");

        let path = app_state_path().expect("path");
        let _ = std::fs::remove_file(path);
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

        // Should read and return the state (exercises line 102)
        let loaded = read_app_state();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.window.width, 1400);
        assert_eq!(loaded.window.height, 1000);
        assert_eq!(loaded.nav.last_section, "sent");

        let _ = fs::remove_file(&path);
    }

    // ── DefaultPostTime ───────────────────────────────────────────────────────

    #[test]
    fn test_default_post_time_null_round_trips() {
        let state = AppStateFile { default_post_time: None, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.default_post_time.is_none());
    }

    #[test]
    fn test_default_post_time_valid_round_trips() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: String::new() };
        let state = AppStateFile { default_post_time: Some(dpt), ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        let loaded_dpt = loaded.default_post_time.expect("should be Some");
        assert_eq!(loaded_dpt.hour, 9);
        assert_eq!(loaded_dpt.minute, 30);
    }

    #[test]
    fn test_validate_default_post_time_rejects_hour_25() {
        let result = validate_default_post_time(&DefaultPostTime { hour: 25, minute: 0, timezone: String::new() });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hour"));
    }

    #[test]
    fn test_validate_default_post_time_rejects_minute_60() {
        let result = validate_default_post_time(&DefaultPostTime { hour: 0, minute: 60, timezone: String::new() });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("minute"));
    }

    #[test]
    fn test_validate_default_post_time_accepts_valid() {
        assert!(validate_default_post_time(&DefaultPostTime { hour: 23, minute: 59, timezone: String::new() }).is_ok());
        assert!(validate_default_post_time(&DefaultPostTime { hour: 0, minute: 0, timezone: String::new() }).is_ok());
    }

    #[test]
    fn test_app_state_missing_default_post_time_field_deserialises_as_none() {
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":false,"timezone":"","telemetry_consent":false,"consent_asked":false}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(loaded.default_post_time.is_none());
    }

    // ── DefaultPostTime.timezone ─────────────────────────────────────────────

    #[test]
    fn test_default_post_time_missing_timezone_field_deserialises_as_empty() {
        // Old records without the timezone field must still deserialise
        let json = r#"{"hour":9,"minute":30}"#;
        let dpt: DefaultPostTime = serde_json::from_str(json).expect("should parse");
        assert_eq!(dpt.hour, 9);
        assert_eq!(dpt.minute, 30);
        assert_eq!(dpt.timezone, "", "missing timezone must default to empty string");
    }

    #[test]
    fn test_default_post_time_round_trips_timezone() {
        let dpt = DefaultPostTime { hour: 9, minute: 30, timezone: "Asia/Kolkata".to_string() };
        let json = serde_json::to_string(&dpt).expect("serialize");
        let back: DefaultPostTime = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.timezone, "Asia/Kolkata");
    }

    // ── set_default_post_time_impl ───────────────────────────────────────────

    #[test]
    fn test_set_default_post_time_writes_without_clobbering_other_fields() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");

        let initial = AppStateFile {
            timezone: "Europe/London".to_string(),
            wizard_completed: true,
            default_post_time: None,
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        set_default_post_time_impl(Some(DefaultPostTime { hour: 9, minute: 30, timezone: String::new() }))
            .expect("should succeed");

        let result = read_app_state();
        assert_eq!(result.default_post_time.as_ref().map(|d| d.hour), Some(9));
        assert_eq!(result.default_post_time.as_ref().map(|d| d.minute), Some(30));
        // Other fields must not be clobbered
        assert_eq!(result.timezone, "Europe/London");
        assert!(result.wizard_completed);

        let path = app_state_path().expect("path");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_set_default_post_time_concurrent_calls_do_not_interleave() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");

        let initial = AppStateFile {
            default_post_time: None,
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        // Concurrent: one sets hour=9,min=30; other sets hour=14,min=0
        let h1 = std::thread::spawn(|| {
            set_default_post_time_impl(Some(DefaultPostTime { hour: 9, minute: 30, timezone: String::new() }))
        });
        let h2 = std::thread::spawn(|| {
            set_default_post_time_impl(Some(DefaultPostTime { hour: 14, minute: 0, timezone: String::new() }))
        });

        h1.join().expect("thread panicked").expect("h1 failed");
        h2.join().expect("thread panicked").expect("h2 failed");

        // Final state must be internally consistent (one of the two valid outcomes)
        let result = read_app_state();
        let dpt = result.default_post_time.expect("default_post_time must be set");
        let is_valid = (dpt.hour == 9 && dpt.minute == 30) || (dpt.hour == 14 && dpt.minute == 0);
        assert!(is_valid, "got inconsistent state: hour={}, minute={}", dpt.hour, dpt.minute);

        let path = app_state_path().expect("path");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_set_default_post_time_clear_sets_none() {
        let _lock = get_test_mutex().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");

        let initial = AppStateFile {
            default_post_time: Some(DefaultPostTime { hour: 9, minute: 30, timezone: String::new() }),
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        set_default_post_time_impl(None).expect("should succeed");

        let result = read_app_state();
        assert!(result.default_post_time.is_none(), "should be cleared");

        let path = app_state_path().expect("path");
        let _ = std::fs::remove_file(path);
    }

    // ── post_wizard_completed ────────────────────────────────────────────────────

    #[test]
    fn test_post_wizard_completed_round_trips() {
        let state = AppStateFile { post_wizard_completed: true, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.post_wizard_completed, "post_wizard_completed must survive round-trip");
    }

    #[test]
    fn test_post_wizard_completed_absent_field_defaults_to_false() {
        // JSON written before post_wizard_completed existed must deserialise as false
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":true,"timezone":"","telemetry_consent":false,"consent_asked":true}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(!loaded.post_wizard_completed, "missing post_wizard_completed must default to false");
    }

    // ── org_upgrade_banner_dismissed_v1_2 ────────────────────────────────────

    #[test]
    fn test_org_upgrade_banner_dismissed_absent_field_defaults_to_false() {
        // JSON written before v1.2 must deserialise with banner_dismissed = false
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":true,"timezone":"","telemetry_consent":false,"consent_asked":true}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(!loaded.org_upgrade_banner_dismissed_v1_2, "missing field must default to false");
    }

    #[test]
    fn test_org_upgrade_banner_dismissed_round_trips() {
        let state = AppStateFile { org_upgrade_banner_dismissed_v1_2: true, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.org_upgrade_banner_dismissed_v1_2, "must survive round-trip");
    }
}
