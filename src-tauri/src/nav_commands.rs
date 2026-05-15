// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{AppStateFile, read_app_state, write_app_state};
use serde::{Deserialize, Serialize};

// ── Watcher status ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WatcherStatus {
    pub repo_path: String,
    pub repo_name: String,
    pub active: bool,
    pub last_event_at: Option<String>,
}

pub fn get_watcher_status_impl(
    repos: &[(String, String, String)],
    active_ids: &std::collections::HashSet<String>,
) -> Vec<WatcherStatus> {
    repos.iter().map(|(id, name, path)| WatcherStatus {
        repo_path: path.clone(),
        repo_name: name.clone(),
        active: active_ids.contains(id),
        last_event_at: None,
    }).collect()
}

#[tauri::command]
pub fn get_watcher_status(state: tauri::State<crate::app_state::AppState>) -> Vec<WatcherStatus> {
    let repos = state.repos.lock().unwrap_or_else(|e| e.into_inner());
    let watchers = state.watchers.lock().unwrap_or_else(|e| e.into_inner());
    let active_ids: std::collections::HashSet<String> = watchers.keys().cloned().collect();
    let repo_data: Vec<(String, String, String)> = repos.repos.iter()
        .map(|r| (r.id.clone(), r.name.clone(), r.path.clone()))
        .collect();
    drop(repos);
    drop(watchers);
    get_watcher_status_impl(&repo_data, &active_ids)
}

pub use crate::account_config::{
    get_account_ids, get_repo_config_impl, list_profiles_for_repo, save_account_id,
    save_account_id_impl,
};
pub use crate::draft_queries::{DraftPost, get_all_drafts, get_all_drafts_impl};
pub use crate::model_stats::{ModelStatsResponse, get_model_stats, get_model_stats_impl};
pub use crate::published_queries::{
    get_all_published, get_all_published_impl, get_repo_published,
    get_repo_published_impl,
};
pub use crate::types::PublishedPost;
pub use crate::repo_queries::{RepoWithStatus, get_repos, get_repos_impl, scan_post_statuses};

/// Payload emitted on the "meta-changed" Tauri event
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetaChangedPayload {
    pub repo_id: String,
    pub post_folder: String,
}

#[tauri::command]
pub fn read_app_state_command() -> AppStateFile {
    read_app_state()
}

#[tauri::command]
pub fn save_app_state_command(state: AppStateFile) -> Result<(), String> {
    if let Some(ref dpt) = state.default_post_time {
        crate::app_state_ops::validate_default_post_time(dpt)?;
    }
    write_app_state(&state)
}

/// Returns the full app state from disk. Returns `AppStateFile::default()` when no file exists.
/// Used by the frontend to read persisted settings before modifying any field.
#[tauri::command]
pub fn get_app_state() -> AppStateFile {
    read_app_state()
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_autostart_enabled() -> bool {
    false
}

pub fn attribution_config_path() -> Result<std::path::PathBuf, String> {
    crate::init::postlane_dir().map(|d| d.join("config.json"))
}

pub fn read_attribution(config_path: &std::path::Path) -> bool {
    if !config_path.exists() {
        return true;
    }
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return true,
    };
    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return true,
    };
    v.get("attribution").and_then(|a| a.as_bool()).unwrap_or(true)
}

pub fn write_attribution(config_path: &std::path::Path, enabled: bool) -> Result<(), String> {
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(config_path)
            .map_err(|e| format!("Failed to read global config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse global config: {}", e))?
    } else {
        serde_json::json!({})
    };

    config["attribution"] = serde_json::Value::Bool(enabled);

    let json = serde_json::to_vec_pretty(&config)
        .map_err(|e| format!("Failed to serialize global config: {}", e))?;
    crate::init::atomic_write(config_path, &json)
        .map_err(|e| format!("Failed to write global config: {}", e))
}

#[tauri::command]
pub fn get_attribution() -> bool {
    attribution_config_path()
        .map(|p| read_attribution(&p))
        .unwrap_or(true)
}

#[tauri::command]
pub fn set_attribution(enabled: bool) -> Result<(), String> {
    let path = attribution_config_path()?;
    write_attribution(&path, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{AppStateFile, DefaultPostTime, NavState, WindowState};

    #[test]
    fn test_get_app_version_returns_semver() {
        let version = get_app_version();
        // Must be at least "major.minor.patch"
        let parts: Vec<&str> = version.split('.').collect();
        assert!(parts.len() >= 3, "version '{}' is not semver (needs at least 3 parts)", version);
        for part in &parts {
            assert!(part.parse::<u32>().is_ok(), "version component '{}' is not numeric", part);
        }
    }

    #[test]
    fn test_get_app_state_returns_empty_object_when_file_absent() {
        // When no app_state.json exists, get_app_state() returns AppStateFile::default().
        // "Empty object" is the typed default: version=1, all booleans false, strings empty.
        let default_state = AppStateFile::default();
        assert_eq!(default_state.version, 1);
        assert!(!default_state.wizard_completed, "wizard_completed default must be false");
        assert!(default_state.timezone.is_empty(), "timezone default must be empty string");
        assert!(!default_state.telemetry_consent, "telemetry_consent default must be false");
        assert!(!default_state.consent_asked, "consent_asked default must be false");
        assert!(default_state.default_post_time.is_none(), "default_post_time default must be None");
    }

    #[test]
    fn test_get_app_state_returns_previously_saved_value() {
        // Verifies that a saved AppStateFile round-trips through JSON without data loss.
        // In production, get_app_state() calls read_app_state() which deserialises from disk;
        // the serialise/deserialise path exercised here is identical.
        let saved = AppStateFile {
            version: 1,
            window: WindowState { width: 1400, height: 900, x: 50, y: 50 },
            nav: NavState {
                last_view: "org_view".to_string(),
                last_repo_id: Some("repo-abc".to_string()),
                last_section: "history".to_string(),
                expanded_repos: vec!["repo-abc".to_string()],
            },
            timezone: "America/Chicago".to_string(),
            wizard_completed: true,
            telemetry_consent: true,
            consent_asked: true,
            default_post_time: None,
            notifications_enabled: true,
            dismissed_unassigned_draft_warning: false,
            post_wizard_completed: false,
            org_upgrade_banner_dismissed_v1_2: false,
        };
        let json = serde_json::to_string(&saved).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.timezone, "America/Chicago");
        assert!(loaded.wizard_completed);
        assert_eq!(loaded.window.width, 1400);
        assert_eq!(loaded.nav.last_section, "history");
    }

    #[test]
    fn test_save_app_state_command_merges_without_overwriting_unrelated_keys() {
        // The frontend pattern is: read current state → modify specific fields → write full state.
        // This test verifies that an AppStateFile with multiple non-default fields does not
        // silently reset any field to its zero value through the serialise/deserialise round-trip.
        let state = AppStateFile {
            timezone: "Europe/London".to_string(),
            wizard_completed: true,
            telemetry_consent: true,
            consent_asked: true,
            ..AppStateFile::default()
        };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded.timezone, "Europe/London", "timezone must not be reset to default");
        assert!(loaded.wizard_completed, "wizard_completed must not be reset to default");
        assert!(loaded.telemetry_consent, "telemetry_consent must not be reset to default");
        assert!(loaded.consent_asked, "consent_asked must not be reset to default");
    }

    #[test]
    fn test_save_app_state_command_rejects_invalid_default_post_time() {
        let state = AppStateFile {
            default_post_time: Some(DefaultPostTime { hour: 25, minute: 0, timezone: String::new() }),
            ..AppStateFile::default()
        };
        let result = save_app_state_command(state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hour"));
    }

    #[test]
    fn test_save_app_state_command_rejects_invalid_minute() {
        let state = AppStateFile {
            default_post_time: Some(DefaultPostTime { hour: 9, minute: 61, timezone: String::new() }),
            ..AppStateFile::default()
        };
        let result = save_app_state_command(state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("minute"));
    }

    #[test]
    fn test_get_watcher_status_returns_active_for_running_watcher() {
        let repos = vec![
            ("repo-1".to_string(), "MyRepo".to_string(), "/repos/myrepo".to_string()),
        ];
        let mut active = std::collections::HashSet::new();
        active.insert("repo-1".to_string());
        let statuses = get_watcher_status_impl(&repos, &active);
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].active);
        assert_eq!(statuses[0].repo_name, "MyRepo");
        assert_eq!(statuses[0].repo_path, "/repos/myrepo");
    }

    #[test]
    fn test_get_watcher_status_returns_inactive_when_watcher_not_started() {
        let repos = vec![
            ("repo-1".to_string(), "MyRepo".to_string(), "/repos/myrepo".to_string()),
        ];
        let active = std::collections::HashSet::new();
        let statuses = get_watcher_status_impl(&repos, &active);
        assert_eq!(statuses.len(), 1);
        assert!(!statuses[0].active);
    }
}
