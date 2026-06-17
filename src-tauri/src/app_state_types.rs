// SPDX-License-Identifier: BUSL-1.1

//! Plain data types and serde schemas for the persisted app state file.
//!
//! Extracted from `app_state` to keep each file under the 400-line limit.

use serde::{Deserialize, Serialize};

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
    /// Set to true after the v1 credential migration (bare → project-scoped keys) completes.
    /// Uses serde default so pre-v1.3 app_state.json files deserialise as false (not yet migrated).
    #[serde(default)]
    pub credential_migration_v1: bool,
    /// Set to true after `repos.json` is successfully rewritten from v1 to v2 schema.
    /// Prevents the migration from running on every launch once complete.
    #[serde(default)]
    pub repos_schema_v2: bool,
    /// Set to true when the user clicks "Not now" on the workspace migration banner (22.5.4).
    /// Banner is permanently hidden; Settings re-entry button remains visible.
    #[serde(default)]
    pub workspace_migration_dismissed: bool,
    /// Set to true at the start of Step 1; cleared when Step 5 succeeds (22.7.7a).
    #[serde(default)]
    pub deletion_incomplete: bool,
    /// Set to true when the user dismisses the voice guide hint (22.3.22a).
    /// Persists across launches so the hint is shown once per app installation.
    #[serde(default)]
    pub voice_guide_hint_dismissed: bool,
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
            credential_migration_v1: false,
            repos_schema_v2: false,
            workspace_migration_dismissed: false,
            deletion_incomplete: false,
            voice_guide_hint_dismissed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppStateFile::default();
        assert_eq!(state.version, 1);
        assert_eq!(state.window.width, 1100);
        assert_eq!(state.nav.last_view, "all_repos");
        assert_eq!(state.nav.last_section, "drafts");
    }

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
    fn test_app_state_missing_default_post_time_field_deserialises_as_none() {
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":false,"timezone":"","telemetry_consent":false,"consent_asked":false}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(loaded.default_post_time.is_none());
    }

    #[test]
    fn test_default_post_time_missing_timezone_field_deserialises_as_empty() {
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

    #[test]
    fn test_app_state_version_mismatch_returns_default() {
        let state = AppStateFile { version: 999, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("Failed to serialize");
        let parsed: AppStateFile = serde_json::from_str(&json).expect("Parse should succeed");
        assert_eq!(parsed.version, 999);
    }

    #[test]
    fn test_post_wizard_completed_round_trips() {
        let state = AppStateFile { post_wizard_completed: true, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.post_wizard_completed, "post_wizard_completed must survive round-trip");
    }

    #[test]
    fn test_post_wizard_completed_absent_field_defaults_to_false() {
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":true,"timezone":"","telemetry_consent":false,"consent_asked":true}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(!loaded.post_wizard_completed, "missing post_wizard_completed must default to false");
    }

    #[test]
    fn test_org_upgrade_banner_dismissed_absent_field_defaults_to_false() {
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

    #[test]
    fn test_credential_migration_v1_absent_field_defaults_to_false() {
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":true,"timezone":"","telemetry_consent":false,"consent_asked":true}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(!loaded.credential_migration_v1, "missing field must default to false");
    }

    #[test]
    fn test_credential_migration_v1_round_trips() {
        let state = AppStateFile { credential_migration_v1: true, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.credential_migration_v1, "must survive round-trip");
    }

    #[test]
    fn test_voice_guide_hint_dismissed_defaults_to_false() {
        let json = r#"{"version":1,"window":{"width":1100,"height":700,"x":0,"y":0},"nav":{"last_view":"all_repos","last_repo_id":null,"last_section":"drafts","expanded_repos":[]},"wizard_completed":false,"timezone":"","telemetry_consent":false,"consent_asked":false}"#;
        let loaded: AppStateFile = serde_json::from_str(json).expect("should parse");
        assert!(!loaded.voice_guide_hint_dismissed, "missing field must default to false");
    }

    #[test]
    fn test_voice_guide_hint_dismissed_round_trips() {
        let state = AppStateFile { voice_guide_hint_dismissed: true, ..AppStateFile::default() };
        let json = serde_json::to_string(&state).expect("serialize");
        let loaded: AppStateFile = serde_json::from_str(&json).expect("deserialize");
        assert!(loaded.voice_guide_hint_dismissed, "voice_guide_hint_dismissed must survive round-trip");
    }
}
