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

pub(crate) fn candidate_dirs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        return vec![];
    };
    let mut dirs = vec![home.clone()];
    for sub in &["GitHub", "Projects", "Developer", "Code", "src", "workspace"] {
        dirs.push(home.join(sub));
    }
    dirs
}

/// Aggregated result for one project's discovery pass.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProjectDiscoveryResult {
    pub project_id: String,
    pub project_name: String,
    pub discovery: DiscoveryResult,
}

/// Runs `discover_repos_impl` for every project in `projects`, fetching each
/// project's GitHub App repos from the API. Projects whose API call fails are
/// included in the result with an empty `DiscoveryResult` — the loop always
/// continues so a single failure does not block the remaining projects.
pub async fn run_discovery_for_all_projects(
    projects: &[crate::project_registry::ProjectSummary],
    api_base: &str,
    token: &str,
    base_dirs: &[PathBuf],
    repos_path: &Path,
) -> Vec<ProjectDiscoveryResult> {
    log::info!(
        "[discovery] starting: {} project(s), {} base dir(s): {:?}",
        projects.len(), base_dirs.len(), base_dirs,
    );
    let mut results = Vec::with_capacity(projects.len());
    for project in projects {
        let result = discover_for_project(project, api_base, token, base_dirs, repos_path).await;
        results.push(result);
    }
    log::info!("[discovery] complete for all projects");
    results
}

async fn discover_for_project(
    project: &crate::project_registry::ProjectSummary,
    api_base: &str,
    token: &str,
    base_dirs: &[PathBuf],
    repos_path: &Path,
) -> ProjectDiscoveryResult {
    log::info!("[discovery] project '{}' ({}): fetching GitHub App repos", project.name, project.id);
    let app_repos = match crate::github_app::list_github_app_repos_impl(api_base, &project.id, token).await {
        Ok(repos) => repos,
        Err(e) => {
            log::warn!("[discovery] project '{}' ({}): GitHub App API failed — {}", project.name, project.id, e);
            return ProjectDiscoveryResult {
                project_id: project.id.clone(),
                project_name: project.name.clone(),
                discovery: DiscoveryResult::default(),
            };
        }
    };
    let names: Vec<&str> = app_repos.iter().map(|r| r.full_name.as_str()).collect();
    log::info!(
        "[discovery] project '{}' ({}): GitHub App returned {} repo(s): {:?}",
        project.name, project.id, names.len(), names,
    );
    let dirs = base_dirs.to_vec();
    let path = repos_path.to_path_buf();
    let pid = project.id.clone();
    let discovery = tokio::task::spawn_blocking(move || discover_repos_impl(&app_repos, &dirs, &path, &pid))
        .await
        .unwrap_or_else(|e| {
            log::warn!("[discovery] task panicked for project '{}': {}", project.id, e);
            DiscoveryResult::default()
        });
    log::info!(
        "[discovery] project '{}' ({}): added={:?} already_registered={:?} not_found_on_disk={:?} failed={:?}",
        project.name, project.id,
        discovery.added, discovery.already_registered, discovery.not_found_on_disk,
        discovery.failed_to_register.iter().map(|(p, e)| format!("{p}: {e}")).collect::<Vec<_>>(),
    );
    ProjectDiscoveryResult { project_id: project.id.clone(), project_name: project.name.clone(), discovery }
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
#[path = "repo_discovery_tests.rs"]
mod tests;
