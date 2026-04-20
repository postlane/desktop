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
}
