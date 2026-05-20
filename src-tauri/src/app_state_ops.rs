// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands and impl functions for mutating `AppStateFile`.
//!
//! These are split from `app_state` to keep each file under the 400-line limit.

use crate::app_state::{read_app_state, write_app_state, DefaultPostTime};
use std::sync::{Mutex, OnceLock};

static APP_STATE_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

static WIZARD_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Validates a `DefaultPostTime`, returning `Err` with a descriptive message if out of range.
pub fn validate_default_post_time(dpt: &DefaultPostTime) -> Result<(), String> {
    if dpt.hour > 23 {
        return Err(format!("hour {} is out of range (0–23)", dpt.hour));
    }
    if dpt.minute > 59 {
        return Err(format!("minute {} is out of range (0–59)", dpt.minute));
    }
    Ok(())
}

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

/// Tauri command: sets the global default post time for new drafts.
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

/// Tauri command: marks the onboarding wizard as completed.
#[tauri::command]
pub fn set_wizard_completed() -> Result<(), String> {
    set_wizard_completed_impl()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{AppStateFile, DefaultPostTime};
    use crate::test_fixtures::AppStateGuard;

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
    fn test_wizard_completed_written_atomically() {
        let _guard = AppStateGuard::acquire();

        let initial = AppStateFile { wizard_completed: false, ..AppStateFile::default() };
        write_app_state(&initial).expect("write initial");

        set_wizard_completed_impl().expect("set_wizard_completed_impl should succeed");

        let result = read_app_state();
        assert!(result.wizard_completed, "wizard_completed should be true after set_wizard_completed_impl");
    }

    #[test]
    fn test_set_wizard_completed_concurrent_calls_all_succeed() {
        let _guard = AppStateGuard::acquire();

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
    }

    #[test]
    fn test_set_default_post_time_writes_without_clobbering_other_fields() {
        let _guard = AppStateGuard::acquire();

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
        assert_eq!(result.timezone, "Europe/London");
        assert!(result.wizard_completed);
    }

    #[test]
    fn test_set_default_post_time_concurrent_calls_do_not_interleave() {
        let _guard = AppStateGuard::acquire();

        let initial = AppStateFile {
            default_post_time: None,
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        let h1 = std::thread::spawn(|| {
            set_default_post_time_impl(Some(DefaultPostTime { hour: 9, minute: 30, timezone: String::new() }))
        });
        let h2 = std::thread::spawn(|| {
            set_default_post_time_impl(Some(DefaultPostTime { hour: 14, minute: 0, timezone: String::new() }))
        });

        h1.join().expect("thread panicked").expect("h1 failed");
        h2.join().expect("thread panicked").expect("h2 failed");

        let result = read_app_state();
        let dpt = result.default_post_time.expect("default_post_time must be set");
        let is_valid = (dpt.hour == 9 && dpt.minute == 30) || (dpt.hour == 14 && dpt.minute == 0);
        assert!(is_valid, "got inconsistent state: hour={}, minute={}", dpt.hour, dpt.minute);
    }

    #[test]
    fn test_set_default_post_time_command_delegates_to_impl() {
        let _guard = AppStateGuard::acquire();

        let initial = AppStateFile {
            default_post_time: None,
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        set_default_post_time(Some(DefaultPostTime { hour: 10, minute: 15, timezone: String::new() }))
            .expect("set_default_post_time command should succeed");

        let result = read_app_state();
        let dpt = result.default_post_time.expect("default_post_time must be set");
        assert_eq!(dpt.hour, 10);
        assert_eq!(dpt.minute, 15);
    }

    #[test]
    fn test_set_wizard_completed_command_delegates_to_impl() {
        let _guard = AppStateGuard::acquire();

        let initial = AppStateFile { wizard_completed: false, ..AppStateFile::default() };
        write_app_state(&initial).expect("write initial");

        set_wizard_completed().expect("set_wizard_completed command should succeed");

        let result = read_app_state();
        assert!(result.wizard_completed, "wizard_completed must be true after command call");
    }

    #[test]
    fn test_set_default_post_time_clear_sets_none() {
        let _guard = AppStateGuard::acquire();

        let initial = AppStateFile {
            default_post_time: Some(DefaultPostTime { hour: 9, minute: 30, timezone: String::new() }),
            ..AppStateFile::default()
        };
        write_app_state(&initial).expect("write initial");

        set_default_post_time_impl(None).expect("should succeed");

        let result = read_app_state();
        assert!(result.default_post_time.is_none(), "should be cleared");
    }
}
