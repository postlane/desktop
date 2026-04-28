// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{read_app_state, write_app_state, AppState};
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
    s.telemetry_consent = consent;
    s.consent_asked = true;
    write_app_state(&s)
}
