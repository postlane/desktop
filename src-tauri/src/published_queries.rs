// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Reads the scheduler provider from `.postlane/config.json` for a given repo path.
/// Returns `None` if the file is missing, unreadable, or has no `scheduler.provider` field.
fn read_repo_provider(repo_path: &str) -> Option<String> {
    let config_path = std::path::PathBuf::from(repo_path)
        .join(".postlane/config.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("scheduler")
        .and_then(|s| s.get("provider"))
        .and_then(|p| p.as_str())
        .map(String::from)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PublishedPost {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_path: String,
    pub post_folder: String,
    pub status: String,
    pub platforms: Vec<String>,
    pub platform_results: Option<std::collections::HashMap<String, String>>,
    pub schedule: Option<String>,
    pub scheduler_ids: Option<std::collections::HashMap<String, String>>,
    pub platform_urls: Option<std::collections::HashMap<String, String>>,
    /// Scheduler provider name from repo config.json (e.g. "zernio"), or None.
    pub provider: Option<String>,
    pub llm_model: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: Option<String>,
}

pub fn get_repo_published_impl(
    repo_id: &str,
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<PublishedPost>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;

    let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
    if !posts_dir.exists() {
        return Ok(vec![]);
    }

    let entries = match fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(_) => return Ok(vec![]),
    };

    let mut posts: Vec<PublishedPost> = Vec::new();

    for entry in entries.flatten() {
        let post_path = entry.path();
        if !post_path.is_dir() {
            continue;
        }
        let meta_path = post_path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let status = match meta.get("status").and_then(|s| s.as_str()) {
            Some(s @ "sent") | Some(s @ "queued") => s.to_string(),
            _ => continue,
        };

        let post_folder = post_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let platforms: Vec<String> = meta
            .get("platforms")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let platform_results = meta.get("platform_results").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
        });

        let scheduler_ids = meta.get("scheduler_ids").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
        });

        let platform_urls = meta.get("platform_urls").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
        });

        posts.push(PublishedPost {
            repo_id: repo.id.clone(),
            repo_name: repo.name.clone(),
            repo_path: repo.path.clone(),
            post_folder,
            status,
            platforms,
            platform_results,
            schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
            scheduler_ids,
            platform_urls,
            provider: read_repo_provider(&repo.path),
            llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
            sent_at: meta.get("sent_at").and_then(|v| v.as_str()).map(String::from),
            created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
        });
    }

    posts.sort_by(|a, b| {
        match (a.status.as_str(), b.status.as_str()) {
            ("queued", "sent") => std::cmp::Ordering::Less,
            ("sent", "queued") => std::cmp::Ordering::Greater,
            _ => match (&b.sent_at, &a.sent_at) {
                (Some(bt), Some(at)) => bt.cmp(at),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });

    let page: Vec<PublishedPost> = posts.into_iter().skip(offset).take(limit).collect();
    Ok(page)
}

pub fn get_all_published_impl(
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<PublishedPost>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut posts: Vec<PublishedPost> = Vec::new();

    for repo in &repos.repos {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }

        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let post_path = entry.path();
            if !post_path.is_dir() {
                continue;
            }
            let meta_path = post_path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            let content = match fs::read_to_string(&meta_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let status = match meta.get("status").and_then(|s| s.as_str()) {
                Some(s @ "sent") | Some(s @ "queued") => s.to_string(),
                _ => continue,
            };

            let post_folder = post_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let platforms: Vec<String> = meta
                .get("platforms")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let platform_results = meta.get("platform_results").and_then(|v| {
                v.as_object().map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
            });

            let scheduler_ids = meta.get("scheduler_ids").and_then(|v| {
                v.as_object().map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
            });

            let platform_urls = meta.get("platform_urls").and_then(|v| {
                v.as_object().map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
            });

            posts.push(PublishedPost {
                repo_id: repo.id.clone(),
                repo_name: repo.name.clone(),
                repo_path: repo.path.clone(),
                post_folder,
                status,
                platforms,
                platform_results,
                schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
                scheduler_ids,
                platform_urls,
                provider: read_repo_provider(&repo.path),
                llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
                sent_at: meta.get("sent_at").and_then(|v| v.as_str()).map(String::from),
                created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
            });
        }
    }

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
) -> Result<Vec<PublishedPost>, String> {
    get_repo_published_impl(&repo_id, offset, limit, &state)
}

#[tauri::command]
pub fn get_all_published(
    offset: usize,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<PublishedPost>, String> {
    get_all_published_impl(offset, limit, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_state(repos: Vec<Repo>) -> AppState {
        AppState::new(ReposConfig { version: 1, repos })
    }

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
        let dir = std::env::temp_dir().join("postlane_test_published_filter_pq");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir, "p2", r#"{"status":"ready","platforms":["x"]}"#);
        write_published_meta(&dir, "p3", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2, "only sent + queued");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_queued_before_sent() {
        let dir = std::env::temp_dir().join("postlane_test_published_sort_pq");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir, "p2", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result[0].status, "queued");
        assert_eq!(result[1].status, "sent");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_pagination() {
        let dir = std::env::temp_dir().join("postlane_test_published_pagination_pq");
        for i in 0..105 {
            write_published_meta(
                &dir,
                &format!("post-{:03}", i),
                &format!(r#"{{"status":"sent","platforms":["x"],"sent_at":"2026-04-{:02}T10:00:00Z"}}"#, (i % 28) + 1),
            );
        }

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let page1 = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(page1.len(), 100);

        let page2 = get_repo_published_impl("r1", 100, 100, &state).expect("ok");
        assert_eq!(page2.len(), 5);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_repo_not_found() {
        let state = make_state(vec![]);
        let result = get_repo_published_impl("nonexistent", 0, 100, &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_repo_provider_returns_provider_from_config() {
        let dir = std::env::temp_dir().join("postlane_test_read_provider");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("config.json"),
            r#"{"scheduler":{"provider":"zernio"}}"#,
        )
        .expect("write config");

        let result = read_repo_provider(dir.to_str().unwrap());
        assert_eq!(result, Some("zernio".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_repo_provider_returns_none_when_config_missing() {
        let result = read_repo_provider("/path/that/does/not/exist");
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_repo_provider_returns_none_when_field_absent() {
        let dir = std::env::temp_dir().join("postlane_test_read_provider_no_field");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(config_dir.join("config.json"), r#"{"other":"data"}"#)
            .expect("write config");

        let result = read_repo_provider(dir.to_str().unwrap());
        assert_eq!(result, None);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_provider_field_included_in_published_post() {
        let dir = std::env::temp_dir().join("postlane_test_provider_in_post");
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("config.json"),
            r#"{"scheduler":{"provider":"zernio"}}"#,
        )
        .expect("write config");
        write_published_meta(
            &dir,
            "post-abc",
            r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#,
        );

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(),
            name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].provider, Some("zernio".to_string()));

        let _ = fs::remove_dir_all(&dir);
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
        let dir = std::env::temp_dir().join("postlane_test_all_pub_filter");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir, "p2", r#"{"status":"queued","platforms":["x"]}"#);
        write_published_meta(&dir, "p3", r#"{"status":"ready","platforms":["x"]}"#);
        write_published_meta(&dir, "p4", r#"{"status":"failed","platforms":["x"]}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|p| p.status == "sent" || p.status == "queued"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_merges_across_multiple_repos() {
        let dir1 = std::env::temp_dir().join("postlane_test_all_pub_multi_r1");
        let dir2 = std::env::temp_dir().join("postlane_test_all_pub_multi_r2");
        write_published_meta(&dir1, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir2, "p2", r#"{"status":"sent","platforms":["bluesky"],"sent_at":"2026-04-14T10:00:00Z"}"#);

        let state = make_state(vec![
            make_repo("r1", "repo-one", dir1.to_str().unwrap()),
            make_repo("r2", "repo-two", dir2.to_str().unwrap()),
        ]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        // Most recent first
        assert_eq!(result[0].repo_id, "r1");
        assert_eq!(result[1].repo_id, "r2");
        let _ = fs::remove_dir_all(&dir1);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn test_get_all_published_sorts_by_sent_at_descending() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_sort");
        write_published_meta(&dir, "old", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-01-01T00:00:00Z"}"#);
        write_published_meta(&dir, "new", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-20T00:00:00Z"}"#);
        write_published_meta(&dir, "mid", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-03-01T00:00:00Z"}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].post_folder, "new");
        assert_eq!(result[1].post_folder, "mid");
        assert_eq!(result[2].post_folder, "old");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_none_sent_at_sorts_first() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_none_sent");
        write_published_meta(&dir, "with-time", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-20T00:00:00Z"}"#);
        write_published_meta(&dir, "no-time", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        // None sent_at (queued) sorts before Some sent_at (sent)
        assert!(result[0].sent_at.is_none());
        assert!(result[1].sent_at.is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_two_none_sent_at_are_equal() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_two_none");
        write_published_meta(&dir, "q1", r#"{"status":"queued","platforms":["x"]}"#);
        write_published_meta(&dir, "q2", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_pagination() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_pages");
        for i in 0..15u32 {
            write_published_meta(
                &dir,
                &format!("post-{:02}", i),
                &format!(r#"{{"status":"sent","platforms":["x"],"sent_at":"2026-04-{:02}T00:00:00Z"}}"#, (i % 28) + 1),
            );
        }

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let page1 = get_all_published_impl(0, 10, &state).expect("ok");
        assert_eq!(page1.len(), 10);
        let page2 = get_all_published_impl(10, 10, &state).expect("ok");
        assert_eq!(page2.len(), 5);
        // Pages are disjoint
        let ids1: Vec<_> = page1.iter().map(|p| &p.post_folder).collect();
        let ids2: Vec<_> = page2.iter().map(|p| &p.post_folder).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_includes_optional_fields() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_fields");
        write_published_meta(&dir, "p1", r#"{
            "status":"sent","platforms":["x"],
            "sent_at":"2026-04-15T10:00:00Z",
            "platform_results":{"x":"sent"},
            "scheduler_ids":{"x":"sched-123"},
            "platform_urls":{"x":"https://x.com/post/123"},
            "schedule":"2026-04-15T10:00:00Z",
            "llm_model":"claude-3-5-sonnet"
        }"#);

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1);
        let post = &result[0];
        assert_eq!(post.platform_results.as_ref().unwrap().get("x").map(String::as_str), Some("sent"));
        assert_eq!(post.scheduler_ids.as_ref().unwrap().get("x").map(String::as_str), Some("sched-123"));
        assert_eq!(post.platform_urls.as_ref().unwrap().get("x").map(String::as_str), Some("https://x.com/post/123"));
        assert_eq!(post.llm_model.as_deref(), Some("claude-3-5-sonnet"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_skips_invalid_meta_json() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_invalid");
        write_published_meta(&dir, "valid", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        // Write invalid JSON directly
        let bad_dir = dir.join(".postlane/posts/bad-post");
        fs::create_dir_all(&bad_dir).expect("create bad dir");
        fs::write(bad_dir.join("meta.json"), b"not json").expect("write bad meta");

        let state = make_state(vec![make_repo("r1", "repo", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result.len(), 1, "invalid meta.json must be skipped");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_published_carries_repo_name_and_path() {
        let dir = std::env::temp_dir().join("postlane_test_all_pub_meta");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);

        let state = make_state(vec![make_repo("r1", "my-project", dir.to_str().unwrap())]);
        let result = get_all_published_impl(0, 100, &state).expect("ok");
        assert_eq!(result[0].repo_id, "r1");
        assert_eq!(result[0].repo_name, "my-project");
        assert_eq!(result[0].repo_path, dir.to_str().unwrap());
        let _ = fs::remove_dir_all(&dir);
    }
}
