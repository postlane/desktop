// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::post_meta::PostMeta;
use crate::project_config_ops::read_project_id_from_path_impl;
use crate::storage::{Repo, ReposConfig};
use crate::types::PublishedPost;
use std::path::{Path, PathBuf};
use tauri::State;

/// Returns `true` iff `path` is absolute and begins with `home_dir`.
fn is_within_home(path: &Path, home_dir: &Path) -> bool {
    path.is_absolute() && path.starts_with(home_dir)
}

/// Returns `true` iff `s` is a single filesystem component (no `/`, `..`, etc.).
fn is_single_component(s: &str) -> bool {
    Path::new(s).components().count() == 1
}

/// Read platform `.md` text from a post folder; returns empty string if absent.
fn read_platform_text(posts_dir: &Path, post_folder: &str, platform: &str) -> String {
    let md_path = posts_dir.join(post_folder).join(format!("{}.md", platform));
    std::fs::read_to_string(&md_path).unwrap_or_default()
}

/// Collect `PublishedPost` rows from one post folder for all valid sent platforms.
fn collect_from_post_folder(
    posts_dir: &Path,
    post_folder: &str,
    repo_path: &str,
    project_id: &str,
) -> Vec<PublishedPost> {
    let meta_path = PostMeta::path_for(Path::new(repo_path), post_folder);
    let meta = match PostMeta::load(&meta_path) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("[get_org_published] could not load meta for {}/{}: {}", repo_path, post_folder, e);
            return vec![];
        }
    };
    meta.sent_platforms
        .iter()
        .filter_map(|(platform, sent_at)| {
            if !is_single_component(platform) {
                log::warn!("[get_org_published] skipping platform key with traversal: {}", platform);
                return None;
            }
            let text = read_platform_text(posts_dir, post_folder, platform);
            Some(PublishedPost {
                text,
                platform: platform.clone(),
                repo_path: repo_path.to_string(),
                post_folder: post_folder.to_string(),
                sent_at: sent_at.clone(),
                project_id: Some(project_id.to_string()),
                scheduler_ids: meta.scheduler_ids.clone(),
                platform_urls: meta.platform_urls.clone(),
                platform_results: std::collections::HashMap::new(),
            })
        })
        .collect()
}

/// Collect all published posts from one repo that match `project_id`.
/// Returns empty if the repo is outside home, has no config, or has a mismatched project_id.
fn collect_from_repo(repo: &Repo, project_id: &str, home_dir: &Path, repos: &ReposConfig) -> Vec<PublishedPost> {
    let repo_path = PathBuf::from(&repo.path);
    if !is_within_home(&repo_path, home_dir) {
        log::warn!("[get_org_published] skipping repo outside home: {}", repo.path);
        return vec![];
    }
    let repo_pid = match read_project_id_from_path_impl(&repo.path, repos) {
        Ok(Some(pid)) if pid == project_id => pid,
        Ok(_) => return vec![],
        Err(e) => {
            log::warn!("[get_org_published] could not read project_id for {}: {}", repo.path, e);
            return vec![];
        }
    };
    let posts_dir = repo_path.join(".postlane/posts");
    if !posts_dir.exists() {
        return vec![];
    }
    let entries = match std::fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[get_org_published] could not read posts dir {}: {}", posts_dir.display(), e);
            return vec![];
        }
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.path().file_name()?.to_str().map(str::to_string))
        .filter(|folder| {
            if is_single_component(folder) { true } else {
                log::warn!("[get_org_published] skipping post_folder with traversal: {}", folder);
                false
            }
        })
        .flat_map(|folder| collect_from_post_folder(&posts_dir, &folder, &repo.path, &repo_pid))
        .collect()
}

pub fn get_org_published_impl(project_id: &str, state: &AppState) -> Result<Vec<PublishedPost>, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory".to_string())?;
    let repos = state.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))?;
    let mut results: Vec<PublishedPost> = repos
        .repos
        .iter()
        .filter(|r| r.active)
        .flat_map(|repo| collect_from_repo(repo, project_id, &home_dir, &repos))
        .collect();
    results.sort_by(|a, b| b.sent_at.cmp(&a.sent_at));
    Ok(results)
}

#[tauri::command]
pub fn get_org_published(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<PublishedPost>, String> {
    get_org_published_impl(&project_id, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_state, make_repo, home_tmp, write_config, write_meta};
    use std::fs;

    fn write_platform_md(dir: &Path, folder: &str, platform: &str, text: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join(format!("{}.md", platform)), text).expect("write platform md");
    }

    #[test]
    fn test_get_org_published_returns_posts_for_project() {
        let dir = home_tmp("gop_returns");
        write_config(&dir, r#"{"project_id":"proj-abc"}"#);
        write_meta(&dir, "post-1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_platform_md(&dir, "post-1", "x", "Hello world");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_org_published_impl("proj-abc", &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].platform, "x");
        assert_eq!(result[0].text, "Hello world");
        assert_eq!(result[0].sent_at, "2026-04-15T10:00:00Z");
        assert_eq!(result[0].project_id, Some("proj-abc".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_org_published_excludes_other_projects() {
        let dir1 = home_tmp("gop_excl_r1");
        let dir2 = home_tmp("gop_excl_r2");
        write_config(&dir1, r#"{"project_id":"proj-abc"}"#);
        write_meta(&dir1, "post-1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);
        write_config(&dir2, r#"{"project_id":"proj-xyz"}"#);
        write_meta(&dir2, "post-2", r#"{"sent_platforms":{"x":"2026-04-16T10:00:00Z"}}"#);

        let state = make_state(vec![
            make_repo("r1", dir1.to_str().unwrap()),
            make_repo("r2", dir2.to_str().unwrap()),
        ]);
        let result = get_org_published_impl("proj-abc", &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].repo_path, dir1.to_str().unwrap());
        let _ = fs::remove_dir_all(&dir1);
        let _ = fs::remove_dir_all(&dir2);
    }

    #[test]
    fn test_get_org_published_returns_empty_for_unknown_project() {
        let dir = home_tmp("gop_unknown");
        write_config(&dir, r#"{"project_id":"proj-abc"}"#);
        write_meta(&dir, "post-1", r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#);

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_org_published_impl("proj-nonexistent", &state).expect("ok");
        assert!(result.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_org_published_validates_repo_paths_are_within_home_directory() {
        // /tmp is outside $HOME on macOS (home is /Users/...) — skipped with warn
        let state = make_state(vec![make_repo("r1", "/tmp/definitely-not-home")]);
        let result = get_org_published_impl("any-project", &state).expect("ok");
        assert!(result.is_empty(), "repo outside home must be skipped");
    }

    #[test]
    fn test_get_org_published_skips_post_folder_with_path_traversal() {
        let dir = home_tmp("gop_folder_traversal");
        write_config(&dir, r#"{"project_id":"proj-abc"}"#);
        // Create a multi-segment directory inside posts/ — read_dir returns "sub" (single component),
        // but meta.json is nested inside sub/evil so "sub" has no meta.json and is skipped.
        let evil_dir = dir.join(".postlane/posts/sub/evil");
        fs::create_dir_all(&evil_dir).expect("create multi-segment post dir");
        fs::write(evil_dir.join("meta.json"), r#"{"sent_platforms":{"x":"2026-04-15T10:00:00Z"}}"#)
            .expect("write meta");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_org_published_impl("proj-abc", &state).expect("ok");
        assert!(result.is_empty(), "multi-segment post folder path must produce no rows");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_org_published_skips_platform_key_with_path_traversal() {
        let dir = home_tmp("gop_platform_traversal");
        write_config(&dir, r#"{"project_id":"proj-abc"}"#);
        // Platform key "../evil" in sent_platforms — validated and skipped; "x" is valid
        write_meta(&dir, "post-1", r#"{"sent_platforms":{"../evil":"2026-04-15T10:00:00Z","x":"2026-04-16T10:00:00Z"}}"#);
        write_platform_md(&dir, "post-1", "x", "valid post");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_org_published_impl("proj-abc", &state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].platform, "x");
        let _ = fs::remove_dir_all(&dir);
    }
}
