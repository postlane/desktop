// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::post_mutations::{read_post_meta, write_post_meta};
use chrono::{DateTime, Utc};
use std::fs;
use tauri::State;

/// Update (or clear) the schedule field of a post's `meta.json`.
///
/// `schedule` must be an ISO 8601 UTC string that is in the future relative to `now`,
/// or `None` to clear. `repo_path` must be in the registered repos list.
pub fn update_post_schedule_impl(
    repo_path: &str,
    post_folder: &str,
    schedule: Option<&str>,
    state: &AppState,
    now: DateTime<Utc>,
    timezone: Option<&str>,
) -> Result<(), String> {
    let canonical = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to resolve path '{}': {}", repo_path, e))?;
    let canonical_str = canonical.to_str().ok_or("Path contains invalid UTF-8")?;

    let registered = {
        let repos = state.repos.lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;
        repos.repos.iter().any(|r| r.path == canonical_str)
    };
    if !registered {
        return Err(format!("Path '{}' is not in the registered repos list", repo_path));
    }

    if let Some(s) = schedule {
        let parsed: DateTime<Utc> = s.parse()
            .map_err(|_| format!("'{}' is not a valid ISO 8601 datetime", s))?;
        if parsed <= now {
            return Err(format!(
                "Schedule '{}' is in the past (current UTC time: {}) — choose a future time, and check your timezone setting",
                s, now.format("%Y-%m-%dT%H:%M:%SZ")
            ));
        }
    }

    let meta_path = canonical.join(".postlane/posts").join(post_folder).join("meta.json");
    let mut meta = read_post_meta(&meta_path)?;
    meta.schedule = schedule.map(str::to_string);
    meta.schedule_source = schedule.map(|_| "user".to_string());
    meta.schedule_timezone = if schedule.is_some() { timezone.map(str::to_string) } else { None };
    write_post_meta(&meta_path, &meta)
}

#[tauri::command]
pub fn update_post_schedule(
    repo_path: String,
    post_folder: String,
    schedule: Option<String>,
    timezone: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_post_schedule_impl(&repo_path, &post_folder, schedule.as_deref(), &state, Utc::now(), timezone.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Repo, ReposConfig};
    use std::path::PathBuf;

    fn make_state_with_dir(dir: &std::path::Path) -> (AppState, tempfile::TempDir) {
        let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let state = AppState::new_with_path(
            ReposConfig {
                version: 1,
                repos: vec![Repo {
                    id: "r1".to_string(),
                    name: "test".to_string(),
                    path: canonical.to_str().unwrap_or("").to_string(),
                    active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                }],
            },
            _tmp_repos.path().join("repos.json"),
        );
        (state, _tmp_repos)
    }

    fn write_meta_in(dir: &std::path::Path, post_folder: &str) -> PathBuf {
        let post_dir = dir.join(".postlane/posts").join(post_folder);
        fs::create_dir_all(&post_dir).expect("create post dir");
        let path = post_dir.join("meta.json");
        fs::write(&path, r#"{"status":"ready","platforms":["x"]}"#).expect("write meta.json");
        path
    }

    fn future() -> DateTime<Utc> {
        "2026-06-01T09:00:00Z".parse().unwrap()
    }

    fn schedule_tomorrow() -> &'static str { "2026-06-02T10:00:00Z" }

    #[test]
    fn test_sets_future_schedule() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result = update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some(schedule_tomorrow()), &state, future(), None,
        );
        assert!(result.is_ok(), "{:?}", result);
        let meta = crate::post_mutations::read_post_meta(
            &dir.path().join(".postlane/posts/post-001/meta.json")
        ).unwrap();
        assert_eq!(meta.schedule.as_deref(), Some(schedule_tomorrow()));
    }

    #[test]
    fn test_clears_schedule_with_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta_in(dir.path(), "post-001");
        // pre-set a schedule
        let mut meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        meta.schedule = Some(schedule_tomorrow().to_string());
        crate::post_mutations::write_post_meta(&meta_path, &meta).unwrap();
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result = update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", None, &state, future(), None,
        );
        assert!(result.is_ok(), "{:?}", result);
        let meta2 = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert!(meta2.schedule.is_none());
    }

    #[test]
    fn test_rejects_past_timestamp() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let now: DateTime<Utc> = "2026-06-03T12:00:00Z".parse().unwrap();
        let result = update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some("2026-06-03T11:00:00Z"), &state, now, None,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("past"), "error should mention 'past': {}", msg);
        assert!(msg.contains("current UTC time"), "error should include current UTC time: {}", msg);
        assert!(msg.contains("timezone"), "error should mention timezone setting: {}", msg);
    }

    #[test]
    fn test_rejects_malformed_iso() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        let result = update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some("not-a-date"), &state, future(), None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("valid ISO 8601"));
    }

    #[test]
    fn test_rejects_unregistered_repo_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let _tmp_repos = tempfile::TempDir::new().expect("create temp dir");
        let empty_state = AppState::new_with_path(
            ReposConfig { version: 1, repos: vec![] },
            _tmp_repos.path().join("repos.json"),
        );
        let result = update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some(schedule_tomorrow()), &empty_state, future(), None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    #[test]
    fn test_atomic_write_leaves_no_tmp_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some(schedule_tomorrow()), &state, future(), None,
        ).unwrap();
        assert!(!dir.path().join(".postlane/posts/post-001/meta.json.tmp").exists());
    }

    #[test]
    fn test_stores_timezone_when_setting_schedule() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some(schedule_tomorrow()), &state, future(),
            Some("America/New_York"),
        ).unwrap();
        let meta = crate::post_mutations::read_post_meta(
            &dir.path().join(".postlane/posts/post-001/meta.json")
        ).unwrap();
        assert_eq!(meta.schedule_timezone.as_deref(), Some("America/New_York"), "timezone should be stored");
    }

    #[test]
    fn test_clears_timezone_when_clearing_schedule() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta_in(dir.path(), "post-001");
        let mut meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        meta.schedule = Some(schedule_tomorrow().to_string());
        meta.schedule_timezone = Some("America/New_York".to_string());
        crate::post_mutations::write_post_meta(&meta_path, &meta).unwrap();
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        update_post_schedule_impl(dir.path().to_str().unwrap(), "post-001", None, &state, future(), None).unwrap();
        let meta2 = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert!(meta2.schedule_timezone.is_none(), "timezone should be cleared when schedule is cleared");
    }

    #[test]
    fn test_sets_schedule_source_to_user_when_setting_schedule() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta_in(dir.path(), "post-001");
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        update_post_schedule_impl(
            dir.path().to_str().unwrap(), "post-001", Some(schedule_tomorrow()), &state, future(), None,
        ).unwrap();
        let meta = crate::post_mutations::read_post_meta(
            &dir.path().join(".postlane/posts/post-001/meta.json")
        ).unwrap();
        assert_eq!(meta.schedule_source.as_deref(), Some("user"), "schedule_source should be 'user' when set by user");
    }

    #[test]
    fn test_clears_schedule_source_when_clearing_schedule() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta_in(dir.path(), "post-001");
        let mut meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        meta.schedule = Some(schedule_tomorrow().to_string());
        meta.schedule_source = Some("user".to_string());
        crate::post_mutations::write_post_meta(&meta_path, &meta).unwrap();
        let (state, _tmp_repos) = make_state_with_dir(dir.path());
        update_post_schedule_impl(dir.path().to_str().unwrap(), "post-001", None, &state, future(), None).unwrap();
        let meta2 = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert!(meta2.schedule_source.is_none(), "schedule_source should be None when schedule is cleared");
    }
}
