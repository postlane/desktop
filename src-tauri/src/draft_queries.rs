// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::post_meta::{PostMeta, PostStatus};
use crate::project_registry::read_project_id_from_path_impl;
use crate::storage::Repo;
use crate::types::Post;
use std::path::{Path, PathBuf};
use tauri::State;

pub type DraftPost = Post;

fn is_single_component(s: &str) -> bool {
    Path::new(s).components().count() == 1
}

fn status_str(meta: &PostMeta) -> String {
    match &meta.status {
        Some(PostStatus::Failed) => "failed".to_string(),
        _ => "ready".to_string(),
    }
}

fn build_draft(
    repo: &Repo,
    post_folder: &str,
    platform: &str,
    text: String,
    meta: &PostMeta,
    project_id: Option<String>,
) -> Post {
    Post {
        repo_id: repo.id.clone(),
        repo_name: repo.name.clone(),
        repo_path: repo.path.clone(),
        post_folder: post_folder.to_string(),
        platform: platform.to_string(),
        text,
        status: status_str(meta),
        platforms: vec![platform.to_string()],
        project_id,
        model_name: meta.model_name.clone(),
        scheduled_for: meta.scheduled_for.clone(),
        error: meta.error.clone(),
        llm_model: meta.model_name.clone(),
        schedule: None,
        schedule_source: None,
        trigger: None,
        platform_results: None,
        image_url: None,
        created_at: None,
        scheduler_ids: None,
        platform_urls: None,
        provider: None,
        sent_at: None,
        edited_at: meta.edited_at.clone(),
    }
}

fn drafts_from_folder(
    repo: &Repo,
    folder_path: &Path,
    post_folder: &str,
    project_id: Option<String>,
) -> Vec<Post> {
    let meta_path = PostMeta::path_for(Path::new(&repo.path), post_folder);
    let meta = PostMeta::load(&meta_path).unwrap_or_default();
    let Ok(entries) = std::fs::read_dir(folder_path) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "md" {
                return None;
            }
            let stem = path.file_stem()?.to_str()?;
            if !is_single_component(stem) || meta.sent_platforms.contains_key(stem) {
                return None;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            Some(build_draft(repo, post_folder, stem, text, &meta, project_id.clone()))
        })
        .collect()
}

fn drafts_from_repo(repo: &Repo) -> Vec<Post> {
    let repo_path = PathBuf::from(&repo.path);
    let posts_dir = repo_path.join(".postlane/posts");
    if !posts_dir.exists() {
        return vec![];
    }
    let project_id = read_project_id_from_path_impl(&repo.path).ok().flatten();
    let Ok(entries) = std::fs::read_dir(&posts_dir) else {
        return vec![];
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.path().file_name()?.to_str().map(str::to_string))
        .filter(|f| is_single_component(f))
        .flat_map(|folder| {
            let fp = posts_dir.join(&folder);
            drafts_from_folder(repo, &fp, &folder, project_id.clone())
        })
        .collect()
}

pub fn get_all_drafts_impl(state: &AppState) -> Result<Vec<Post>, String> {
    let repos = state.repos.lock().map_err(|e| format!("Failed to lock repos: {}", e))?;
    let mut drafts: Vec<Post> = repos
        .repos
        .iter()
        .filter(|r| r.active)
        .flat_map(drafts_from_repo)
        .collect();
    drafts.sort_by(|a, b| {
        a.repo_path
            .cmp(&b.repo_path)
            .then(a.post_folder.cmp(&b.post_folder))
            .then(a.platform.cmp(&b.platform))
    });
    Ok(drafts)
}

#[tauri::command]
pub fn get_all_drafts(state: State<'_, AppState>) -> Result<Vec<Post>, String> {
    get_all_drafts_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;
    use std::path::Path;

    fn make_state(repos: Vec<Repo>) -> AppState {
        AppState::new(ReposConfig { version: 1, repos })
    }

    fn make_repo(id: &str, path: &str) -> Repo {
        Repo {
            id: id.to_string(),
            name: id.to_string(),
            path: path.to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn write_config(dir: &Path, json: &str) {
        let d = dir.join(".postlane");
        fs::create_dir_all(&d).expect("create .postlane");
        fs::write(d.join("config.json"), json).expect("write config.json");
    }

    fn write_meta(dir: &Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join("meta.json"), json).expect("write meta.json");
    }

    fn write_md(dir: &Path, folder: &str, platform: &str, text: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join(format!("{}.md", platform)), text).expect("write md");
    }

    #[test]
    fn test_get_all_drafts_empty() {
        let state = make_state(vec![]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_drafts_inactive_repo_excluded() {
        let dir = std::env::temp_dir().join("postlane_test_gad_inactive");
        write_md(&dir, "my-post", "x", "Inactive");
        let mut repo = make_repo("r1", dir.to_str().unwrap());
        repo.active = false;
        let state = make_state(vec![repo]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_includes_project_id() {
        let dir = std::env::temp_dir().join("postlane_test_gad_project_id");
        write_config(&dir, r#"{"project_id":"proj-abc"}"#);
        write_md(&dir, "my-post", "x", "Hello");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project_id, Some("proj-abc".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_includes_scheduled_for() {
        let dir = std::env::temp_dir().join("postlane_test_gad_scheduled_for");
        write_meta(&dir, "my-post", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);
        write_md(&dir, "my-post", "x", "Hello");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].scheduled_for.as_deref(), Some("2026-06-01T10:00:00Z"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_excludes_sent_platforms() {
        let dir = std::env::temp_dir().join("postlane_test_gad_excl_sent");
        write_meta(
            &dir,
            "my-post",
            r#"{"sent_platforms":{"x":"2026-05-01T10:00:00Z"}}"#,
        );
        write_md(&dir, "my-post", "x", "Already sent");
        write_md(&dir, "my-post", "bluesky", "Not sent yet");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].platform, "bluesky");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_draft_event_disappears_from_queue_when_all_platforms_sent() {
        let dir = std::env::temp_dir().join("postlane_test_gad_all_sent");
        write_meta(
            &dir,
            "my-post",
            r#"{"sent_platforms":{"x":"2026-05-01T10:00:00Z","bluesky":"2026-05-01T10:00:00Z"}}"#,
        );
        write_md(&dir, "my-post", "x", "X post");
        write_md(&dir, "my-post", "bluesky", "Bluesky post");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "all sent → zero draft rows");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_returns_failed_status() {
        let dir = std::env::temp_dir().join("postlane_test_gad_failed");
        write_meta(
            &dir,
            "my-post",
            r#"{"status":"failed","error":"scheduler timeout"}"#,
        );
        write_md(&dir, "my-post", "x", "Failed post");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[0].error.as_deref(), Some("scheduler timeout"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_treats_absent_post_meta_as_clean() {
        let dir = std::env::temp_dir().join("postlane_test_gad_no_meta");
        write_md(&dir, "my-post", "x", "No meta");

        let state = make_state(vec![make_repo("r1", dir.to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "ready");
        assert!(result[0].error.is_none());
        assert!(result[0].scheduled_for.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_sorted_by_repo_post_folder_platform() {
        let dir_a = std::env::temp_dir().join("postlane_test_gad_sort_repo_a");
        let dir_b = std::env::temp_dir().join("postlane_test_gad_sort_repo_b");
        write_md(&dir_a, "folder-1", "x", "A x");
        write_md(&dir_a, "folder-1", "bluesky", "A bluesky");
        write_md(&dir_b, "folder-1", "x", "B x");
        write_md(&dir_b, "folder-1", "bluesky", "B bluesky");

        let path_a = dir_a.to_str().unwrap().to_string();
        let path_b = dir_b.to_str().unwrap().to_string();
        let state = make_state(vec![
            make_repo("rb", &path_b),
            make_repo("ra", &path_a),
        ]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 4);

        let (ra, rb) = if path_a < path_b { (&path_a, &path_b) } else { (&path_b, &path_a) };
        assert_eq!(&result[0].repo_path, ra);
        assert_eq!(result[0].platform, "bluesky");
        assert_eq!(&result[1].repo_path, ra);
        assert_eq!(result[1].platform, "x");
        assert_eq!(&result[2].repo_path, rb);
        assert_eq!(result[2].platform, "bluesky");
        assert_eq!(&result[3].repo_path, rb);
        assert_eq!(result[3].platform, "x");

        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }
}
