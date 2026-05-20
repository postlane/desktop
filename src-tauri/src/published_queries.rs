// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::post_io::{collect_posts_from_dir, collect_posts_from_repos, parse_published_post, sort_by_status_priority_then_timestamp};
use crate::types::Post;
use std::path::PathBuf;
use tauri::State;

pub fn get_repo_published_impl(
    repo_id: &str,
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<Post>, String> {
    let repos = state.lock_repos()?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;

    let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
    if !posts_dir.exists() {
        return Ok(vec![]);
    }

    let mut posts = collect_posts_from_dir(&posts_dir, |p| {
        parse_published_post(p, &repo.id, &repo.name, &repo.path)
    });
    sort_by_status_priority_then_timestamp(&mut posts, "queued", "sent", |p| &p.status, |p| p.sent_at.as_deref());
    Ok(posts.into_iter().skip(offset).take(limit).collect())
}

pub fn get_all_published_impl(
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<Post>, String> {
    let repos = state.lock_repos()?;
    let mut posts = collect_posts_from_repos(&repos.repos, false, parse_published_post);
    posts.sort_by(|a, b| match (&b.sent_at, &a.sent_at) {
        (Some(bt), Some(at)) => bt.cmp(at),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    Ok(posts.into_iter().skip(offset).take(limit).collect())
}

#[tauri::command]
pub fn get_repo_published(
    repo_id: String,
    offset: usize,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<Post>, String> {
    get_repo_published_impl(&repo_id, offset, limit, &state)
}

#[tauri::command]
pub fn get_all_published(
    offset: usize,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<Post>, String> {
    get_all_published_impl(offset, limit, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::make_state;
    use std::fs;

    fn write_published_meta(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_repo_published_empty() {
        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: "/nonexistent".to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_repo_published_only_sent_and_queued() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "p1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_published_meta(dir.path(), "p2", r#"{}"#); // no sent_platforms, no scheduled_for → excluded
        write_published_meta(dir.path(), "p3", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.path().to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2, "only sent + queued");
    }

    #[test]
    fn test_get_repo_published_queued_before_sent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "p1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_published_meta(dir.path(), "p2", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.path().to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result[0].status, "queued");
        assert_eq!(result[1].status, "sent");
    }

    #[test]
    fn test_get_repo_published_pagination() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        for i in 0..105 {
            write_published_meta(
                dir.path(),
                &format!("post-{:03}", i),
                &format!(r#"{{"sent_platforms":{{"x":"2026-04-{:02}T10:00:00Z"}}}}"#, (i % 28) + 1),
            );
        }

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.path().to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let page1 = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(page1.len(), 100);

        let page2 = get_repo_published_impl("r1", 100, 100, &state).expect("ok");
        assert_eq!(page2.len(), 5);
    }

    #[test]
    fn test_get_repo_published_repo_not_found() {
        let state = make_state(vec![]);
        let result = get_repo_published_impl("nonexistent", 0, 100, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_field_included_in_published_post() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("config.json"),
            r#"{"scheduler":{"provider":"zernio"}}"#,
        )
        .expect("write config");
        write_published_meta(
            dir.path(),
            "post-abc",
            r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#,
        );

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(),
            name: "Repo".to_string(),
            path: dir.path().to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].provider, Some("zernio".to_string()));
    }

    // -----------------------------------------------------------------------
    // get_all_published_impl
    // -----------------------------------------------------------------------

    fn make_repo(id: &str, name: &str, path: &str) -> crate::storage::Repo {
        crate::storage::Repo {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_get_all_published_empty_repos() {
        let state = make_state(vec![]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_published_repo_with_no_posts_dir() {
        let state = make_state(vec![make_repo("r1", "repo", "/nonexistent/path")]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_published_filters_to_sent_and_queued_only() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "p1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_published_meta(dir.path(), "p2", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);
        write_published_meta(dir.path(), "p3", r#"{}"#);
        write_published_meta(dir.path(), "p4", r#"{}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.status == "sent" || p.status == "queued"));
    }

    #[test]
    fn test_get_all_published_merges_across_multiple_repos() {
        let dir1 = tempfile::TempDir::new().expect("create temp dir");
        let dir2 = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir1.path(), "p1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_published_meta(dir2.path(), "p2", r#"{"sent_platforms":{"bluesky":"2026-04-14T10:00:00Z"}}"#);

        let state = make_state(vec![
            make_repo("r1", "repo-one", dir1.path().to_str().unwrap()),
            make_repo("r2", "repo-two", dir2.path().to_str().unwrap()),
        ]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        // Most recent first
        assert_eq!(result[0].repo_id, "r1");
        assert_eq!(result[1].repo_id, "r2");
    }

    #[test]
    fn test_get_all_published_sorts_by_sent_at_descending() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "old", r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z"}}"#);
        write_published_meta(dir.path(), "new", r#"{"sent_platforms":{"x":"2026-04-20T00:00:00Z"}}"#);
        write_published_meta(dir.path(), "mid", r#"{"sent_platforms":{"x":"2026-03-01T00:00:00Z"}}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].post_folder, "new");
        assert_eq!(result[1].post_folder, "mid");
        assert_eq!(result[2].post_folder, "old");
    }

    #[test]
    fn test_get_all_published_none_sent_at_sorts_first() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "with-time", r#"{"sent_platforms":{"x":"2026-04-20T00:00:00Z"}}"#);
        write_published_meta(dir.path(), "no-time", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        // None sent_at (queued) sorts before Some sent_at (sent)
        assert!(result[0].sent_at.is_none());
        assert!(result[1].sent_at.is_some());
    }

    #[test]
    fn test_get_all_published_two_none_sent_at_are_equal() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "q1", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);
        write_published_meta(dir.path(), "q2", r#"{"scheduled_for":"2026-06-02T10:00:00Z"}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_all_published_pagination() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        for i in 0..15u32 {
            write_published_meta(
                dir.path(),
                &format!("post-{:02}", i),
                &format!(r#"{{"sent_platforms":{{"x":"2026-04-{:02}T00:00:00Z"}}}}"#, (i % 28) + 1),
            );
        }

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let page1 = get_all_published_impl(0, 10, &state).expect("ok");
        assert_eq!(page1.len(), 10);
        let page2 = get_all_published_impl(10, 10, &state).expect("ok");
        assert_eq!(page2.len(), 5);
        // Pages are disjoint
        let ids1: Vec<_> = page1.iter().map(|p| &p.post_folder).collect();
        let ids2: Vec<_> = page2.iter().map(|p| &p.post_folder).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }

    #[test]
    fn test_get_all_published_includes_optional_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "p1", r#"{
            "sent_platforms":{"x":"2026-04-15T10:00:00Z"},
            "scheduler_ids":{"x":"sched-123"},
            "platform_urls":{"x":"https://x.com/post/123"},
            "model_name":"claude-3-5-sonnet"
        }"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1);
        let post = &result[0];
        assert!(post.platform_results.is_none());
        assert_eq!(post.scheduler_ids.as_ref().unwrap().get("x").map(String::as_str), Some("sched-123"));
        assert_eq!(post.platform_urls.as_ref().unwrap().get("x").map(String::as_str), Some("https://x.com/post/123"));
        assert_eq!(post.llm_model.as_deref(), Some("claude-3-5-sonnet"));
    }

    #[test]
    fn test_get_all_published_skips_invalid_meta_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "valid", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        // Write invalid JSON directly
        let bad_dir = dir.path().join(".postlane/posts/bad-post");
        fs::create_dir_all(&bad_dir).expect("create bad dir");
        fs::write(bad_dir.join("meta.json"), b"not json").expect("write bad meta");

        let state = make_state(vec![make_repo("r1", "repo", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1, "invalid meta.json must be skipped");
    }

    #[test]
    fn test_get_all_published_carries_repo_name_and_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_published_meta(dir.path(), "p1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);

        let state = make_state(vec![make_repo("r1", "my-project", dir.path().to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result[0].repo_id, "r1");
        assert_eq!(result[0].repo_name, "my-project");
        assert_eq!(result[0].repo_path, dir.path().to_str().unwrap());
    }
}
