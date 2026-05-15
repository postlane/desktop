// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct WizardState {
    pub step: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_name: Option<String>,
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

fn write_to_path(path: &Path, step: u32, workspace_id: Option<String>, workspace_name: Option<String>) -> Result<(), String> {
    let state = WizardState { step, workspace_id, workspace_name };
    let json = serde_json::to_string(&state).map_err(|e| format!("Serialise error: {}", e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("Failed to write wizard state: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Failed to commit wizard state: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn read_wizard_state() -> Result<Option<WizardState>, String> {
    let path = wizard_state_path()?;
    read_from_path(&path)
}

#[tauri::command]
pub fn write_wizard_state(step: u32, workspace_id: Option<String>, workspace_name: Option<String>) -> Result<(), String> {
    if step < 2 {
        return Ok(());
    }
    let path = wizard_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .postlane dir: {}", e))?;
    }
    write_to_path(&path, step, workspace_id, workspace_name)
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
    fn test_write_to_path_persists_workspace_fields() {
        let path = tmp_path("workspace_fields");
        write_to_path(&path, 4, Some("ws-1".to_string()), Some("My Org".to_string())).unwrap();
        let state = read_from_path(&path).unwrap().unwrap();
        assert_eq!(state.step, 4);
        assert_eq!(state.workspace_id.as_deref().unwrap(), "ws-1");
        assert_eq!(state.workspace_name.as_deref().unwrap(), "My Org");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_old_format_without_workspace_fields_returns_none_for_those_fields() {
        let path = tmp_path("old_format_no_workspace");
        std::fs::write(&path, r#"{"step":4}"#).unwrap();
        let state = read_from_path(&path).unwrap().unwrap();
        assert_eq!(state.step, 4);
        assert!(state.workspace_id.is_none());
        assert!(state.workspace_name.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_to_path_with_none_workspace_omits_fields_from_json() {
        let path = tmp_path("no_workspace");
        write_to_path(&path, 3, None, None).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("workspace_id"));
        assert!(!json.contains("workspace_name"));
        let _ = std::fs::remove_file(&path);
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
