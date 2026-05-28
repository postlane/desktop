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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
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

fn write_to_path(path: &Path, step: u32, workspace_id: Option<String>, workspace_name: Option<String>, provider: Option<String>) -> Result<(), String> {
    let state = WizardState { step, workspace_id, workspace_name, provider };
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
pub fn write_wizard_state(step: u32, workspace_id: Option<String>, workspace_name: Option<String>, provider: Option<String>) -> Result<(), String> {
    if step < 2 {
        return Ok(());
    }
    let path = wizard_state_path()?;
    write_wizard_state_at(&path, step, workspace_id, workspace_name, provider)
}

pub(crate) fn write_wizard_state_at(path: &Path, step: u32, workspace_id: Option<String>, workspace_name: Option<String>, provider: Option<String>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .postlane dir: {}", e))?;
    }
    write_to_path(path, step, workspace_id, workspace_name, provider)
}

#[tauri::command]
pub fn clear_wizard_state() -> Result<(), String> {
    let path = wizard_state_path()?;
    clear_wizard_state_at(&path)
}

pub(crate) fn clear_wizard_state_at(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
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
    fn test_write_to_path_persists_provider() {
        let path = tmp_path("with_provider");
        write_to_path(&path, 3, None, None, Some("github".to_string())).unwrap();
        let state = read_from_path(&path).unwrap().unwrap();
        assert_eq!(state.provider.as_deref(), Some("github"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_to_path_with_none_provider_omits_field_from_json() {
        let path = tmp_path("no_provider");
        write_to_path(&path, 3, None, None, None).unwrap();
        let json = std::fs::read_to_string(&path).unwrap();
        assert!(!json.contains("provider"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_old_format_without_provider_returns_none_for_provider() {
        let path = tmp_path("old_format_no_provider");
        std::fs::write(&path, r#"{"step":4}"#).unwrap();
        let state = read_from_path(&path).unwrap().unwrap();
        assert!(state.provider.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_to_path_persists_workspace_fields() {
        let path = tmp_path("workspace_fields");
        write_to_path(&path, 4, Some("ws-1".to_string()), Some("My Org".to_string()), None).unwrap();
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
        write_to_path(&path, 3, None, None, None).unwrap();
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

    #[test]
    fn test_write_wizard_state_with_step_1_is_noop() {
        // step < 2 must return Ok without writing anything
        let result = write_wizard_state(1, None, None, None);
        assert!(result.is_ok(), "step=1 must return Ok");
    }

    #[test]
    fn test_clear_wizard_state_when_file_absent_returns_ok() {
        let path = tmp_path("clear_absent");
        // Ensure the file does not exist
        let _ = fs::remove_file(&path);
        let result = clear_wizard_state_at(&path);
        assert!(result.is_ok(), "clearing absent file must return Ok");
    }

    #[test]
    fn test_clear_wizard_state_when_file_exists_removes_it() {
        let path = tmp_path("clear_present");
        write_to_path(&path, 3, None, None, None).expect("write before clear");
        assert!(path.exists(), "file must exist before clear");
        let result = clear_wizard_state_at(&path);
        assert!(result.is_ok(), "clear must succeed");
        assert!(!path.exists(), "file must be gone after clear");
    }

    #[test]
    fn test_write_wizard_state_creates_dir_if_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Place the target path inside a non-existent subdirectory
        let path = dir.path().join("subdir_that_does_not_exist").join("wizard_state.json");
        let result = write_wizard_state_at(&path, 3, None, None, None);
        assert!(result.is_ok(), "must create missing parent dir");
        assert!(path.exists(), "file must be created");
    }

    // wizard_state line 25 — non-NotFound IO error from read_from_path
    #[test]
    fn test_read_from_path_returns_err_on_non_not_found_io_error() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // A directory path triggers "Is a directory" error (not NotFound)
        let result = read_from_path(dir.path());
        assert!(result.is_err(), "IO error (not NotFound) must return Err");
        assert!(
            result.unwrap_err().contains("Failed to read wizard state"),
            "error must mention 'Failed to read wizard state'"
        );
    }

    // wizard_state line 79 — remove_file on a directory returns a non-NotFound error
    #[test]
    fn test_clear_wizard_state_at_returns_error_when_path_is_directory() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Place a *directory* at the path where the file would be
        let path = dir.path().join("wizard_state.json");
        std::fs::create_dir(&path).expect("create directory at path");
        let result = clear_wizard_state_at(&path);
        assert!(result.is_err(), "remove_file on a directory must return Err");
        assert!(
            result.unwrap_err().contains("Failed to clear wizard state"),
            "error message must start with 'Failed to clear wizard state'"
        );
    }
}
