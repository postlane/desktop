// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use std::fs;
use tauri::State;

/// Writes a `pending-redraft.json` file into the repo's `.postlane` directory.
///
/// The file is written atomically via a `.tmp` rename. Returns an error if a
/// pending redraft already exists — callers must cancel first.
pub fn queue_redraft_impl(
    repo_path: &str,
    post_folder: &str,
    instruction: &str,
    repos: &crate::storage::ReposConfig,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let canonical_str = canonical_path
        .to_str()
        .ok_or("Invalid path: not valid UTF-8")?;
    let is_registered = repos.repos.iter().any(|r| r.path == canonical_str);
    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err("Invalid post folder: must not contain path separators or '..'".to_string());
    }

    if instruction.len() > 10_000 {
        return Err(format!(
            "Instruction too long ({} chars). Maximum is 10,000 characters.",
            instruction.len()
        ));
    }

    let postlane_dir = canonical_path.join(".postlane");
    fs::create_dir_all(&postlane_dir).map_err(|e| {
        format!(
            "Failed to create .postlane directory in {}: {}",
            canonical_path.display(),
            e
        )
    })?;

    let pending_path = postlane_dir.join("pending-redraft.json");
    if pending_path.exists() {
        return Err("A redraft is already queued. Cancel the existing redraft first.".to_string());
    }
    let tmp_path = postlane_dir.join("pending-redraft.json.tmp");

    let sanitized_instruction: String = instruction
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();

    let queued_at = chrono::Utc::now().to_rfc3339();
    let payload = serde_json::json!({
        "post_folder": post_folder,
        "instruction": sanitized_instruction,
        "queued_at": queued_at,
    });

    let json_content = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize pending-redraft.json: {}", e))?;

    fs::write(&tmp_path, json_content)
        .map_err(|e| format!("Failed to write pending-redraft.json.tmp: {}", e))?;
    fs::rename(&tmp_path, &pending_path)
        .map_err(|e| format!("Failed to rename pending-redraft.json: {}", e))?;

    Ok(())
}

/// Removes the `pending-redraft.json` file from a repo's `.postlane` directory.
/// This operation is idempotent — no error is returned if the file does not exist.
pub fn cancel_redraft_impl(
    repo_path: &str,
    repos: &crate::storage::ReposConfig,
) -> Result<(), String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;
    let canonical_str = canonical_path
        .to_str()
        .ok_or("Invalid path: not valid UTF-8")?;
    let is_registered = repos.repos.iter().any(|r| r.path == canonical_str);
    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    let pending_path = canonical_path.join(".postlane/pending-redraft.json");
    if pending_path.exists() {
        fs::remove_file(&pending_path)
            .map_err(|e| format!("Failed to delete pending-redraft.json: {}", e))?;
    }
    // If file doesn't exist, that's fine — idempotent
    Ok(())
}

/// Tauri command — cancels any pending redraft for the given repo.
#[tauri::command]
pub fn cancel_redraft(repo_path: String, state: State<AppState>) -> Result<(), String> {
    let repos = state.lock_repos()?;
    cancel_redraft_impl(&repo_path, &repos)
}

/// Tauri command — queues a redraft instruction for the given post folder.
#[tauri::command]
pub fn queue_redraft(
    repo_path: String,
    post_folder: String,
    instruction: String,
    state: State<AppState>,
) -> Result<(), String> {
    let repos = state.lock_repos()?;
    queue_redraft_impl(&repo_path, &post_folder, &instruction, &repos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_repos_canonical(dirs: &[&std::path::Path]) -> ReposConfig {
        ReposConfig {
            version: 1, workspaces: vec![], repos: dirs
                .iter()
                .map(|d| {
                    let canonical = fs::canonicalize(d).unwrap_or_else(|_| d.to_path_buf());
                    Repo {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: "test".to_string(),
                        path: canonical.to_str().unwrap_or("").to_string(),
                        active: true,
                        added_at: "2026-01-01T00:00:00Z".to_string(),
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn test_queue_redraft_writes_correct_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        let result =
            queue_redraft_impl(dir.path().to_str().unwrap(), "20260101-v100-changelog", "make it shorter", &repos);
        assert!(result.is_ok(), "expected Ok but got: {:?}", result);

        let pending_path = dir.path().join(".postlane/pending-redraft.json");
        assert!(pending_path.exists(), "pending-redraft.json should exist");

        let content = std::fs::read_to_string(&pending_path).expect("read pending-redraft.json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");

        assert_eq!(parsed["post_folder"].as_str(), Some("20260101-v100-changelog"));
        assert_eq!(parsed["instruction"].as_str(), Some("make it shorter"));
        assert!(parsed["queued_at"].as_str().is_some(), "queued_at must be present");
    }

    #[test]
    fn test_queue_redraft_rejects_unregistered_repo() {
        let registered = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[registered.path()]);

        // Use a path that does not exist so canonicalize fails, triggering 403
        let result = queue_redraft_impl(
            "/nonexistent/postlane_test_queue_redraft_rejects",
            "20260101-v100-changelog",
            "make it shorter",
            &repos,
        );

        assert!(result.is_err(), "expected Err for unregistered repo");
    }

    #[test]
    fn test_queue_redraft_blocks_overwrite_of_existing_pending() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        let first =
            queue_redraft_impl(dir.path().to_str().unwrap(), "20260101-v100-changelog", "first instruction", &repos);
        assert!(first.is_ok(), "first queue_redraft should succeed");

        let second = queue_redraft_impl(
            dir.path().to_str().unwrap(),
            "20260201-v110-changelog",
            "second instruction",
            &repos,
        );
        assert!(second.is_err(), "second queue_redraft should return an error");
        let err = second.unwrap_err();
        assert!(
            err.contains("already queued"),
            "error must mention 'already queued', got: {}",
            err
        );

        let pending_path = dir.path().join(".postlane/pending-redraft.json");
        let content = std::fs::read_to_string(&pending_path).expect("read pending-redraft.json");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        assert_eq!(parsed["post_folder"].as_str(), Some("20260101-v100-changelog"));
    }

    #[test]
    fn test_queue_redraft_rejects_path_traversal_in_post_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        let result = queue_redraft_impl(
            dir.path().to_str().unwrap(),
            "../../../etc",
            "make it shorter",
            &repos,
        );

        assert!(result.is_err(), "expected Err for path traversal");
        assert!(
            result.unwrap_err().contains("Invalid post folder"),
            "error must mention 'Invalid post folder'"
        );
    }

    #[test]
    fn test_queue_redraft_rejects_instruction_too_long() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);
        let long_instruction = "x".repeat(10_001);

        let result =
            queue_redraft_impl(dir.path().to_str().unwrap(), "post-folder", &long_instruction, &repos);

        assert!(result.is_err(), "expected Err for too-long instruction");
        assert!(
            result.unwrap_err().contains("Instruction too long"),
            "error must mention 'Instruction too long'"
        );
    }

    #[test]
    fn test_cancel_redraft_deletes_pending_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        queue_redraft_impl(dir.path().to_str().unwrap(), "post-folder", "make it shorter", &repos)
            .expect("queue should succeed");

        let pending_path = dir.path().join(".postlane/pending-redraft.json");
        assert!(pending_path.exists(), "pending file should exist after queue");

        let result = cancel_redraft_impl(dir.path().to_str().unwrap(), &repos);
        assert!(result.is_ok(), "cancel should succeed: {:?}", result);
        assert!(!pending_path.exists(), "pending file should be gone after cancel");
    }

    #[test]
    fn test_cancel_redraft_is_idempotent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        let result = cancel_redraft_impl(dir.path().to_str().unwrap(), &repos);
        assert!(result.is_ok(), "cancel with no pending file should return Ok");
    }

    #[test]
    fn test_queue_redraft_sanitizes_control_characters() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[dir.path()]);

        let instruction_with_controls = "make it shorter\x00\x01\x08";
        let result = queue_redraft_impl(
            dir.path().to_str().unwrap(),
            "post-folder",
            instruction_with_controls,
            &repos,
        );
        assert!(result.is_ok(), "should succeed: {:?}", result);

        let pending_path = dir.path().join(".postlane/pending-redraft.json");
        let content = std::fs::read_to_string(&pending_path).expect("read file");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        let saved = parsed["instruction"].as_str().expect("instruction field");
        assert!(!saved.contains('\x00'), "null bytes must be stripped");
        assert!(!saved.contains('\x01'), "control chars must be stripped");
        assert!(saved.contains("make it shorter"), "printable text must be kept");
    }

    #[test]
    fn test_cancel_redraft_rejects_unregistered_repo() {
        let registered = tempfile::TempDir::new().expect("create temp dir");
        let repos = make_repos_canonical(&[registered.path()]);

        let result = cancel_redraft_impl("/nonexistent/postlane_test_cancel_redraft_unregistered", &repos);
        assert!(result.is_err(), "expected Err for unregistered repo");
    }

    /// Error messages for filesystem operations must include the repo path for debugging.
    #[test]
    fn test_queue_redraft_rejects_registered_but_wrong_repo() {
        // Path EXISTS and canonicalizes successfully, but it is not in the repos list.
        // This exercises the is_registered check at line 24 (after canonicalize succeeds).
        let registered = tempfile::TempDir::new().expect("registered dir");
        let unregistered = tempfile::TempDir::new().expect("unregistered dir");
        let repos = make_repos_canonical(&[registered.path()]);
        let result = queue_redraft_impl(
            unregistered.path().to_str().unwrap(),
            "post-folder",
            "make it shorter",
            &repos,
        );
        assert!(result.is_err(), "must Err for existing but unregistered path");
        let err = result.unwrap_err();
        assert!(err.contains("403"), "error must contain 403, got: {}", err);
    }

    #[test]
    fn test_cancel_redraft_rejects_existing_but_unregistered_path() {
        // Path EXISTS and canonicalizes successfully, but is not in repos —
        // exercises the is_registered check at line 89 inside cancel_redraft_impl.
        let registered = tempfile::TempDir::new().expect("registered dir");
        let unregistered = tempfile::TempDir::new().expect("unregistered dir");
        let repos = make_repos_canonical(&[registered.path()]);
        let result = cancel_redraft_impl(unregistered.path().to_str().unwrap(), &repos);
        assert!(result.is_err(), "must Err for existing but unregistered path");
        let err = result.unwrap_err();
        assert!(err.contains("403"), "error must contain 403, got: {}", err);
    }

    #[test]
    fn test_queue_redraft_create_dir_error_includes_path() {
        let parent = tempfile::TempDir::new().expect("create temp dir");
        let base = parent.path().join("postlane_test_path_in_error");
        std::fs::write(&base, b"not a dir").expect("write file");
        let canonical = fs::canonicalize(&base).expect("canonicalize");
        let repos = crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![crate::storage::Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        let result =
            queue_redraft_impl(canonical.to_str().unwrap(), "post-folder", "make it shorter", &repos);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains(canonical.to_str().unwrap()),
            "error must include repo path for debugging, got: {}",
            err
        );
    }
}
