// SPDX-License-Identifier: BUSL-1.1
// Repo auto-discovery: matches locally cloned repos against GitHub App repos.
// [include] and [includeIf] git config directives are not followed (21.10.4).
// Symlinks in candidate directories are not followed (21.10.11).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

/// Normalises a GitHub remote URL to a lowercase `owner/repo` slug.
/// Returns `None` for non-GitHub remotes.
pub fn normalize_github_url(url: &str) -> Option<String> {
    let url = url.trim();
    let slug = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest.strip_suffix(".git").unwrap_or(rest)
    } else if let Some(rest) = url.strip_prefix("https://github.com/") {
        rest.strip_suffix(".git").unwrap_or(rest).trim_end_matches('/')
    } else {
        return None;
    };
    let (owner, repo) = slug.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(slug.to_lowercase())
}

/// Reads all remote URL values from a `.git/config` file.
/// Returns an empty vec on read error; never panics.
pub fn read_git_remote_urls(git_config_path: &Path) -> Vec<String> {
    match std::fs::read_to_string(git_config_path) {
        Ok(content) => extract_remote_urls_from_config(&content),
        Err(_) => vec![],
    }
}

// [include] and [includeIf] are not followed — repos connected only via a
// conditional include are excluded from discovery (documented in 21.10.4).
fn extract_remote_urls_from_config(content: &str) -> Vec<String> {
    let mut in_remote = false;
    let mut urls = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_remote = section_is_remote(line);
            continue;
        }
        if in_remote {
            if let Some(url) = extract_url_key_value(line) {
                urls.push(url);
            }
        }
    }
    urls
}

fn section_is_remote(header: &str) -> bool {
    let inner = header.trim_start_matches('[').split(']').next().unwrap_or("").trim();
    let mut parts = inner.splitn(2, '"');
    matches!(parts.next().map(str::trim), Some("remote")) && parts.next().is_some()
}

fn extract_url_key_value(line: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    if k.trim() == "url" { Some(v.trim().to_string()) } else { None }
}

/// Scans `base_dirs` for directories containing `.git/`, up to 2 levels deep.
/// Stops after examining `limit` total directories. Symlinks are not followed.
pub fn scan_for_git_dirs(base_dirs: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    use std::collections::VecDeque;
    let mut found = Vec::new();
    let mut queue: VecDeque<(PathBuf, u8)> = VecDeque::new();
    for base in base_dirs {
        if base.is_dir() {
            queue.push_back((base.clone(), 0));
        }
    }
    let mut count = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if count >= limit {
            break;
        }
        count += 1;
        if dir.join(".git").is_dir() {
            found.push(dir);
            continue;
        }
        if depth >= 2 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            log::warn!("repo_discovery: cannot read {:?}", dir);
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_symlink() && meta.is_dir() {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    found
}

/// Writes `.postlane/config.json` with the given `project_id` and appends a
/// new entry to `repos.json` for the given directory. Both writes are atomic.
fn register_discovered_repo(repos_path: &Path, dir: &Path, project_id: &str) -> Result<String, String> {
    let canonical = std::fs::canonicalize(dir)
        .map_err(|e| format!("canonicalize {dir:?}: {e}"))?;
    let name = canonical.file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid folder name")?
        .to_string();
    crate::project_config_ops::write_initial_config_files(&canonical, project_id)?;
    let path_str = canonical.to_str().ok_or("invalid path")?.to_string();
    let mut cfg = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_else(|_| ReposConfig { version: 1, repos: vec![] });
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
        .unwrap_or_else(|_| ReposConfig { version: 1, repos: vec![] });
    let mut registered: HashSet<String> =
        existing.repos.iter().map(|r| r.path.to_lowercase()).collect();
    let mut result = DiscoveryResult::default();
    for app_repo in app_repos {
        let slug = app_repo.full_name.to_lowercase();
        match slug_to_path.get(&slug) {
            None => result.not_found_on_disk.push(app_repo.full_name.clone()),
            Some(dir) => {
                let path_lc = dir.to_string_lossy().to_lowercase();
                if registered.contains(&path_lc) {
                    result.already_registered.push(app_repo.name.clone());
                } else {
                    match register_discovered_repo(repos_path, dir, project_id) {
                        Ok(name) => { registered.insert(path_lc); result.added.push(name); }
                        Err(e) => result.failed_to_register.push((dir.to_string_lossy().into(), e)),
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

    // ── normalize_github_url ──────────────────────────────────────────────────

    #[test]
    fn test_normalize_ssh_url() {
        assert_eq!(
            normalize_github_url("git@github.com:my-org/my-repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_https_url() {
        assert_eq!(
            normalize_github_url("https://github.com/my-org/my-repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_https_no_git_suffix() {
        assert_eq!(
            normalize_github_url("https://github.com/my-org/my-repo"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_lowercases() {
        assert_eq!(
            normalize_github_url("git@github.com:My-Org/My-Repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_gitlab_returns_none() {
        assert_eq!(normalize_github_url("git@gitlab.com:org/repo.git"), None);
    }

    #[test]
    fn test_normalize_bitbucket_https_returns_none() {
        assert_eq!(normalize_github_url("https://bitbucket.org/org/repo.git"), None);
    }

    // ── read_git_remote_urls ─────────────────────────────────────────────────

    #[test]
    fn test_read_ssh_remote_from_git_config() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(&cfg, "[remote \"origin\"]\n\turl = git@github.com:org/repo.git\n").unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:org/repo.git"]);
    }

    #[test]
    fn test_read_returns_empty_for_missing_file() {
        assert!(read_git_remote_urls(Path::new("/no/such/file")).is_empty());
    }

    #[test]
    fn test_read_skips_non_remote_sections() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(
            &cfg,
            "[core]\n\turl = not-a-real-url\n[remote \"origin\"]\n\turl = git@github.com:org/r.git\n",
        ).unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:org/r.git"]);
    }

    #[test]
    fn test_read_skips_comment_lines() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(
            &cfg,
            "[remote \"origin\"]\n# url = git@github.com:bad/repo.git\n\turl = git@github.com:good/repo.git\n",
        ).unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:good/repo.git"]);
    }

    // ── scan_for_git_dirs ────────────────────────────────────────────────────

    #[test]
    fn test_scan_finds_git_dir_at_depth_1() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.contains(&repo));
    }

    #[test]
    fn test_scan_finds_git_dir_at_depth_2() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("projects").join("my-repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.contains(&repo));
    }

    #[test]
    fn test_scan_does_not_exceed_limit() {
        let tmp = TempDir::new().unwrap();
        for i in 0..600u32 {
            std::fs::create_dir_all(tmp.path().join(format!("dir-{i}")).join(".git")).unwrap();
        }
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.len() <= 500, "must not exceed 500");
    }

    #[test]
    fn test_scan_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real-repo");
        std::fs::create_dir_all(real.join(".git")).unwrap();
        let link = tmp.path().join("link-repo");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        // real-repo should be found; link-repo (symlink) should not be
        assert!(found.contains(&real));
        assert!(!found.contains(&link));
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
        let existing = serde_json::json!({
            "version": 1,
            "repos": [{"id":"u1","name":"my-repo","path": repo_dir.to_str().unwrap(),"active":true,"added_at":"2024-01-01T00:00:00Z"}]
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
        // File must be valid JSON after the write
        let raw = std::fs::read_to_string(&repos_path).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&raw).is_ok());
        // No .tmp files should remain
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
        // Use a path that IS a directory so the atomic write fails
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
}
