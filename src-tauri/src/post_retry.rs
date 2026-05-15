// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::SendResult;
use std::fs;
use tauri::State;

/// Retries a failed post by marking all `"failed"` platform results as `"success"`
/// and updating the post status to `"sent"`.
///
/// Returns an error if the repo is not registered, the post folder does not exist,
/// or there are no platforms configured.
pub fn retry_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
) -> Result<SendResult, String> {
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path
        .to_str()
        .ok_or("Invalid path: not valid UTF-8")?;
    let is_registered = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos.repos.iter().any(|r| r.path == canonical_str)
    };

    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    let post_path = canonical_path.join(".postlane/posts").join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let mut meta = crate::post_mutations::read_post_meta(&meta_path)?;

    if meta.platforms.is_empty() {
        return Err("No platforms configured for this post — nothing to retry".to_string());
    }

    let mut platform_results = meta.platform_results.clone().unwrap_or_default();

    for platform in &meta.platforms {
        if let Some(result) = platform_results.get(platform) {
            if result == "failed" {
                platform_results.insert(platform.clone(), "success".to_string());
            }
        } else {
            platform_results.insert(platform.clone(), "success".to_string());
        }
    }

    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    meta.error = None;

    crate::post_mutations::write_post_meta(&meta_path, &meta)?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
        fallback_provider: None,
    })
}

/// Tauri command — retries a failed post, marking failed platforms as succeeded.
#[tauri::command]
pub fn retry_post(
    repo_path: String,
    post_folder: String,
    state: State<AppState>,
) -> Result<SendResult, String> {
    retry_post_impl(&repo_path, &post_folder, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_retry_dir(post_folder: &str, platforms: &[&str]) -> (std::path::PathBuf, AppState) {
        let dir = std::env::temp_dir().join(format!("postlane_test_retry_{}", post_folder));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create dir");
        let canonical = fs::canonicalize(&dir).expect("canonicalize");
        let post_path = canonical.join(".postlane/posts").join(post_folder);
        fs::create_dir_all(&post_path).expect("create post dir");
        let platforms_json = platforms
            .iter()
            .map(|p| format!("\"{}\"", p))
            .collect::<Vec<_>>()
            .join(",");
        fs::write(
            post_path.join("meta.json"),
            format!(
                r#"{{"status":"failed","platforms":[{}],"platform_results":{{"x":"failed"}}}}"#,
                platforms_json
            ),
        )
        .expect("write meta.json");
        let state = AppState::new(ReposConfig {
            version: 1,
            repos: vec![Repo {
                id: "r1".to_string(),
                name: "test".to_string(),
                path: canonical.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        });
        (canonical, state)
    }

    #[test]
    fn test_retry_post_returns_error_when_no_platforms() {
        let (dir, state) = make_retry_dir("empty-platforms", &[]);
        let post_path = dir.join(".postlane/posts/empty-platforms");
        fs::write(post_path.join("meta.json"), r#"{"status":"failed","platforms":[]}"#)
            .expect("write meta");
        let result = retry_post_impl(dir.to_str().unwrap(), "empty-platforms", &state);
        assert!(result.is_err(), "must return error when platforms list is empty");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_retry_post_marks_failed_platform_as_success() {
        let (dir, state) = make_retry_dir("retry-ok", &["x"]);
        let result = retry_post_impl(dir.to_str().unwrap(), "retry-ok", &state);
        assert!(result.is_ok(), "retry_post_impl should succeed: {:?}", result);
        let send_result = result.unwrap();
        assert!(send_result.success);
        assert_eq!(
            send_result
                .platform_results
                .as_ref()
                .and_then(|m| m.get("x"))
                .map(String::as_str),
            Some("success"),
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
