// SPDX-License-Identifier: BUSL-1.1

//! Low-level draft post scanning: folder traversal, .md file reading, and Post construction.

use crate::post_meta::{PostMeta, PostStatus};
use crate::storage::Repo;
use crate::types::Post;
use std::path::Path;

pub(crate) fn is_single_component(s: &str) -> bool {
    Path::new(s).components().count() == 1
}

pub(crate) fn status_str(meta: &PostMeta) -> String {
    match &meta.status {
        Some(PostStatus::Failed) => "failed".to_string(),
        _ => "ready".to_string(),
    }
}

pub(crate) fn build_draft(
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
        image_url: meta.image_url.clone(),
        image_attribution: meta.image_attribution.clone(),
        created_at: None,
        scheduler_ids: None,
        platform_urls: None,
        provider: None,
        sent_at: None,
        edited_at: meta.edited_at.clone(),
    }
}

/// Scans a single post folder for `.md` platform files, skipping already-sent platforms.
///
/// Meta is read from `{folder_path}/meta.json` — this works for both the legacy
/// per-repo layout (`{repo}/.postlane/posts/{folder}/`) and the v1.4 workspace
/// layout (`{workspace}/posts/{posts_dir}/{folder}/`).
pub(crate) fn drafts_from_folder(
    repo: &Repo,
    folder_path: &Path,
    post_folder: &str,
    project_id: Option<String>,
) -> Vec<Post> {
    let meta_path = folder_path.join("meta.json");
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

/// Reads `project_id` from `.postlane/config.json` at `config_path`. Returns `None` on any error.
pub(crate) fn project_id_from_config(config_path: &Path) -> Option<String> {
    let v: serde_json::Value = crate::init::read_json_file(config_path).ok()?;
    v["project_id"].as_str().map(str::to_string)
}

/// Scans all post folders directly inside `posts_dir` (no `.postlane/posts` appended).
///
/// Used for v1.4 workspace repos where posts live at `{workspace}/posts/{posts_dir}/`
/// rather than `{repo}/.postlane/posts/`. The `repo.path` field on returned `Post`
/// objects is set to `repo.path` (the child repo path), not the workspace path.
pub(crate) fn drafts_from_posts_dir(
    repo: &Repo,
    posts_dir: &Path,
    project_id: Option<String>,
) -> Vec<Post> {
    if !posts_dir.exists() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(posts_dir) else {
        return vec![];
    };
    entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.path().file_name()?.to_str().map(str::to_string))
        .filter(|f| is_single_component(f))
        .flat_map(|folder| {
            let fp = posts_dir.join(&folder);
            let mut drafts = drafts_from_folder(repo, &fp, &folder, project_id.clone());
            for d in &mut drafts {
                d.repo_path = repo.path.clone();
            }
            drafts
        })
        .collect()
}

/// Scans all post folders inside `{repo_path}/.postlane/posts/` and returns draft posts.
pub(crate) fn drafts_from_repo_path(
    repo: &Repo,
    repo_path: &Path,
    project_id: Option<String>,
) -> Vec<Post> {
    let posts_dir = repo_path.join(".postlane/posts");
    if !posts_dir.exists() {
        return vec![];
    }
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
            let mut drafts = drafts_from_folder(repo, &fp, &folder, project_id.clone());
            let path_str = repo_path.to_str().unwrap_or(&repo.path).to_string();
            for d in &mut drafts {
                d.repo_path = path_str.clone();
            }
            drafts
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::post_meta::{PostMeta, PostStatus};
    use crate::storage::Repo;
    use std::fs;

    fn make_repo(id: &str, path: &str) -> Repo {
        Repo {
            id: id.to_string(),
            name: id.to_string(),
            path: path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // ── is_single_component ──────────────────────────────────────────────────

    #[test]
    fn test_is_single_component_returns_true_for_plain_name() {
        assert!(is_single_component("my-post"));
        assert!(is_single_component("post_2026_04"));
    }

    #[test]
    fn test_is_single_component_returns_false_for_path_traversal() {
        assert!(!is_single_component("../evil"));
    }

    #[test]
    fn test_is_single_component_returns_false_for_multi_segment_path() {
        assert!(!is_single_component("sub/dir"));
    }

    // ── status_str ───────────────────────────────────────────────────────────

    #[test]
    fn test_status_str_returns_ready_when_status_is_none() {
        let meta = PostMeta::default();
        assert_eq!(status_str(&meta), "ready");
    }

    #[test]
    fn test_status_str_returns_ready_when_status_is_ok() {
        let mut meta = PostMeta::default();
        meta.status = Some(PostStatus::Ok);
        assert_eq!(status_str(&meta), "ready");
    }

    #[test]
    fn test_status_str_returns_failed_when_status_is_failed() {
        let mut meta = PostMeta::default();
        meta.status = Some(PostStatus::Failed);
        assert_eq!(status_str(&meta), "failed");
    }

    // ── build_draft ──────────────────────────────────────────────────────────

    #[test]
    fn test_build_draft_sets_platform_and_text() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let meta = PostMeta::default();
        let draft = build_draft(&repo, "my-post", "x", "Hello world".to_string(), &meta, None);
        assert_eq!(draft.post_folder, "my-post");
        assert_eq!(draft.platform, "x");
        assert_eq!(draft.text, "Hello world");
        assert_eq!(draft.status, "ready");
        assert!(draft.project_id.is_none());
    }

    #[test]
    fn test_build_draft_propagates_project_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let meta = PostMeta::default();
        let draft = build_draft(
            &repo,
            "my-post",
            "bluesky",
            String::new(),
            &meta,
            Some("proj-123".to_string()),
        );
        assert_eq!(draft.project_id, Some("proj-123".to_string()));
        assert_eq!(draft.platform, "bluesky");
    }

    #[test]
    fn test_build_draft_sets_failed_status_from_meta() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let mut meta = PostMeta::default();
        meta.status = Some(PostStatus::Failed);
        meta.error = Some("scheduler timeout".to_string());
        let draft = build_draft(&repo, "fail-post", "x", String::new(), &meta, None);
        assert_eq!(draft.status, "failed");
        assert_eq!(draft.error.as_deref(), Some("scheduler timeout"));
    }

    // ── drafts_from_folder ───────────────────────────────────────────────────

    /// When folder_path is a file rather than a directory, read_dir returns Err
    /// and drafts_from_folder returns empty (line 72 in draft_post_scanner.rs).
    #[test]
    fn test_drafts_from_folder_returns_empty_when_folder_is_a_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("not-a-dir");
        fs::write(&file_path, b"I am a file").expect("write file");

        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_folder(&repo, &file_path, "not-a-dir", None);
        assert!(result.is_empty(), "read_dir on a file must return empty vec");
    }

    // ── drafts_from_posts_dir ────────────────────────────────────────────────

    /// Non-existent posts_dir returns empty immediately (line 108 in draft_post_scanner.rs).
    #[test]
    fn test_drafts_from_posts_dir_returns_empty_when_directory_does_not_exist() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let missing = dir.path().join("does-not-exist");

        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_posts_dir(&repo, &missing, None);
        assert!(result.is_empty(), "non-existent posts_dir must return empty vec");
    }

    /// When posts_dir exists as a file rather than a directory, exists() returns true
    /// but read_dir returns Err, so drafts_from_posts_dir returns empty (line 111).
    #[test]
    fn test_drafts_from_posts_dir_returns_empty_when_dir_is_a_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let file_path = dir.path().join("posts-as-file");
        fs::write(&file_path, b"I am a file").expect("write file");

        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_posts_dir(&repo, &file_path, None);
        assert!(result.is_empty(), "posts_dir as file must return empty vec");
    }

    #[test]
    fn test_drafts_from_posts_dir_returns_drafts_from_valid_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join("posts");
        let post_folder = posts_dir.join("my-post");
        fs::create_dir_all(&post_folder).expect("create post folder");
        fs::write(post_folder.join("x.md"), "Hello X").expect("write x.md");

        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_posts_dir(&repo, &posts_dir, Some("proj-1".to_string()));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].platform, "x");
        assert_eq!(result[0].text, "Hello X");
        assert_eq!(result[0].project_id, Some("proj-1".to_string()));
    }

    // ── drafts_from_repo_path ────────────────────────────────────────────────

    /// When .postlane/posts exists as a file, exists() is true but read_dir returns Err
    /// so drafts_from_repo_path returns empty (line 140 in draft_post_scanner.rs).
    #[test]
    fn test_drafts_from_repo_path_returns_empty_when_posts_dir_is_a_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let dot_postlane = dir.path().join(".postlane");
        fs::create_dir_all(&dot_postlane).expect("create .postlane");
        fs::write(dot_postlane.join("posts"), b"file not dir").expect("write posts as file");

        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_repo_path(&repo, dir.path(), None);
        assert!(result.is_empty(), ".postlane/posts as file must return empty vec");
    }

    #[test]
    fn test_drafts_from_repo_path_returns_empty_when_posts_dir_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // No .postlane/posts directory created
        let repo = make_repo("r1", dir.path().to_str().unwrap());
        let result = drafts_from_repo_path(&repo, dir.path(), None);
        assert!(result.is_empty(), "absent posts dir must return empty vec");
    }
}
