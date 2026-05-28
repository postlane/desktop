// SPDX-License-Identifier: BUSL-1.1
// Repo auto-discovery: matches locally cloned repos against GitHub App repos.
// [include] and [includeIf] git config directives are not followed (21.10.4).
// Symlinks in candidate directories are not followed (21.10.11).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git_url_parser::{normalize_github_url, read_git_remote_urls, scan_for_git_dirs};
use crate::github_app::GitHubAppRepo;
use crate::storage::{write_repos, ReposConfig, Repo};

/// Result of a discovery scan. All four arms are always populated, even if empty.
#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct DiscoveryResult {
    pub added: Vec<String>,
    pub already_registered: Vec<String>,
    pub not_found_on_disk: Vec<String>,
    /// `(path, error_message)` for repos that matched but could not be registered.
    pub failed_to_register: Vec<(String, String)>,
}

/// Writes `.postlane/config.json` with the given `project_id` and appends a
/// new entry to `repos.json` for the given directory. Both writes are atomic.
///
/// Idempotent: if the canonical path is already present in `repos.json` the
/// function returns `Ok` without touching either file. This is the
/// defense-in-depth guard for Bug 21.13.6a — the primary guard lives in
/// `discover_repos_impl`, but this one protects against any future call path.
fn register_discovered_repo(repos_path: &Path, dir: &Path, project_id: &str) -> Result<String, String> {
    let canonical = std::fs::canonicalize(dir)
        .map_err(|e| format!("canonicalize {dir:?}: {e}"))?;
    let name = canonical.file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid folder name")?
        .to_string();
    let path_str = canonical.to_str().ok_or("invalid path")?.to_string();
    let mut cfg = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| ReposConfig { version: 1, workspaces: vec![], repos: vec![] });
    // Defense-in-depth: do not duplicate an already-registered path and do not
    // overwrite its config.json (which may belong to a different project).
    if cfg.repos.iter().any(|r| r.path == path_str) {
        return Ok(name);
    }
    crate::repo_init_config::write_initial_config_files(&canonical, project_id)?;
    cfg.repos.push(Repo {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.clone(),
        path: path_str,
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    });
    write_repos(repos_path, &cfg).map_err(|e| format!("write repos.json: {e:?}"))?;
    Ok(name)
}

pub fn discover_repos_impl(
    app_repos: &[GitHubAppRepo],
    base_dirs: &[PathBuf],
    repos_path: &Path,
    project_id: &str,
) -> DiscoveryResult {
    let git_dirs = scan_for_git_dirs(base_dirs, 500);
    let mut slug_to_path: HashMap<String, PathBuf> = HashMap::new();
    for dir in &git_dirs {
        for url in read_git_remote_urls(&dir.join(".git").join("config")) {
            if let Some(slug) = normalize_github_url(&url) {
                slug_to_path.insert(slug, dir.clone());
            }
        }
    }
    let existing = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| ReposConfig { version: 1, workspaces: vec![], repos: vec![] });
    let mut registered: HashSet<String> =
        existing.repos.iter().map(|r| r.path.to_lowercase()).collect();
    let mut result = DiscoveryResult::default();
    for app_repo in app_repos {
        let slug = app_repo.full_name.to_lowercase();
        match slug_to_path.get(&slug) {
            None => result.not_found_on_disk.push(app_repo.full_name.clone()),
            Some(dir) => {
                // Canonicalise before comparing: connect_repo_from_desktop_impl
                // stores the canonical path in repos.json, but scan_for_git_dirs
                // may return a non-canonical path (e.g. via a symlinked parent or
                // on macOS where /tmp resolves to /private/tmp).  Without this
                // step the pre-check misses the match and creates a duplicate
                // entry (Bug 21.13.6a).
                let canonical_dir = match std::fs::canonicalize(dir) {
                    Ok(p) => p,
                    Err(e) => {
                        result.failed_to_register.push((dir.to_string_lossy().into(), e.to_string()));
                        continue;
                    }
                };
                let path_lc = canonical_dir.to_string_lossy().to_lowercase();
                if registered.contains(&path_lc) {
                    result.already_registered.push(app_repo.name.clone());
                } else {
                    match register_discovered_repo(repos_path, &canonical_dir, project_id) {
                        Ok(name) => { registered.insert(path_lc); result.added.push(name); }
                        Err(e) => result.failed_to_register.push((canonical_dir.to_string_lossy().into(), e)),
                    }
                }
            }
        }
    }
    result
}

fn candidate_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let mut dirs = vec![home.clone()];
    for sub in &["GitHub", "Projects", "Developer", "Code", "src", "workspace"] {
        dirs.push(home.join(sub));
    }
    dirs
}

#[tauri::command]
pub async fn discover_repos(
    project_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<DiscoveryResult, String> {
    use crate::license::POSTLANE_API_BASE;
    use tauri_plugin_keyring::KeyringExt;

    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let app_repos = crate::github_app::list_github_app_repos_impl(POSTLANE_API_BASE, &project_id, &token).await?;
    let base_dirs = candidate_dirs();
    let repos_path = state.repos_path.clone();
    let pid = project_id.clone();

    let result = tokio::task::spawn_blocking(move || {
        discover_repos_impl(&app_repos, &base_dirs, &repos_path, &pid)
    })
    .await
    .map_err(|e| format!("discover_repos task failed: {}", e))?;

    if !result.added.is_empty() {
        if let Ok(new_config) = crate::storage::read_repos_with_recovery(&state.repos_path) {
            let known_ids: std::collections::HashSet<String> = state
                .lock_repos()
                .map(|r| r.repos.iter().map(|rr| rr.id.clone()).collect())
                .unwrap_or_default();
            let new_repos: Vec<_> = new_config.repos.iter()
                .filter(|r| !known_ids.contains(&r.id))
                .cloned()
                .collect();
            if let Ok(mut repos) = state.lock_repos() {
                *repos = new_config;
            }
            for repo in new_repos {
                crate::repo_mgmt::start_repo_watcher(&repo.id, &repo.path, &state, app.clone());
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_app_repo(name: &str, full_name: &str) -> GitHubAppRepo {
        GitHubAppRepo {
            id: 1,
            name: name.to_string(),
            full_name: full_name.to_string(),
            private: false,
            html_url: format!("https://github.com/{}", full_name),
        }
    }

    fn scaffold_git_repo_with_remote(dir: &Path, remote_url: &str) {
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        let cfg = format!(
            "[core]\n\trepofmt = 0\n[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/*\n",
            remote_url
        );
        std::fs::write(dir.join(".git/config"), cfg).unwrap();
    }

    // ── discover_repos_impl ──────────────────────────────────────────────────

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
        assert!(std::fs::read_dir(tmp.path()).unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn test_failed_write_in_failed_to_register() {
        let tmp = TempDir::new().unwrap();
        let repo_dir = tmp.path().join("my-repo");
        scaffold_git_repo_with_remote(&repo_dir, "git@github.com:my-org/my-repo.git");
        let app_repos = vec![make_app_repo("my-repo", "my-org/my-repo")];
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
        assert_eq!(config["version"].as_u64(), Some(1), "version must be 1");
        assert_eq!(
            config["project_id"].as_str(),
            Some("proj-xyz"),
            "field must be 'project_id', not 'project'"
        );
        assert_eq!(config["base_url"].as_str(), Some("https://postlane.dev"));
        assert!(config["llm"].is_object(), "llm block must be present");
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

        assert!(
            repo_dir.join(".postlane/config.local.json").exists(),
            "config.local.json must be written alongside config.json"
        );
        let local_str = std::fs::read_to_string(repo_dir.join(".postlane/config.local.json")).unwrap();
        let local: serde_json::Value = serde_json::from_str(&local_str).unwrap();
        assert!(local["scheduler"].is_object(), "config.local.json must have a scheduler block");
    }

    // ── Bug 21.13.6a — duplicate project via Add-folder + GitHub App ─────────

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
        assert_eq!(result.already_registered, vec!["my-repo"],
            "must report as already registered");
        let written: crate::storage::ReposConfig =
            serde_json::from_str(&std::fs::read_to_string(&repos_path).unwrap()).unwrap();
        assert_eq!(written.repos.len(), 1, "repos.json must not gain a second entry");
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
        assert_eq!(
            config["project_id"].as_str(), Some("proj-a"),
            "config.json must not be overwritten with proj-b"
        );
    }
}
