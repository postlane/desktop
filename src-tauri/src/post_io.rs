// SPDX-License-Identifier: BUSL-1.1

use crate::post_meta::PostMeta;
use crate::types::Post;
use std::{cmp::Ordering, fs, path::{Path, PathBuf}};

/// Parse a single post directory into a published `Post`.
/// Returns `None` if the post has neither sent platforms nor a scheduled time.
pub(crate) fn parse_published_post(
    post_path: &Path,
    repo_id: &str,
    repo_name: &str,
    repo_path: &str,
) -> Option<Post> {
    if !post_path.is_dir() {
        return None;
    }
    let post_folder = post_path.file_name()?.to_str()?.to_string();
    let meta_path = PostMeta::path_for(Path::new(repo_path), &post_folder);
    let meta = PostMeta::load(&meta_path).ok()?;

    let status = if !meta.sent_platforms.is_empty() {
        "sent".to_string()
    } else if meta.scheduled_for.is_some() {
        "queued".to_string()
    } else {
        return None;
    };

    let mut platforms: Vec<String> = meta.sent_platforms.keys().cloned().collect();
    platforms.sort();
    let sent_at = meta.sent_platforms.values().min().cloned();

    Some(Post {
        repo_id: repo_id.to_string(),
        repo_name: repo_name.to_string(),
        repo_path: repo_path.to_string(),
        post_folder,
        status,
        platforms,
        platform_results: None,
        schedule: meta.scheduled_for,
        schedule_source: None,
        scheduler_ids: Some(meta.scheduler_ids).filter(|m| !m.is_empty()),
        platform_urls: Some(meta.platform_urls).filter(|m| !m.is_empty()),
        provider: read_repo_config_provider(repo_path),
        llm_model: meta.model_name.clone(),
        sent_at,
        created_at: None,
        trigger: None,
        error: meta.error,
        image_url: None,
        image_attribution: None,
        project_id: None,
        model_name: meta.model_name,
        scheduled_for: None,
        edited_at: None,
        platform: String::default(),
        text: String::default(),
    })
}

/// Collect posts from a directory by applying `parse` to each entry path.
/// Returns an empty vec if the directory cannot be read.
pub(crate) fn collect_posts_from_dir<T>(
    posts_dir: &Path,
    parse: impl Fn(&Path) -> Option<T>,
) -> Vec<T> {
    match fs::read_dir(posts_dir) {
        Ok(entries) => entries.flatten().filter_map(|e| parse(&e.path())).collect(),
        Err(_) => vec![],
    }
}

/// Read `scheduler.provider` from a repo's `.postlane/config.json`.
/// Returns `None` if the file is absent, unreadable, or the field is missing.
pub(crate) fn read_repo_config_provider(repo_path: &str) -> Option<String> {
    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    let v: serde_json::Value = crate::init::read_json_file(&config_path).ok()?;
    v.get("scheduler")
        .and_then(|s| s.get("provider"))
        .and_then(|p| p.as_str())
        .map(String::from)
}

/// Iterate repos, entering each `.postlane/posts` directory, and collect results
/// by calling `parse(post_path, repo_id, repo_name, repo_path)` for every entry.
///
/// When `active_only` is true, inactive repos are skipped entirely.
pub(crate) fn collect_posts_from_repos<T>(
    repos: &[crate::storage::Repo],
    active_only: bool,
    parse: impl Fn(&Path, &str, &str, &str) -> Option<T>,
) -> Vec<T> {
    let mut results = Vec::new();
    for repo in repos.iter().filter(|r| !active_only || r.active) {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }
        results.extend(collect_posts_from_dir(&posts_dir, |p| {
            parse(p, &repo.id, &repo.name, &repo.path)
        }));
    }
    results
}

/// Sort a slice so that items with `priority_status` come before items with `other_status`,
/// then by `get_timestamp` descending within each group.
pub(crate) fn sort_by_status_priority_then_timestamp<T>(
    items: &mut [T],
    priority_status: &str,
    other_status: &str,
    get_status: impl Fn(&T) -> &str,
    get_timestamp: impl Fn(&T) -> Option<&str>,
) {
    items.sort_by(|a, b| {
        let sa = get_status(a);
        let sb = get_status(b);
        if sa == priority_status && sb == other_status {
            return Ordering::Less;
        }
        if sa == other_status && sb == priority_status {
            return Ordering::Greater;
        }
        compare_timestamps_desc(get_timestamp(b), get_timestamp(a))
    });
}

fn compare_timestamps_desc(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(ta), Some(tb)) => ta.cmp(tb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- collect_posts_from_dir ---

    #[test]
    fn collect_posts_from_dir_returns_empty_for_missing_dir() {
        let result = collect_posts_from_dir(Path::new("/nonexistent/path/xyz"), |_| Some(1u32));
        assert!(result.is_empty());
    }

    #[test]
    fn collect_posts_from_dir_returns_parsed_entries() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("a.txt"), "").unwrap();
        fs::write(dir.path().join("b.txt"), "").unwrap();

        let mut results: Vec<String> = collect_posts_from_dir(dir.path(), |p| {
            p.file_name().and_then(|n| n.to_str()).map(String::from)
        });
        results.sort();

        assert_eq!(results, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn collect_posts_from_dir_skips_entries_returning_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("keep.txt"), "").unwrap();
        fs::write(dir.path().join("skip.txt"), "").unwrap();

        let results: Vec<String> = collect_posts_from_dir(dir.path(), |p| {
            let name = p.file_name()?.to_str()?.to_string();
            if name.starts_with("skip") { None } else { Some(name) }
        });

        assert_eq!(results, vec!["keep.txt"]);
    }

    // --- sort_by_status_priority_then_timestamp ---

    #[derive(Debug, PartialEq)]
    struct Item { status: String, ts: Option<String> }

    fn item(status: &str, ts: Option<&str>) -> Item {
        Item { status: status.into(), ts: ts.map(String::from) }
    }

    #[test]
    fn priority_status_comes_before_other_status() {
        let mut items = vec![item("ready", Some("2024-01-02")), item("failed", Some("2024-01-01"))];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert_eq!(items[0].status, "failed");
        assert_eq!(items[1].status, "ready");
    }

    #[test]
    fn same_status_sorted_by_timestamp_descending() {
        let mut items = vec![
            item("ready", Some("2024-01-01")),
            item("ready", Some("2024-01-03")),
            item("ready", Some("2024-01-02")),
        ];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert_eq!(items[0].ts.as_deref(), Some("2024-01-03"));
        assert_eq!(items[1].ts.as_deref(), Some("2024-01-02"));
        assert_eq!(items[2].ts.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn none_timestamp_sorted_before_some() {
        // Matches original sort_drafts / sort_published_by_status_then_sent_at behaviour:
        // items with no timestamp sort ahead of those with one (None treated as "earliest"
        // in the descending key, so it wins the Less branch).
        let mut items = vec![item("ready", Some("2024-01-01")), item("ready", None)];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert!(items[0].ts.is_none());
        assert_eq!(items[1].ts.as_deref(), Some("2024-01-01"));
    }

    // --- collect_posts_from_repos ---

    fn make_repo(id: &str, path: &str, active: bool) -> crate::storage::Repo {
        crate::storage::Repo {
            id: id.into(), name: id.into(),
            path: path.into(), active,
            added_at: "2024-01-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn collect_posts_from_repos_gathers_from_all_repos() {
        let dir1 = tempfile::TempDir::new().expect("create temp dir");
        let dir2 = tempfile::TempDir::new().expect("create temp dir");
        for d in [dir1.path(), dir2.path()] {
            let posts = d.join(".postlane/posts/p1");
            fs::create_dir_all(&posts).unwrap();
            fs::write(posts.join("meta.json"), "{}").unwrap();
        }

        let repos = vec![
            make_repo("r1", dir1.path().to_str().unwrap(), true),
            make_repo("r2", dir2.path().to_str().unwrap(), true),
        ];
        let results: Vec<String> = collect_posts_from_repos(&repos, false, |_p, id, _name, _path| Some(id.to_string()));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn collect_posts_from_repos_skips_inactive_when_active_only() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts = dir.path().join(".postlane/posts/p1");
        fs::create_dir_all(&posts).unwrap();
        fs::write(posts.join("meta.json"), "{}").unwrap();

        let repos = vec![make_repo("r1", dir.path().to_str().unwrap(), false)];
        let results: Vec<String> = collect_posts_from_repos(&repos, true, |_p, id, _name, _path| Some(id.to_string()));
        assert!(results.is_empty());
    }

    // --- read_repo_config_provider ---

    #[test]
    fn read_repo_config_provider_returns_provider_from_config() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.json"), r#"{"scheduler":{"provider":"zernio"}}"#).unwrap();
        assert_eq!(read_repo_config_provider(dir.path().to_str().unwrap()), Some("zernio".into()));
    }

    #[test]
    fn read_repo_config_provider_returns_none_when_missing() {
        assert_eq!(read_repo_config_provider("/nonexistent/path"), None);
    }

    #[test]
    fn read_repo_config_provider_returns_none_when_field_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let config_dir = dir.path().join(".postlane");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("config.json"), r#"{"other":"data"}"#).unwrap();
        assert_eq!(read_repo_config_provider(dir.path().to_str().unwrap()), None);
    }
}
