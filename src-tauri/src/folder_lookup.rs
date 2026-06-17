// SPDX-License-Identifier: BUSL-1.1
//
// find_project_for_folder — given a folder path, returns the project_id it is
// registered under (from its .postlane/config.json), or None if the folder is
// not registered.
//
// Used by the wizard to detect when the selected folder already belongs to an
// existing workspace so the user can be redirected instead of creating a
// duplicate empty project (Bug 21.13.6a).

use crate::app_state::AppState;
use tauri::State;

/// Returns the `project_id` stored in `{folder}/.postlane/config.json` when
/// `folder` is already registered in `repos.json`, or `None` otherwise.
///
/// # Errors
/// Returns `Err` only for unrecoverable I/O or encoding issues (e.g. the path
/// is not valid UTF-8). A missing config.json or an absent `project_id` field
/// returns `Ok(None)`.
pub fn find_project_for_folder_impl(
    folder_path: &str,
    state: &AppState,
) -> Result<Option<String>, String> {
    let canonical = std::fs::canonicalize(folder_path)
        .map_err(|e| format!("Cannot read folder '{}': {}", folder_path, e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or_else(|| format!("Folder path '{}' contains non-UTF-8 bytes", folder_path))?;

    let repos = state.lock_repos()?;
    let is_registered = repos
        .repos
        .iter()
        .any(|r| r.path.to_lowercase() == canonical_str.to_lowercase());

    if !is_registered {
        return Ok(None);
    }
    drop(repos);

    let config_path = canonical.join(".postlane/config.json");
    if !config_path.exists() {
        return Ok(None);
    }

    let json: serde_json::Value = crate::init::read_json_file(&config_path)?;

    Ok(json
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string()))
}

#[tauri::command]
pub fn find_project_for_folder(
    folder_path: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    find_project_for_folder_impl(&folder_path, &state)
}

// ── tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{home_tmp, make_repo, make_state};
    use std::fs;

    fn write_config(dir: &std::path::Path, project_id: &str) {
        fs::create_dir_all(dir.join(".postlane")).expect("create .postlane");
        fs::write(
            dir.join(".postlane/config.json"),
            format!(r#"{{"version":1,"project_id":"{}"}}"#, project_id),
        )
        .expect("write config.json");
    }

    // --- §find_project_for_folder ---

    #[test]
    fn test_returns_none_when_folder_not_registered() {
        let dir = home_tmp("folder_lookup_not_reg");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        let state = make_state(vec![]);

        let result = find_project_for_folder_impl(dir.to_str().unwrap(), &state);
        assert_eq!(result.unwrap(), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_project_id_when_folder_registered_and_config_present() {
        let dir = home_tmp("folder_lookup_registered");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        write_config(&dir, "proj-abc-123");
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        let result = find_project_for_folder_impl(dir.to_str().unwrap(), &state).unwrap();
        assert_eq!(result, Some("proj-abc-123".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_none_when_folder_registered_but_config_missing() {
        let dir = home_tmp("folder_lookup_no_config");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        // No config.json written
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        let result = find_project_for_folder_impl(dir.to_str().unwrap(), &state).unwrap();
        assert_eq!(result, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_none_when_config_present_but_project_id_missing() {
        let dir = home_tmp("folder_lookup_no_proj_id");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        fs::create_dir_all(dir.join(".postlane")).expect("mkdir");
        fs::write(
            dir.join(".postlane/config.json"),
            r#"{"version":1,"scheduler":{}}"#,
        )
        .expect("write");
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let state = make_state(vec![make_repo("r1", canonical.to_str().unwrap())]);

        let result = find_project_for_folder_impl(dir.to_str().unwrap(), &state).unwrap();
        assert_eq!(result, None, "absent project_id field must yield None");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_returns_err_when_folder_does_not_exist() {
        let state = make_state(vec![]);
        let result = find_project_for_folder_impl("/nonexistent/path/xyz123", &state);
        assert!(result.is_err(), "nonexistent folder must return Err");
    }
}
