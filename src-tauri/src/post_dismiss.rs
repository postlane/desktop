// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use std::fs;
use tauri::State;

/// Marks a post as `"dismissed"` by updating its `meta.json`.
/// Validates that the repo is registered before making any changes.
pub fn dismiss_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
    consent: bool,
) -> Result<(), String> {
    let canonical = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize repo path: {}", e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or("Repo path is not valid UTF-8")?
        .to_string();
    {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        if !repos.repos.iter().any(|r| r.path == canonical_str) {
            return Err(format!("Repo '{}' is not registered", canonical_str));
        }
    }
    let post_path = canonical.join(".postlane/posts").join(post_folder);
    let meta_path = post_path.join("meta.json");

    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let mut meta = crate::post_mutations::read_post_meta(&meta_path)?;
    meta.status = "dismissed".to_string();
    crate::post_mutations::write_post_meta(&meta_path, &meta)?;
    state
        .telemetry
        .record(consent, "post_dismissed", serde_json::json!({ "platforms": meta.platforms }));
    Ok(())
}

/// Tauri command — marks a post as dismissed and records telemetry if consent is given.
#[tauri::command]
pub fn dismiss_post(
    repo_path: String,
    post_folder: String,
    state: tauri::State<AppState>,
) -> Result<(), String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    dismiss_post_impl(&repo_path, &post_folder, &state, consent)
}

/// Deletes a single platform's `.md` file from a post folder.
///
/// If this was the last .md file in the folder, `meta.json` remains on disk.
/// This is a known v1 limitation — ghost folders with only `meta.json` accumulate over time.
/// A cleanup pass is planned for v2.
pub fn delete_post_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
    state: &AppState,
) -> Result<(), String> {
    if std::path::Path::new(post_folder).components().count() != 1 {
        return Err(format!(
            "Invalid post folder '{}': must be a single path component.",
            post_folder
        ));
    }
    if std::path::Path::new(platform).components().count() != 1 {
        return Err(format!(
            "Invalid platform '{}': must be a single path component.",
            platform
        ));
    }
    let canonical = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize repo path: {}", e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or("Repo path is not valid UTF-8")?
        .to_string();
    {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        if !repos.repos.iter().any(|r| r.path == canonical_str) {
            return Err(format!("Repo '{}' is not registered", canonical_str));
        }
    }
    let md_path = canonical
        .join(".postlane/posts")
        .join(post_folder)
        .join(format!("{}.md", platform));
    if !md_path.exists() {
        return Err(format!("{}.md not found in {}", platform, post_folder));
    }
    fs::remove_file(&md_path)
        .map_err(|e| format!("Failed to delete {}.md: {}", platform, e))
}

/// Tauri command — deletes a single platform `.md` file from a post folder.
#[tauri::command]
pub fn delete_post(
    repo_path: String,
    post_folder: String,
    platform: String,
    state: State<AppState>,
) -> Result<(), String> {
    delete_post_impl(&repo_path, &post_folder, &platform, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_repos_canonical(dirs: &[&std::path::Path]) -> ReposConfig {
        ReposConfig {
            version: 1,
            repos: dirs
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

    fn make_dismiss_dir(_suffix: &str) -> (tempfile::TempDir, String) {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/post-d");
        std::fs::create_dir_all(&post_dir).expect("create dir");
        let meta = serde_json::json!({"status": "ready", "platforms": ["x"]});
        std::fs::write(
            post_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).expect("serialize"),
        )
        .expect("write meta");
        let path = dir.path().to_str().unwrap().to_string();
        (dir, path)
    }

    fn make_dismiss_state(dir: &std::path::Path) -> AppState {
        let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: canonical.to_str().unwrap_or("").to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    fn make_delete_state(canonical_str: &str) -> AppState {
        AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: canonical_str.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        })
    }

    #[test]
    fn test_dismiss_post_rejects_unregistered_path() {
        let registered = tempfile::TempDir::new().expect("create temp dir");
        let unregistered = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = unregistered.path().join(".postlane/posts/post-d");
        fs::create_dir_all(&post_dir).expect("create post dir");
        let meta = serde_json::json!({"status": "ready", "platforms": ["x"]});
        fs::write(
            post_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).expect("serialize"),
        )
        .expect("write meta");
        let repos = make_repos_canonical(&[registered.path()]);
        let state = AppState::new(repos);
        let result = dismiss_post_impl(unregistered.path().to_str().unwrap(), "post-d", &state, false);
        assert!(result.is_err(), "dismiss_post_impl must reject unregistered path");
    }

    #[test]
    fn test_dismiss_post_canonicalizes_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/post-c");
        fs::create_dir_all(&post_dir).expect("create dir");
        let meta = serde_json::json!({"status": "ready", "platforms": ["x"]});
        fs::write(
            post_dir.join("meta.json"),
            serde_json::to_string_pretty(&meta).expect("serialize"),
        )
        .expect("write meta");
        let repos = make_repos_canonical(&[dir.path()]);
        let state = AppState::new(repos);
        let canonical_path = fs::canonicalize(dir.path()).expect("canonicalize");
        let result = dismiss_post_impl(canonical_path.to_str().unwrap(), "post-c", &state, false);
        assert!(
            result.is_ok(),
            "dismiss_post_impl must accept registered canonical path: {:?}",
            result
        );
    }

    #[test]
    fn test_dismiss_records_telemetry_when_consent_given() {
        let (dir, path) = make_dismiss_dir("yes");
        let state = make_dismiss_state(dir.path());
        let result = dismiss_post_impl(&path, "post-d", &state, true);
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    }

    #[test]
    fn test_dismiss_telemetry_includes_platforms() {
        let (dir, path) = make_dismiss_dir("platforms-check");
        let state = make_dismiss_state(dir.path());
        dismiss_post_impl(&path, "post-d", &state, true).expect("dismiss must succeed");
        let events = state.telemetry.peek_queue();
        let props = &events[0].properties;
        assert!(props.get("platforms").is_some(), "telemetry must include platforms field");
    }

    #[test]
    fn test_dismiss_no_telemetry_when_consent_not_given() {
        let (dir, path) = make_dismiss_dir("no");
        let state = make_dismiss_state(dir.path());
        let result = dismiss_post_impl(&path, "post-d", &state, false);
        assert!(result.is_ok(), "{:?}", result);
        assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
    }

    #[test]
    fn test_delete_post_removes_only_the_specified_platform_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_path = dir.path().join(".postlane/posts/post-del");
        fs::create_dir_all(&post_path).expect("create post dir");
        fs::write(post_path.join("x.md"), "x content").expect("write x.md");
        fs::write(post_path.join("linkedin.md"), "linkedin content").expect("write linkedin.md");
        fs::write(post_path.join("meta.json"), "{}").expect("write meta.json");
        let canonical = fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_delete_state(&canonical_str);
        let result = delete_post_impl(&canonical_str, "post-del", "x", &state);
        assert!(result.is_ok(), "delete x.md should succeed: {:?}", result);
        assert!(!post_path.join("x.md").exists(), "x.md must be deleted");
        assert!(post_path.join("linkedin.md").exists(), "linkedin.md must NOT be deleted");
        assert!(post_path.join("meta.json").exists(), "meta.json must NOT be deleted");
    }

    #[test]
    fn test_delete_post_rejects_multi_segment_post_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_delete_state(&canonical_str);
        let result = delete_post_impl(&canonical_str, "a/b", "x", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    #[test]
    fn test_delete_post_rejects_path_traversal() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_delete_state(&canonical_str);
        let result = delete_post_impl(&canonical_str, "../etc", "x", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid post folder"));
    }

    #[test]
    fn test_delete_post_rejects_path_traversal_in_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let canonical = fs::canonicalize(dir.path()).expect("canonicalize");
        let canonical_str = canonical.to_str().unwrap().to_string();
        let state = make_delete_state(&canonical_str);
        let result = delete_post_impl(&canonical_str, "post-a", "../secrets", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_lowercase().contains("invalid platform"));
    }

    #[test]
    fn test_delete_post_rejects_path_not_in_repos() {
        let state = make_delete_state("/some/other/repo");
        let result = delete_post_impl("/tmp", "post-a", "x", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }
}
