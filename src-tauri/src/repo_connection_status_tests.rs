// SPDX-License-Identifier: BUSL-1.1
// Tests for repo_connection_status.rs — extracted to keep the main file under 400 lines.

use super::*;
use crate::github_app::GitHubAppRepo;
use crate::storage::Repo;
use std::path::Path;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn make_app_repo(name: &str, full_name: &str) -> GitHubAppRepo {
    GitHubAppRepo {
        id: 1,
        name: name.to_string(),
        full_name: full_name.to_string(),
        private: false,
        html_url: format!("https://github.com/{}", full_name),
    }
}

fn make_repo(path: &Path, name: &str) -> Repo {
    Repo {
        id: format!("id-{}", name),
        name: name.to_string(),
        path: path.to_str().unwrap().to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn scaffold_git_remote(dir: &Path, url: &str) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(
        dir.join(".git/config"),
        format!("[remote \"origin\"]\n\turl = {}\n", url),
    )
    .unwrap();
}

fn scaffold_cli_config(dir: &Path, project_id: &str) {
    std::fs::create_dir_all(dir.join(".postlane")).unwrap();
    std::fs::write(
        dir.join(".postlane/config.json"),
        serde_json::to_string(&serde_json::json!({ "project_id": project_id })).unwrap(),
    )
    .unwrap();
}

// ── check_cli_state ───────────────────────────────────────────────────────────

// No .postlane dir → not initialised, no mismatch.
#[test]
fn test_check_cli_not_initialized() {
    let tmp = TempDir::new().unwrap();
    let (init, mismatch) = check_cli_state(tmp.path(), "proj-1");
    assert!(!init, "cli_initialized must be false when config.json is absent");
    assert!(!mismatch, "project_id_mismatch must be false when config.json is absent");
}

// config.json present with matching project_id → initialised, no mismatch.
#[test]
fn test_check_cli_initialized_matching_project() {
    let tmp = TempDir::new().unwrap();
    scaffold_cli_config(tmp.path(), "proj-1");
    let (init, mismatch) = check_cli_state(tmp.path(), "proj-1");
    assert!(init, "cli_initialized must be true when config.json exists");
    assert!(!mismatch, "project_id_mismatch must be false when project_id matches");
}

// config.json present with a different project_id → initialised and mismatched.
#[test]
fn test_check_cli_initialized_mismatched_project() {
    let tmp = TempDir::new().unwrap();
    scaffold_cli_config(tmp.path(), "other-proj");
    let (init, mismatch) = check_cli_state(tmp.path(), "proj-1");
    assert!(init, "cli_initialized must be true when config.json exists");
    assert!(mismatch, "project_id_mismatch must be true when project_id differs");
}

// config.json exists but is not valid JSON → treat as initialised, no mismatch assertion.
#[test]
fn test_check_cli_malformed_config_treated_as_initialized() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join(".postlane")).unwrap();
    std::fs::write(tmp.path().join(".postlane/config.json"), b"not json").unwrap();
    let (init, mismatch) = check_cli_state(tmp.path(), "proj-1");
    assert!(init, "cli_initialized must be true when file exists even if malformed");
    assert!(!mismatch, "project_id_mismatch must not fire when json cannot be parsed");
}

// config.json path exists as a directory (so `read_to_string` fails) →
// treated as initialised with no project_id mismatch (covers the Err branch in check_cli_state).
#[test]
fn test_check_cli_config_json_is_dir_treated_as_initialized() {
    let tmp = TempDir::new().unwrap();
    // Create `.postlane/config.json` as a *directory*, not a file.
    // `Path::exists()` returns true, but `read_to_string` returns an error.
    std::fs::create_dir_all(tmp.path().join(".postlane").join("config.json"))
        .expect("create config.json as directory");
    let (init, mismatch) = check_cli_state(tmp.path(), "proj-1");
    assert!(
        init,
        "cli_initialized must be true when config.json path exists (even as a directory)"
    );
    assert!(
        !mismatch,
        "project_id_mismatch must be false when the file cannot be read"
    );
}

// ── merge_into_rows ───────────────────────────────────────────────────────────

// Repo only in GitHub App (no local clone, not registered) → one row, correct flags.
#[test]
fn test_github_app_only_repo() {
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[], "proj-1");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].github_full_name.as_deref(), Some("org/my-repo"));
    assert_eq!(rows[0].display_name, "my-repo");
    assert!(rows[0].github_app_connected);
    assert!(!rows[0].folder_registered);
    assert!(!rows[0].cli_initialized);
    assert!(rows[0].local_path.is_none());
}

// Repo registered locally only (no GitHub App entry) → one row, correct flags.
#[test]
fn test_registered_only_repo() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    let repo = make_repo(tmp.path(), "my-repo");
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].github_app_connected);
    assert!(rows[0].folder_registered);
    assert_eq!(rows[0].local_path.as_deref(), Some(tmp.path().to_str().unwrap()));
    assert_eq!(rows[0].github_full_name.as_deref(), Some("org/my-repo"));
}

// Same repo in both sources (slug match) → exactly one row with both flags true.
#[test]
fn test_repo_in_both_sources_produces_single_row() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    let repo = make_repo(tmp.path(), "my-repo");
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[repo], "proj-1");
    assert_eq!(rows.len(), 1, "same repo in both sources must not produce a duplicate row");
    assert!(rows[0].github_app_connected);
    assert!(rows[0].folder_registered);
}

// HTTPS remote URL matches the same slug as the GitHub App full_name.
#[test]
fn test_https_remote_matches_app_repo() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "https://github.com/org/my-repo.git");
    let repo = make_repo(tmp.path(), "my-repo");
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[repo], "proj-1");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].github_app_connected && rows[0].folder_registered);
}

// CLI config present with matching project_id → cli_initialized=true, mismatch=false.
#[test]
fn test_cli_initialized_when_config_exists_and_matches() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    scaffold_cli_config(tmp.path(), "proj-1");
    let repo = make_repo(tmp.path(), "my-repo");
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[repo], "proj-1");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].cli_initialized);
    assert!(!rows[0].project_id_mismatch);
}

// CLI config present with different project_id → the local clone is excluded from
// the slug_map so the GitHub App row shows folder_registered=false. The repo is
// registered for a different project and must not pollute this project's table.
#[test]
fn test_project_id_mismatch_when_config_has_different_id() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    scaffold_cli_config(tmp.path(), "other-proj");
    let repo = make_repo(tmp.path(), "my-repo");
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[repo], "proj-1");
    assert_eq!(rows.len(), 1, "GitHub App row must still appear");
    assert!(rows[0].github_app_connected, "github_app_connected must be true");
    assert!(!rows[0].folder_registered, "folder must not show as registered for this project");
    assert!(!rows[0].cli_initialized, "cli must not show as initialized for this project");
}

// Sort: GitHub App rows first (alphabetical), then local-only rows (alphabetical).
#[test]
fn test_sort_order_github_app_first_then_alphabetical() {
    let tmp = TempDir::new().unwrap();

    let zzz_dir = tmp.path().join("zzz-local");
    std::fs::create_dir_all(&zzz_dir).unwrap();
    scaffold_git_remote(&zzz_dir, "git@github.com:org/zzz-local.git");
    let local_repo = make_repo(&zzz_dir, "zzz-local");

    // Two GitHub App repos — neither has a local clone
    let app_repos = vec![
        make_app_repo("bbb-remote", "org/bbb-remote"),
        make_app_repo("aaa-remote", "org/aaa-remote"),
    ];

    let rows = merge_into_rows(&app_repos, &[local_repo], "proj-1");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].display_name, "aaa-remote", "GitHub App rows must be sorted alphabetically first");
    assert_eq!(rows[1].display_name, "bbb-remote");
    assert_eq!(rows[2].display_name, "zzz-local", "local-only rows must follow GitHub App rows");
}

// Inactive repos must not appear in results.
#[test]
fn test_inactive_repo_excluded_from_results() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    let mut repo = make_repo(tmp.path(), "my-repo");
    repo.active = false;
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert!(rows.is_empty(), "inactive repos must not appear in the table");
}

// When GitHub App API would fail (empty list passed), local repos still appear.
#[test]
fn test_local_repos_appear_when_github_app_returns_empty() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    let repo = make_repo(tmp.path(), "my-repo");
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].github_app_connected);
    assert!(rows[0].folder_registered);
}

// A local-only repo whose config.json has a different project_id must not appear.
#[test]
fn test_local_only_repo_with_wrong_project_excluded() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    scaffold_cli_config(tmp.path(), "other-proj"); // belongs to a different project
    let repo = make_repo(tmp.path(), "my-repo");
    // no GitHub App repo — this would only appear via the local-only path
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert!(rows.is_empty(), "local-only repo with wrong project_id must not appear in this org's table");
}

// A local-only repo whose path no longer exists on disk must be silently dropped.
#[test]
fn test_stale_registration_with_missing_path_excluded() {
    let repo = Repo {
        id: "stale-id".to_string(),
        name: "gone-repo".to_string(),
        path: "/nonexistent/path/gone-repo".to_string(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    };
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert!(rows.is_empty(), "stale registration with missing path must be silently dropped");
}

// A repo in the GitHub App that also has a local clone registered for a different
// project should still show — but with folder_registered=false and project_id_mismatch=false.
// The user needs to re-register the folder for this project.
#[test]
fn test_github_app_repo_with_mismatched_local_shows_as_not_folder_registered() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@github.com:org/my-repo.git");
    scaffold_cli_config(tmp.path(), "other-proj"); // registered for a different project
    let repo = make_repo(tmp.path(), "my-repo");
    let app_repos = vec![make_app_repo("my-repo", "org/my-repo")];
    let rows = merge_into_rows(&app_repos, &[repo], "proj-1");
    assert_eq!(rows.len(), 1, "GitHub App row must still appear");
    assert!(rows[0].github_app_connected, "github_app_connected must be true");
    assert!(!rows[0].folder_registered, "folder must not appear registered for this project");
}

// GitLab remote — registered locally but github_full_name must be None.
#[test]
fn test_non_github_remote_has_no_full_name() {
    let tmp = TempDir::new().unwrap();
    scaffold_git_remote(tmp.path(), "git@gitlab.com:org/my-repo.git");
    let repo = make_repo(tmp.path(), "my-repo");
    let rows = merge_into_rows(&[], &[repo], "proj-1");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].github_full_name.is_none(), "non-GitHub remote must not produce a github_full_name");
    assert!(rows[0].folder_registered);
}

// Two projects, two repos — each registered repo appears in its own row with no cross-contamination.
#[test]
fn test_multiple_repos_all_present() {
    let tmp_a = TempDir::new().unwrap();
    let tmp_b = TempDir::new().unwrap();
    scaffold_git_remote(tmp_a.path(), "git@github.com:org/repo-a.git");
    scaffold_git_remote(tmp_b.path(), "git@github.com:org/repo-b.git");
    let repo_a = make_repo(tmp_a.path(), "repo-a");
    let repo_b = make_repo(tmp_b.path(), "repo-b");
    let app_repos = vec![
        make_app_repo("repo-a", "org/repo-a"),
        make_app_repo("repo-b", "org/repo-b"),
    ];
    let rows = merge_into_rows(&app_repos, &[repo_a, repo_b], "proj-1");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.github_app_connected && r.folder_registered));
}
