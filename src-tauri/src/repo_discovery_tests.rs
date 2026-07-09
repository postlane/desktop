// SPDX-License-Identifier: BUSL-1.1
// Tests for repo_discovery.rs — extracted to keep repo_discovery.rs under 400 lines.

use super::*;
use crate::github_app::GitHubAppRepo;
use crate::project_registry::ProjectSummary;
use httpmock::prelude::*;
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

fn make_project(id: &str, name: &str) -> ProjectSummary {
    ProjectSummary {
        id: id.to_string(),
        name: name.to_string(),
        workspace_type: "github".to_string(),
        tier: "free".to_string(),
        billing_active: true,
        is_owner: true,
        status: "free_owned".to_string(),
        provider_org_login: None,
    }
}

fn scaffold_git_repo_with_remote(dir: &std::path::Path, remote_url: &str) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    let cfg = format!(
        "[core]\n\trepofmt = 0\n[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/*\n",
        remote_url
    );
    std::fs::write(dir.join(".git/config"), cfg).unwrap();
}

fn app_repos_body(repos: &[(&str, &str)]) -> serde_json::Value {
    let items: Vec<serde_json::Value> = repos
        .iter()
        .enumerate()
        .map(|(i, (name, full_name))| {
            serde_json::json!({
                "id": i + 1,
                "name": name,
                "full_name": full_name,
                "private": false,
                "html_url": format!("https://github.com/{}", full_name),
            })
        })
        .collect();
    serde_json::json!({ "repos": items })
}

// ── discover_repos_impl ───────────────────────────────────────────────────────

#[test]
fn test_ssh_remote_matched_and_added() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert_eq!(result.added, vec!["my-repo"]);
    assert!(result.not_found_on_disk.is_empty());
    let written: crate::storage::ReposConfig =
        serde_json::from_str(&std::fs::read_to_string(&repos_path).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 1);
}

#[test]
fn test_https_remote_matched_and_added() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "https://github.com/my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert_eq!(result.added, vec!["my-repo"]);
}

#[test]
fn test_already_registered_not_duplicated() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let repos_path = tmp.path().join("repos.json");
    let canonical = std::fs::canonicalize(&repo_dir).unwrap();
    let existing = serde_json::json!({
        "version": 1,
        "repos": [{"id":"u1","name":"my-repo","path": canonical.to_str().unwrap(),"active":true,"added_at":"2024-01-01T00:00:00Z"}]
    });
    std::fs::write(&repos_path, serde_json::to_string(&existing).unwrap()).unwrap();
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert!(result.added.is_empty());
    assert_eq!(result.already_registered, vec!["my-repo"]);
}

#[test]
fn test_app_repo_not_on_disk_in_not_found() {
    let tmp = TempDir::new().unwrap();
    let app_repos = vec![make_app_repo("missing-repo", "my-org/missing-repo")];
    let repos_path = tmp.path().join("repos.json");
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert!(result.added.is_empty());
    assert_eq!(result.not_found_on_disk, vec!["my-org/missing-repo"]);
}

#[test]
fn test_non_github_remote_not_matched() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@gitlab.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert!(result.added.is_empty());
    assert_eq!(result.not_found_on_disk, vec!["my-org/my-repo"]);
}

#[test]
fn test_repos_json_write_is_atomic() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    let raw = std::fs::read_to_string(&repos_path).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
    assert!(std::fs::read_dir(tmp.path())
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
}

#[test]
fn test_failed_write_in_failed_to_register() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    // repos_path is a directory, not a file — write will fail
    let repos_path = tmp.path().join("repos-dir");
    std::fs::create_dir_all(&repos_path).unwrap();
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-1");
    assert_eq!(result.failed_to_register.len(), 1, "one failed registration");
    assert!(result.added.is_empty());
}

#[test]
fn test_discovered_repo_config_json_has_correct_schema() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    let result = discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-xyz");
    assert_eq!(result.added, vec!["my-repo"]);
    let config_str = std::fs::read_to_string(repo_dir.join(".postlane/config.json"))
        .expect("config.json must exist after discovery");
    let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    assert_eq!(config["version"].as_u64(), Some(1));
    assert_eq!(config["project_id"].as_str(), Some("proj-xyz"));
    assert_eq!(config["base_url"].as_str(), Some("https://postlane.dev"));
    assert!(config["llm"].is_object());
    assert_eq!(config["llm"]["provider"].as_str(), Some("anthropic"));
}

#[test]
fn test_discovered_repo_writes_config_local_json() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let repos_path = tmp.path().join("repos.json");
    discover_repos_impl(&app_repos, &[tmp.path().to_path_buf()], &repos_path, "proj-xyz");
    assert!(repo_dir.join(".postlane/config.local.json").exists());
    let local_str = std::fs::read_to_string(repo_dir.join(".postlane/config.local.json")).unwrap();
    let local: serde_json::Value = serde_json::from_str(&local_str).unwrap();
    assert!(local["scheduler"].is_object());
}

// ── Bug 21.13.6a — duplicate project via Add-folder + GitHub App ──────────────

fn write_canonical_repos_json(repos_path: &std::path::Path, repo_dir: &std::path::Path) {
    let canonical = std::fs::canonicalize(repo_dir).unwrap();
    let json = serde_json::json!({
        "version": 1,
        "repos": [{"id":"u1","name":"my-repo","path": canonical.to_str().unwrap(),
                    "active":true,"added_at":"2026-01-01T00:00:00Z"}]
    });
    std::fs::write(repos_path, serde_json::to_string(&json).unwrap()).unwrap();
}

#[test]
#[cfg(unix)]
fn test_discover_does_not_duplicate_when_scan_path_differs_from_canonical() {
    let tmp = TempDir::new().unwrap();
    let real_parent = tmp.path().join("real");
    let repo_dir = real_parent.join("my-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let link_parent = tmp.path().join("via-link");
    std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();
    let repos_path = tmp.path().join("repos.json");
    write_canonical_repos_json(&repos_path, &repo_dir);
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let result = discover_repos_impl(&app_repos, &[link_parent], &repos_path, "proj-b");
    assert!(result.added.is_empty(), "must not add duplicate: {:?}", result.added);
    assert_eq!(result.already_registered, vec!["my-repo"]);
    let written: crate::storage::ReposConfig =
        serde_json::from_str(&std::fs::read_to_string(&repos_path).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 1);
}

#[test]
#[cfg(unix)]
fn test_discover_does_not_overwrite_project_config_for_registered_repo() {
    let tmp = TempDir::new().unwrap();
    let real_parent = tmp.path().join("real");
    let repo_dir = real_parent.join("my-repo");
    std::fs::create_dir_all(&repo_dir).unwrap();
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let link_parent = tmp.path().join("via-link");
    std::os::unix::fs::symlink(&real_parent, &link_parent).unwrap();
    let repos_path = tmp.path().join("repos.json");
    write_canonical_repos_json(&repos_path, &repo_dir);
    let postlane_dir = repo_dir.join(".postlane");
    std::fs::create_dir_all(&postlane_dir).unwrap();
    std::fs::write(postlane_dir.join("config.json"), r#"{"project_id":"proj-a"}"#).unwrap();
    let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
    let _ = discover_repos_impl(&app_repos, &[link_parent], &repos_path, "proj-b");
    let config_str = std::fs::read_to_string(repo_dir.join(".postlane/config.json")).unwrap();
    let config: serde_json::Value = serde_json::from_str(&config_str).unwrap();
    assert_eq!(config["project_id"].as_str(), Some("proj-a"));
}

// ── run_discovery_for_all_projects ────────────────────────────────────────────

// Discovery succeeds for a single project with one locally cloned repo.
#[tokio::test]
async fn test_discovers_single_repo_for_single_project() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let repos_path = tmp.path().join("repos.json");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-1");
        then.status(200).json_body(app_repos_body(&[("my-repo", "my-org/my-repo")]));
    });

    let projects = vec![make_project("proj-1", "My Project")];
    let results = run_discovery_for_all_projects(
        &projects,
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].project_id, "proj-1");
    assert_eq!(results[0].project_name, "My Project");
    assert_eq!(results[0].discovery.added, vec!["my-repo"]);
    assert!(results[0].discovery.not_found_on_disk.is_empty());
}

// Discovery runs independently for each project in the list.
#[tokio::test]
async fn test_discovers_repos_for_multiple_projects() {
    let tmp = TempDir::new().unwrap();
    let repo1 = tmp.path().join("repo-one");
    scaffold_git_repo_with_remote(&repo1, "git@github.com:org/repo-one.git");
    let repo2 = tmp.path().join("repo-two");
    scaffold_git_repo_with_remote(&repo2, "git@github.com:org/repo-two.git");
    let repos_path = tmp.path().join("repos.json");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-a");
        then.status(200).json_body(app_repos_body(&[("repo-one", "org/repo-one")]));
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-b");
        then.status(200).json_body(app_repos_body(&[("repo-two", "org/repo-two")]));
    });

    let projects = vec![make_project("proj-a", "Project A"), make_project("proj-b", "Project B")];
    let results = run_discovery_for_all_projects(
        &projects,
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;

    assert_eq!(results.len(), 2);
    let added: Vec<String> = results.iter().flat_map(|r| r.discovery.added.clone()).collect();
    assert!(added.contains(&"repo-one".to_string()));
    assert!(added.contains(&"repo-two".to_string()));
}

// Repos connected via GitHub App but not cloned locally land in not_found_on_disk.
#[tokio::test]
async fn test_not_found_repos_appear_in_discovery_result() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-1");
        then.status(200)
            .json_body(app_repos_body(&[("missing-repo", "my-org/missing-repo")]));
    });

    let projects = vec![make_project("proj-1", "My Project")];
    let results = run_discovery_for_all_projects(
        &projects,
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].discovery.not_found_on_disk, vec!["my-org/missing-repo"]);
    assert!(results[0].discovery.added.is_empty());
}

// An API failure for one project does not prevent discovery for other projects.
#[tokio::test]
async fn test_api_failure_returns_empty_result_and_other_projects_continue() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("ok-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:org/ok-repo.git");
    let repos_path = tmp.path().join("repos.json");

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-fail");
        then.status(500);
    });
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-ok");
        then.status(200).json_body(app_repos_body(&[("ok-repo", "org/ok-repo")]));
    });

    let projects = vec![
        make_project("proj-fail", "Failing Project"),
        make_project("proj-ok", "OK Project"),
    ];
    let results = run_discovery_for_all_projects(
        &projects,
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;

    assert_eq!(results.len(), 2);
    let fail = results.iter().find(|r| r.project_id == "proj-fail").unwrap();
    assert!(fail.discovery.added.is_empty(), "API failure must produce empty discovery");
    assert!(fail.discovery.not_found_on_disk.is_empty());
    let ok = results.iter().find(|r| r.project_id == "proj-ok").unwrap();
    assert_eq!(ok.discovery.added, vec!["ok-repo"]);
}

// A repo already in repos.json is reported as already_registered, not duplicated.
#[tokio::test]
async fn test_already_registered_repo_is_not_duplicated() {
    let tmp = TempDir::new().unwrap();
    let repo_dir = tmp.path().join("my-repo");
    scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
    let canonical = std::fs::canonicalize(&repo_dir).unwrap();
    let repos_path = tmp.path().join("repos.json");
    std::fs::write(
        &repos_path,
        serde_json::to_string(&serde_json::json!({
            "version": 1,
            "repos": [{"id":"existing-id","name":"my-repo","path":canonical.to_str().unwrap(),"active":true,"added_at":"2026-01-01T00:00:00Z"}]
        }))
        .unwrap(),
    )
    .unwrap();

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/v1/github/app-repos")
            .query_param("project_id", "proj-1");
        then.status(200).json_body(app_repos_body(&[("my-repo", "my-org/my-repo")]));
    });

    let projects = vec![make_project("proj-1", "My Project")];
    let results = run_discovery_for_all_projects(
        &projects,
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;

    assert_eq!(results.len(), 1);
    assert!(results[0].discovery.added.is_empty(), "must not duplicate already-registered repo");
    assert_eq!(results[0].discovery.already_registered, vec!["my-repo"]);
    let written: crate::storage::ReposConfig =
        serde_json::from_str(&std::fs::read_to_string(&repos_path).unwrap()).unwrap();
    assert_eq!(written.repos.len(), 1, "repos.json must not gain a second entry");
}

// No projects → empty results, no API calls made.
#[tokio::test]
async fn test_empty_projects_returns_empty_results() {
    let tmp = TempDir::new().unwrap();
    let repos_path = tmp.path().join("repos.json");
    let server = MockServer::start();
    // No mock routes registered — any call would fail with connection refused.
    let results = run_discovery_for_all_projects(
        &[],
        &server.base_url(),
        "tok",
        &[tmp.path().to_path_buf()],
        &repos_path,
    )
    .await;
    assert!(results.is_empty());
}

// ── candidate_dirs ────────────────────────────────────────────────────────────

// candidate_dirs returns the home directory as the first entry (when home exists).
#[test]
fn test_candidate_dirs_includes_home_dir() {
    let dirs = candidate_dirs();
    if let Some(home) = dirs::home_dir() {
        assert!(
            dirs.contains(&home),
            "candidate_dirs must include the home directory; got: {:?}",
            dirs
        );
    } else {
        assert!(dirs.is_empty(), "candidate_dirs must return empty vec when home is unavailable");
    }
}

// candidate_dirs includes every standard subdirectory under home.
#[test]
fn test_candidate_dirs_includes_all_standard_subdirs() {
    let dirs = candidate_dirs();
    if let Some(home) = dirs::home_dir() {
        for sub in &["GitHub", "Projects", "Developer", "Code", "src", "workspace"] {
            assert!(
                dirs.contains(&home.join(sub)),
                "candidate_dirs must include home/{sub}; got: {dirs:?}"
            );
        }
    }
}

// candidate_dirs contains exactly 7 entries (home + 6 subdirs) when home exists.
#[test]
fn test_candidate_dirs_count_when_home_exists() {
    let dirs = candidate_dirs();
    if dirs::home_dir().is_some() {
        assert_eq!(
            dirs.len(),
            7,
            "expected home + 6 subdirs = 7 entries; got {}: {:?}",
            dirs.len(),
            dirs
        );
    }
}
