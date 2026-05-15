// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct WizardState {
    pub step: u32,
}

fn wizard_state_path() -> Result<std::path::PathBuf, String> {
    Ok(postlane_dir()?.join("wizard_state.json"))
}

fn read_from_path(path: &Path) -> Result<Option<WizardState>, String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to read wizard state: {}", e)),
        Ok(content) => {
            let state: WizardState = serde_json::from_str(&content)
                .map_err(|e| format!("Corrupt wizard state: {}", e))?;
            if state.step < 2 {
                return Ok(None);
            }
            Ok(Some(state))
        }
    }
}

#[tauri::command]
pub fn read_wizard_state() -> Result<Option<WizardState>, String> {
    let path = wizard_state_path()?;
    read_from_path(&path)
}

#[tauri::command]
pub fn write_wizard_state(step: u32) -> Result<(), String> {
    if step < 2 {
        return Ok(());
    }
    let path = wizard_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .postlane dir: {}", e))?;
    }
    let state = WizardState { step };
    let json = serde_json::to_string(&state).map_err(|e| format!("Serialise error: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| format!("Failed to write wizard state: {}", e))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("Failed to commit wizard state: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn clear_wizard_state() -> Result<(), String> {
    let path = wizard_state_path()?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Failed to clear wizard state: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("postlane_wiz_{}", name))
    }

    #[test]
    fn test_read_returns_none_when_file_absent() {
        let path = tmp_path("absent");
        let _ = fs::remove_file(&path);
        assert!(read_from_path(&path).unwrap().is_none());
    }

    #[test]
    fn test_read_returns_none_when_step_is_1() {
        let path = tmp_path("step1");
        fs::write(&path, r#"{"step":1}"#).unwrap();
        assert!(read_from_path(&path).unwrap().is_none());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_returns_state_when_step_gte_2() {
        let path = tmp_path("step3");
        fs::write(&path, r#"{"step":3}"#).unwrap();
        let state = read_from_path(&path).unwrap().unwrap();
        assert_eq!(state.step, 3);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_read_returns_error_on_corrupt_json() {
        let path = tmp_path("corrupt");
        fs::write(&path, "not json").unwrap();
        assert!(read_from_path(&path).is_err());
        let _ = fs::remove_file(&path);
    }
}
