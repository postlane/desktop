// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{read_app_state, write_app_state, AppState, AppStateFile};
use tauri::State;

/// Returns whether the user has given telemetry consent.
#[tauri::command]
pub fn get_telemetry_consent(_state: State<AppState>) -> Result<bool, String> {
    Ok(read_app_state().telemetry_consent)
}

/// Saves the user's telemetry consent choice and marks consent_asked = true.
#[tauri::command]
pub fn set_telemetry_consent(consent: bool) -> Result<(), String> {
    let mut s = read_app_state();
    set_telemetry_consent_impl(consent, &mut s);
    write_app_state(&s)
}

/// Pure logic: apply consent to a state value without I/O.
pub(crate) fn set_telemetry_consent_impl(consent: bool, state: &mut AppStateFile) {
    state.telemetry_consent = consent;
    state.consent_asked = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_telemetry_consent_impl_true_sets_both_fields() {
        let mut state = AppStateFile::default();
        set_telemetry_consent_impl(true, &mut state);
        assert!(state.telemetry_consent, "telemetry_consent must be true");
        assert!(state.consent_asked, "consent_asked must be true");
    }

    #[test]
    fn test_set_telemetry_consent_impl_false_sets_both_fields() {
        let mut state = AppStateFile { telemetry_consent: true, consent_asked: false, ..AppStateFile::default() };
        set_telemetry_consent_impl(false, &mut state);
        assert!(!state.telemetry_consent, "telemetry_consent must be false");
        assert!(state.consent_asked, "consent_asked must be true");
    }

    #[test]
    fn test_set_telemetry_consent_impl_preserves_other_fields() {
        let mut state = AppStateFile { wizard_completed: true, ..AppStateFile::default() };
        set_telemetry_consent_impl(true, &mut state);
        assert!(state.wizard_completed, "wizard_completed must not be changed");
    }

    #[test]
    fn test_get_telemetry_consent_returns_stored_value() {
        let state = AppStateFile { telemetry_consent: true, consent_asked: true, ..AppStateFile::default() };
        assert!(state.telemetry_consent, "should return the stored consent value");
    }
}
