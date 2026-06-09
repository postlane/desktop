// SPDX-License-Identifier: BUSL-1.1

//! `save_post_draft` — writes updated platform text and updates PostMeta.

use crate::app_state::AppState;
use crate::init::atomic_write;
use crate::platform_constants::POST_META_LOCKS;
use crate::post_approval::pipeline::post_location::{PostLocation, validate_repo_path};
use crate::post_meta::PostMeta;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

/// Saves updated draft text for one platform and updates `edited_platforms` in PostMeta.
/// Security:
///   - `repo_path` must be in `repos.json`
///   - `post_folder` and `platform` must each be a single path component (rejects `..`, `/`)
///   - `text` must be non-empty and ≤ 15,000 bytes
pub async fn save_post_draft_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
    text: &str,
    state: &AppState,
) -> Result<(), String> {
    // Workspace children first — validate_repo_path canonicalizes and checks
    // workspace repos.json entries. Fall back to a raw-path legacy check so
    // that existing repos.repos[] entries stored without canonicalization still work.
    let location = match validate_repo_path(repo_path, state) {
        Ok(loc @ PostLocation::Workspace { .. }) => loc,
        _ => {
            let is_registered = {
                let repos = state.lock_repos()?;
                repos.repos.iter().any(|r| r.path == repo_path)
            };
            if !is_registered {
                return Err(format!("Path '{}' is not in the registered repos list", repo_path));
            }
            PostLocation::Legacy { canonical: repo_path.to_string() }
        }
    };

    if Path::new(post_folder).components().count() != 1 {
        return Err(format!("post_folder '{}' must be a single path component", post_folder));
    }
    if Path::new(platform).components().count() != 1 {
        return Err(format!("platform '{}' must be a single path component", platform));
    }
    if text.is_empty() {
        return Err("text must not be empty".to_string());
    }
    if text.len() > 15_000 {
        return Err(format!("text exceeds 15,000-byte safety cap (got {} bytes)", text.len()));
    }

    let lock_key = format!("{}\x00{}", location.canonical(), post_folder);
    let lock = POST_META_LOCKS
        .entry(lock_key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    let _guard = lock.lock().await;

    let post_dir = location.posts_base(post_folder);
    std::fs::create_dir_all(&post_dir)
        .map_err(|e| format!("Failed to create post directory: {}", e))?;

    let platform_file = post_dir.join(format!("{}.md", platform));
    atomic_write(&platform_file, text.as_bytes())
        .map_err(|e| format!("Failed to write draft text: {}", e))?;

    let meta_path = post_dir.join("meta.json");
    let mut meta = PostMeta::load(&meta_path)?;
    let ep = meta.edited_platforms.get_or_insert_with(Vec::new);
    if !ep.contains(&platform.to_string()) {
        ep.push(platform.to_string());
    }
    meta.edited_at = Some(chrono::Utc::now().to_rfc3339());
    meta.save(&meta_path)
}

#[tauri::command]
pub async fn save_post_draft(
    repo_path: String,
    post_folder: String,
    platform: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    save_post_draft_impl(&repo_path, &post_folder, &platform, &text, &state).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_state, make_repo, setup_post_dir};
    use std::fs;

    #[tokio::test]
    async fn test_save_post_draft_writes_updated_text() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        setup_post_dir(dir.path(), "commit-abc");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);

        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", "New draft text.", &state)
            .await.expect("should succeed");

        let content = fs::read_to_string(dir.path().join(".postlane/posts/commit-abc/x.md")).expect("read");
        assert_eq!(content, "New draft text.");
    }

    #[tokio::test]
    async fn test_save_post_draft_appends_platform_to_edited_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        setup_post_dir(dir.path(), "commit-abc");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);

        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", "Draft for X.", &state)
            .await.expect("first save");
        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "linkedin", "Draft for LinkedIn.", &state)
            .await.expect("second save");
        // calling twice for the same platform must not duplicate
        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", "Updated X.", &state)
            .await.expect("duplicate save");

        let meta_path = PostMeta::path_for(dir.path(), "commit-abc");
        let meta = PostMeta::load(&meta_path).expect("load meta");
        let ep = meta.edited_platforms.expect("edited_platforms must be Some");
        assert_eq!(ep.iter().filter(|p| *p == "x").count(), 1, "x must appear exactly once");
        assert!(ep.contains(&"linkedin".to_string()), "linkedin must be present");
    }

    #[tokio::test]
    async fn test_save_post_draft_initialises_edited_platforms_when_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        setup_post_dir(dir.path(), "commit-pre-m19");
        // Write a meta.json with no edited_platforms key (pre-M19 post)
        let meta_path = PostMeta::path_for(dir.path(), "commit-pre-m19");
        fs::create_dir_all(meta_path.parent().unwrap()).expect("dirs");
        fs::write(&meta_path, r#"{"sent_platforms":{}}"#).expect("write pre-M19 meta");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-pre-m19", "x", "Draft.", &state)
            .await.expect("should not panic on pre-M19 post");

        let meta = PostMeta::load(&meta_path).expect("load");
        assert_eq!(
            meta.edited_platforms,
            Some(vec!["x".to_string()]),
            "pre-M19 post must get Some([\"x\"]) after first save"
        );
    }

    #[tokio::test]
    async fn test_save_post_draft_writes_edited_at() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        setup_post_dir(dir.path(), "commit-abc");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);

        save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", "Draft.", &state)
            .await.expect("should succeed");

        let meta_path = PostMeta::path_for(dir.path(), "commit-abc");
        let meta = PostMeta::load(&meta_path).expect("load");
        assert!(meta.edited_at.is_some(), "edited_at must be set after save");
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_path_not_in_repos() {
        let state = make_state(vec![make_repo("r1", "/some/other/path")]);
        let result = save_post_draft_impl("/not/registered", "commit-abc", "x", "Draft.", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in the registered repos list"));
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_empty_text() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", "", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"), "error must mention empty");
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_text_exceeding_max_length() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let long_text = "x".repeat(15_001);
        let result = save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "x", &long_text, &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("15,000"), "error must mention the limit");
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_multi_segment_post_folder() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = save_post_draft_impl(dir.path().to_str().unwrap(), "a/b", "x", "Draft.", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single path component"));
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_path_traversal() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = save_post_draft_impl(dir.path().to_str().unwrap(), "../etc", "x", "Draft.", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single path component"));
    }

    /// 22.10.5 — save_post_draft must accept workspace child repo paths and write
    /// to the workspace posts layout, not the legacy per-repo layout.
    #[tokio::test]
    async fn test_save_post_draft_accepts_workspace_child_and_writes_to_workspace_path() {
        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("my-repo");
        std::fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

        // Write workspace repos.json
        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "child-id".to_string(),
                name: "my-repo".to_string(),
                path: canonical_child.to_str().unwrap().to_string(),
                posts_dir: "my-repo".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&canonical_ws.join("repos.json"), &ws_repos).expect("write ws repos");

        // Pre-create the workspace post
        let post_dir = canonical_ws.join("posts").join("my-repo").join("ws-post");
        std::fs::create_dir_all(&post_dir).expect("create post dir");
        std::fs::write(post_dir.join("bluesky.md"), "Original content").expect("write md");
        std::fs::write(post_dir.join("meta.json"), "{}").expect("write meta");

        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: "ws-proj".to_string(),
                name: "my-workspace".to_string(),
                workspace_path: canonical_ws.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!("draft_edits_ws_{}.json", std::process::id()));
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = save_post_draft_impl(
            canonical_child.to_str().unwrap(),
            "ws-post",
            "bluesky",
            "Edited content",
            &state,
        ).await;
        assert!(result.is_ok(), "save_post_draft must accept workspace child repo: {:?}", result);

        // Content must be written to workspace path, not legacy child path
        let ws_file = post_dir.join("bluesky.md");
        let content = std::fs::read_to_string(&ws_file).expect("read workspace file");
        assert_eq!(content, "Edited content", "file must be updated in workspace posts dir");

        let legacy_file = canonical_child.join(".postlane/posts/ws-post/bluesky.md");
        assert!(!legacy_file.exists(), "must NOT write to legacy child repo path");
    }

    #[tokio::test]
    async fn test_save_post_draft_rejects_path_traversal_in_platform() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = save_post_draft_impl(dir.path().to_str().unwrap(), "commit-abc", "../evil", "Draft.", &state).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("single path component"));
    }
}
