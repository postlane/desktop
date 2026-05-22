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
pub(crate) fn drafts_from_folder(
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

/// Reads `project_id` from `.postlane/config.json` at `config_path`. Returns `None` on any error.
pub(crate) fn project_id_from_config(config_path: &Path) -> Option<String> {
    let v: serde_json::Value = crate::init::read_json_file(config_path).ok()?;
    v["project_id"].as_str().map(str::to_string)
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
