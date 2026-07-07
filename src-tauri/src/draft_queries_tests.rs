// SPDX-License-Identifier: BUSL-1.1
// Tests for draft_queries.rs — extracted to keep the main file under 400 lines.

use super::*;
use crate::test_fixtures::{make_state, make_repo, write_config, write_meta, write_workspace_config};
use std::fs;
use std::path::{Path, PathBuf};

    fn write_md(dir: &Path, folder: &str, platform: &str, text: &str) {
        fs::create_dir_all(dir.join(".git")).expect("create .git");
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join(format!("{}.md", platform)), text).expect("write md");
    }

    #[test]
    fn test_get_all_drafts_includes_image_attribution() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(
            dir.path(),
            "my-post",
            r#"{"image_url":"https://images.unsplash.com/photo-abc","image_attribution":{"photographer_name":"Jane Doe","photographer_url":"https://unsplash.com/@janedoe"}}"#,
        );
        write_md(dir.path(), "my-post", "x", "Hello");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        let attr = result[0].image_attribution.as_ref().expect("image_attribution must be Some");
        assert_eq!(attr.photographer_name, "Jane Doe");
        assert_eq!(attr.photographer_url, "https://unsplash.com/@janedoe");
    }

    #[test]
    fn test_get_all_drafts_empty() {
        let state = make_state(vec![]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_drafts_inactive_repo_excluded() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_md(dir.path(), "my-post", "x", "Inactive");
        let mut repo = make_repo("r1", dir.path().to_str().unwrap());
        repo.active = false;
        let state = make_state(vec![repo]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_drafts_includes_project_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_config(dir.path(), r#"{"project_id":"proj-abc"}"#);
        write_md(dir.path(), "my-post", "x", "Hello");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project_id, Some("proj-abc".to_string()));
    }

    #[test]
    fn test_get_all_drafts_includes_scheduled_for() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(dir.path(), "my-post", r#"{"scheduled_for":"2026-06-01T10:00:00Z"}"#);
        write_md(dir.path(), "my-post", "x", "Hello");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].scheduled_for.as_deref(), Some("2026-06-01T10:00:00Z"));
    }

    #[test]
    fn test_get_all_drafts_excludes_sent_platforms() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(
            dir.path(),
            "my-post",
            r#"{"sent_platforms":{"x":"2026-05-01T10:00:00Z"}}"#,
        );
        write_md(dir.path(), "my-post", "x", "Already sent");
        write_md(dir.path(), "my-post", "bluesky", "Not sent yet");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].platform, "bluesky");
    }

    #[test]
    fn test_draft_event_disappears_from_queue_when_all_platforms_sent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(
            dir.path(),
            "my-post",
            r#"{"sent_platforms":{"x":"2026-05-01T10:00:00Z","bluesky":"2026-05-01T10:00:00Z"}}"#,
        );
        write_md(dir.path(), "my-post", "x", "X post");
        write_md(dir.path(), "my-post", "bluesky", "Bluesky post");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "all sent → zero draft rows");
    }

    #[test]
    fn test_get_all_drafts_returns_failed_status() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(
            dir.path(),
            "my-post",
            r#"{"status":"failed","error":"scheduler timeout"}"#,
        );
        write_md(dir.path(), "my-post", "x", "Failed post");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[0].error.as_deref(), Some("scheduler timeout"));
    }

    #[test]
    fn test_get_all_drafts_treats_absent_post_meta_as_clean() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_md(dir.path(), "my-post", "x", "No meta");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "ready");
        assert!(result[0].error.is_none());
        assert!(result[0].scheduled_for.is_none());
    }

    // ── Workspace tests (20.8) ────────────────────────────────────────────────

    fn make_workspace(_name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        let child_a = ws.path().join("repo-a");
        let child_b = ws.path().join("repo-b");
        fs::create_dir_all(child_a.join(".git")).expect("git a");
        fs::create_dir_all(child_b.join(".git")).expect("git b");
        (ws, child_a, child_b)
    }

    #[test]
    fn test_workspace_drafts_set_repo_path_to_child_path() {
        let (ws, child_a, _) = make_workspace("repo_path_13");
        write_md(&child_a, "my-post", "x", "Hello");
        let state = make_state(vec![make_repo("ws", ws.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "workspace must surface child drafts");
        assert_eq!(
            result[0].repo_path,
            child_a.to_str().unwrap(),
            "repo_path must be child path, not workspace root"
        );
    }

    #[test]
    fn test_workspace_child_with_own_config_uses_child_project_id() {
        let (ws, child_a, _) = make_workspace("own_cfg_11");
        write_workspace_config(ws.path(), r#"{"project_id":"parent-proj"}"#);
        write_config(&child_a, r#"{"project_id":"child-proj"}"#);
        write_md(&child_a, "my-post", "x", "Child post");
        let state = make_state(vec![make_repo("ws", ws.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project_id, Some("child-proj".to_string()));
    }

    #[test]
    fn test_workspace_child_without_config_inherits_parent_project_id() {
        let (ws, child_a, _) = make_workspace("inherit_cfg_11");
        write_workspace_config(ws.path(), r#"{"project_id":"parent-proj"}"#);
        write_md(&child_a, "my-post", "x", "Child post");
        let state = make_state(vec![make_repo("ws", ws.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project_id, Some("parent-proj".to_string()));
    }

    #[test]
    fn test_workspace_registered_child_not_double_counted() {
        let (ws, child_a, _) = make_workspace("dedup_16");
        write_md(&child_a, "my-post", "x", "Post");
        let state = make_state(vec![
            make_repo("ws", ws.path().to_str().unwrap()),
            make_repo("child-r", child_a.to_str().unwrap()),
        ]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "workspace scan must skip registered child");
    }

    #[test]
    fn test_workspace_get_all_drafts_from_all_children_deterministic() {
        let (ws, child_a, child_b) = make_workspace("all_children_17");
        write_md(&child_a, "post-1", "x", "A");
        write_md(&child_b, "post-1", "x", "B");
        let state = make_state(vec![make_repo("ws", ws.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2, "both children must be scanned");
        let pa = child_a.to_str().unwrap();
        let pb = child_b.to_str().unwrap();
        assert_eq!(result[0].repo_path, pa.min(pb));
        assert_eq!(result[1].repo_path, pa.max(pb));
    }

    // 20.10.13 — markdown_file outputs in draft_output_dir do not appear in get_all_drafts
    #[test]
    fn test_markdown_file_output_in_draft_output_dir_not_in_queue() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        // Write a file directly to .postlane/drafts/ (not .postlane/posts/)
        let drafts_dir = dir.path().join(".postlane").join("drafts");
        fs::create_dir_all(&drafts_dir).expect("create drafts dir");
        fs::write(drafts_dir.join("newsletter.md"), "# Update\n\nSome content.").expect("write");
        // Also write a normal social post to confirm queue still works for those
        write_md(dir.path(), "social-post", "x", "X post");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        // Only the social post should appear; the markdown_file output must not
        assert_eq!(result.len(), 1, "markdown_file output must not appear in queue");
        assert_eq!(result[0].platform, "x", "queue must still contain social posts");
    }

    #[test]
    fn test_get_all_drafts_sorted_by_repo_post_folder_platform() {
        let dir_a = tempfile::TempDir::new().expect("create temp dir");
        let dir_b = tempfile::TempDir::new().expect("create temp dir");
        write_md(dir_a.path(), "folder-1", "x", "A x");
        write_md(dir_a.path(), "folder-1", "bluesky", "A bluesky");
        write_md(dir_b.path(), "folder-1", "x", "B x");
        write_md(dir_b.path(), "folder-1", "bluesky", "B bluesky");

        let path_a = dir_a.path().to_str().unwrap().to_string();
        let path_b = dir_b.path().to_str().unwrap().to_string();
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
    }

    #[test]
    fn test_get_all_drafts_capped_at_max_page() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        for i in 0..MAX_DRAFT_PAGE + 5 {
            write_md(dir.path(), &format!("folder-{:03}", i), "x", "content");
        }
        let path = dir.path().to_str().unwrap().to_string();
        let state = make_state(vec![make_repo("r1", &path)]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), MAX_DRAFT_PAGE, "result must be capped at MAX_DRAFT_PAGE");
    }

    #[test]
    fn test_get_all_drafts_repo_with_no_posts_dir_returns_empty() {
        // posts_dir doesn't exist → drafts_from_repo_path returns vec![] early
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        // No .postlane/posts directory
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "repo without posts dir must produce no drafts");
    }

    #[test]
    fn test_get_all_drafts_post_folder_with_no_md_files() {
        // Folder exists but contains no .md files → no drafts
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        let post_dir = dir.path().join(".postlane/posts/empty-folder");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(post_dir.join("notes.txt"), "notes").expect("write non-md file");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert!(result.is_empty(), "folder with only non-.md files must produce no drafts");
    }

    #[test]
    fn test_get_all_drafts_failed_status_is_surfaced() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_meta(
            dir.path(),
            "fail-post",
            r#"{"status":"failed","error":"oops"}"#,
        );
        write_md(dir.path(), "fail-post", "x", "Failed content");

        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[0].error.as_deref(), Some("oops"));
    }


    // ── 22.2 workspace draft path tests ─────────────────────────────────────

    /// Helper: write a draft to a workspace posts subdirectory.
    fn write_workspace_draft(workspace: &Path, posts_dir: &str, folder: &str, platform: &str, text: &str) {
        let p = workspace.join("posts").join(posts_dir).join(folder);
        fs::create_dir_all(&p).expect("create workspace post dir");
        fs::write(p.join(format!("{}.md", platform)), text).expect("write md");
    }

    /// Helper: write {workspace}/repos.json with a single RepoEntry.
    fn write_workspace_repos_json(workspace: &Path, id: &str, name: &str, path: &str, posts_dir: &str) {
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};
        let config = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: id.to_string(),
                name: name.to_string(),
                path: path.to_string(),
                posts_dir: posts_dir.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&workspace.join("repos.json"), &config).expect("write workspace repos.json");
    }

    /// 22.2.10 — draft in workspace posts subdirectory appears in queue with correct posts_dir.
    #[test]
    fn test_workspace_draft_read_from_workspace_posts_subdir() {
        let ws = tempfile::TempDir::new().expect("create ws");
        let child = ws.path().join("frontend");
        fs::create_dir_all(child.join(".git")).expect("git");

        write_workspace_repos_json(ws.path(), "r1", "frontend", child.to_str().unwrap(), "frontend");
        write_workspace_draft(ws.path(), "frontend", "my-post", "x", "Hello workspace");

        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-1".to_string(),
                name: "myorg".to_string(),
                workspace_path: ws.path().to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!("repos_ws_scan_{}.json", std::process::id()));
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "workspace draft must appear in queue");
        assert_eq!(result[0].platform, "x");
        assert_eq!(result[0].text, "Hello workspace");
    }

    /// 22.2.11 — legacy per-repo draft still appears in queue (backward compat).
    #[test]
    fn test_legacy_per_repo_draft_still_appears_in_queue() {
        let dir = tempfile::TempDir::new().expect("create dir");
        write_md(dir.path(), "legacy-post", "bluesky", "Legacy content");
        let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 1, "legacy per-repo draft must still appear");
        assert_eq!(result[0].platform, "bluesky");
        assert_eq!(result[0].text, "Legacy content");
    }

    /// 22.2.15 — two child repos produce drafts in separate subdirs; group labels are human-readable names.
    #[test]
    fn test_two_workspace_children_produce_separate_subdir_drafts() {
        let ws = tempfile::TempDir::new().expect("create ws");
        let child_a = ws.path().join("frontend");
        let child_b = ws.path().join("backend");
        fs::create_dir_all(child_a.join(".git")).expect("git a");
        fs::create_dir_all(child_b.join(".git")).expect("git b");

        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};
        let config = WorkspaceReposConfig {
            version: 1,
            repos: vec![
                RepoEntry {
                    id: "r1".to_string(), name: "frontend".to_string(),
                    path: child_a.to_str().unwrap().to_string(),
                    posts_dir: "frontend".to_string(),
                    active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
                },
                RepoEntry {
                    id: "r2".to_string(), name: "backend".to_string(),
                    path: child_b.to_str().unwrap().to_string(),
                    posts_dir: "backend".to_string(),
                    active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        write_workspace_repos(&ws.path().join("repos.json"), &config).expect("write");
        write_workspace_draft(ws.path(), "frontend", "post-1", "x", "Frontend post");
        write_workspace_draft(ws.path(), "backend", "post-2", "bluesky", "Backend post");

        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        let repos_config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-1".to_string(), name: "myorg".to_string(),
                workspace_path: ws.path().to_str().unwrap().to_string(),
                active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!("repos_ws_two_children_{}.json", std::process::id()));
        let state = crate::app_state::AppState::new_with_path(repos_config, repos_path);

        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2, "both workspace child drafts must appear");

        let repo_names: std::collections::HashSet<&str> = result.iter().map(|p| p.repo_name.as_str()).collect();
        assert!(repo_names.contains("frontend"), "frontend child name must appear");
        assert!(repo_names.contains("backend"), "backend child name must appear");

        // No cross-contamination — each draft in the correct subdirectory
        let frontend_post = result.iter().find(|p| p.repo_name == "frontend").expect("frontend post");
        assert_eq!(frontend_post.platform, "x");
        let backend_post = result.iter().find(|p| p.repo_name == "backend").expect("backend post");
        assert_eq!(backend_post.platform, "bluesky");
    }

    /// 22.10.7 — legacy per-repo entry in `repos[]` still shows posts alongside a workspace entry.
    /// Backward compat: upgrading to v1.4 must not break existing repos that were never migrated.
    #[test]
    fn test_legacy_repo_queue_coexists_with_workspace() {
        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;

        // Workspace with one post
        let ws = tempfile::TempDir::new().expect("create ws");
        let ws_child = ws.path().join("ws-child");
        fs::create_dir_all(ws_child.join(".git")).expect("git");
        write_workspace_repos_json(ws.path(), "ws-r1", "ws-child", ws_child.to_str().unwrap(), "ws-child");
        write_workspace_draft(ws.path(), "ws-child", "ws-post", "bluesky", "Workspace content");

        // Legacy repo with a post at {repo}/.postlane/posts/
        let legacy = tempfile::TempDir::new().expect("create legacy");
        write_md(legacy.path(), "legacy-post", "x", "Legacy content");

        let repos_path = std::env::temp_dir().join(
            format!("repos_coexist_{}.json", std::process::id()),
        );
        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-1".to_string(),
                name: "myorg".to_string(),
                workspace_path: ws.path().to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![make_repo("legacy-r1", legacy.path().to_str().unwrap())],
        };
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2, "both workspace and legacy posts must appear");
        assert!(result.iter().any(|p| p.text == "Workspace content"), "workspace post missing");
        assert!(result.iter().any(|p| p.text == "Legacy content"), "legacy post missing");
    }

    /// 22.10.10 — two workspace child repos with the same folder name (e.g. both named
    /// "frontend" from different parent orgs) get distinct posts_dir values ("frontend"
    /// and "frontend-2"). Drafts from each appear in the queue under the correct subdir
    /// with the correct repo name — no cross-contamination.
    #[test]
    fn test_collision_repos_both_appear_in_queue_with_distinct_posts_dir() {
        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

        let ws = tempfile::TempDir::new().expect("create ws");

        // Two child repos whose folder name is both "frontend" (different parent paths).
        let child_a = ws.path().join("org-a").join("frontend");
        let child_b = ws.path().join("org-b").join("frontend");
        fs::create_dir_all(child_a.join(".git")).expect("git a");
        fs::create_dir_all(child_b.join(".git")).expect("git b");

        // Simulate assign_posts_dir result: "frontend" and "frontend-2".
        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![
                RepoEntry {
                    id: "r-a".to_string(), name: "org-a/frontend".to_string(),
                    path: child_a.to_str().unwrap().to_string(),
                    posts_dir: "frontend".to_string(),
                    active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
                },
                RepoEntry {
                    id: "r-b".to_string(), name: "org-b/frontend".to_string(),
                    path: child_b.to_str().unwrap().to_string(),
                    posts_dir: "frontend-2".to_string(),
                    active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
                },
            ],
        };
        write_workspace_repos(&ws.path().join("repos.json"), &ws_repos).expect("write repos.json");

        write_workspace_draft(ws.path(), "frontend",   "post-a", "bluesky", "Post from org-a");
        write_workspace_draft(ws.path(), "frontend-2", "post-b", "bluesky", "Post from org-b");

        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-1".to_string(), name: "myorg".to_string(),
                workspace_path: ws.path().to_str().unwrap().to_string(),
                active: true, added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir()
            .join(format!("repos_collision_{}.json", std::process::id()));
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = get_all_drafts_impl(&state).expect("ok");
        assert_eq!(result.len(), 2, "both collision repos must appear in queue");

        let post_a = result.iter().find(|p| p.text == "Post from org-a")
            .expect("org-a post missing");
        let post_b = result.iter().find(|p| p.text == "Post from org-b")
            .expect("org-b post missing");

        assert_eq!(post_a.repo_name, "org-a/frontend", "repo_name must identify org-a");
        assert_eq!(post_b.repo_name, "org-b/frontend", "repo_name must identify org-b");

        // Posts must be in separate subdirectories — no cross-contamination.
        // repo_path is the child repo path; post_folder is the post name.
        // The actual file lives at {workspace}/posts/{posts_dir}/{post_folder}/.
        assert_eq!(post_a.post_folder, "post-a", "org-a post_folder must be post-a");
        assert_eq!(post_b.post_folder, "post-b", "org-b post_folder must be post-b");
        assert_ne!(post_a.repo_path, post_b.repo_path, "child repo paths must differ");
    }

    // ── drafts_from_workspace_entry error branches ───────────────────────────

    /// When repos.json is absent, read_workspace_repos returns Err → drafts_from_workspace_entry
    /// logs a warning and returns an empty vec (lines 24-26 in draft_queries.rs).
    #[test]
    fn test_drafts_from_workspace_entry_returns_empty_when_repos_json_missing() {
        let ws = tempfile::TempDir::new().expect("create ws");
        // No repos.json written → read_workspace_repos will fail
        let ws_entry = crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
            id: "ws-1".to_string(),
            name: "ws".to_string(),
            workspace_path: ws.path().to_str().unwrap().to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = drafts_from_workspace_entry(&ws_entry);
        assert!(result.is_empty(), "missing repos.json must yield empty draft list");
    }

    /// When repos.json exists but contains invalid JSON, read_workspace_repos returns Err →
    /// drafts_from_workspace_entry returns empty (lines 24-26 in draft_queries.rs).
    #[test]
    fn test_drafts_from_workspace_entry_returns_empty_when_repos_json_malformed() {
        let ws = tempfile::TempDir::new().expect("create ws");
        fs::write(ws.path().join("repos.json"), b"not valid json at all")
            .expect("write malformed repos.json");
        let ws_entry = crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
            id: "ws-1".to_string(),
            name: "ws".to_string(),
            workspace_path: ws.path().to_str().unwrap().to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = drafts_from_workspace_entry(&ws_entry);
        assert!(result.is_empty(), "malformed repos.json must yield empty draft list");
    }

    /// When repos.json is valid but the posts subdir for a repo entry does not exist,
    /// drafts_from_workspace_entry returns empty for that entry (line 37 in draft_queries.rs).
    #[test]
    fn test_drafts_from_workspace_entry_returns_empty_when_posts_subdir_missing() {
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};
        let ws = tempfile::TempDir::new().expect("create ws");
        let child = ws.path().join("repo-a");
        // repos.json lists the repo entry, but posts/repo-a/ is never created
        let config = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "r1".to_string(),
                name: "repo-a".to_string(),
                path: child.to_str().unwrap().to_string(),
                posts_dir: "repo-a".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&ws.path().join("repos.json"), &config)
            .expect("write repos.json");
        // posts/repo-a/ intentionally not created → posts_subdir.exists() == false

        let ws_entry = crate::workspace_entry::WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
            id: "ws-1".to_string(),
            name: "ws".to_string(),
            workspace_path: ws.path().to_str().unwrap().to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let result = drafts_from_workspace_entry(&ws_entry);
        assert!(result.is_empty(), "missing posts subdir must yield empty draft list");
    }
