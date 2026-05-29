// SPDX-License-Identifier: BUSL-1.1
//! Tauri command and supporting logic for the repo connection-status table.
//!
//! Each row surfaces three independent signals for one repo:
//!   github_app_connected — the GitHub App API returned this repo for the project
//!   folder_registered    — the repo has an active entry in repos.json
//!   cli_initialized      — `.postlane/config.json` exists at the local path

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::git_url_parser::{normalize_github_url, read_git_remote_urls};
use crate::github_app::GitHubAppRepo;
use crate::storage::Repo;

// ── public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct RepoConnectionStatus {
    /// The `repos.json` ID — `None` for GitHub App–only rows (not yet registered locally).
    /// Required by the frontend to call `unregister_repo`.
    pub repo_id: Option<String>,
    /// `"owner/repo"` slug — from the GitHub App API, or derived from the
    /// registered repo's git remote. `None` for non-GitHub remotes.
    pub github_full_name: Option<String>,
    /// Absolute local path from `repos.json`. `None` if only known via the
    /// GitHub App and not yet cloned or registered on this machine.
    pub local_path: Option<String>,
    /// Best display name: GitHub repo name when available, else `repos.json` name.
    pub display_name: String,
    /// GitHub App returned this repo for the queried `project_id`.
    pub github_app_connected: bool,
    /// Repo has an active entry in `repos.json` (folder was connected in the UI).
    pub folder_registered: bool,
    /// `{local_path}/.postlane/config.json` exists — `postlane init` was run.
    pub cli_initialized: bool,
    /// `config.json` exists but its `project_id` does not match the queried one.
    /// Signals the repo is initialised for a different project.
    pub project_id_mismatch: bool,
}

// ── internal helpers (pub(crate) for tests) ───────────────────────────────────

/// Returns `(cli_initialized, project_id_mismatch)`.
/// `cli_initialized` is true when `.postlane/config.json` exists at `path`.
/// `project_id_mismatch` is true when the file exists, is valid JSON, and
/// contains a `project_id` field that differs from `expected_project_id`.
pub(crate) fn check_cli_state(path: &Path, expected_project_id: &str) -> (bool, bool) {
    let config_path = path.join(".postlane").join("config.json");
    if !config_path.exists() {
        return (false, false);
    }
    let content = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => return (true, false),
    };
    let val: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return (true, false),
    };
    let stored = val.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
    let mismatch = !stored.is_empty() && stored != expected_project_id;
    (true, mismatch)
}

/// Builds a map from normalised GitHub slug → `&Repo` for every active repo
/// whose `.git/config` contains a parseable GitHub remote URL.
fn slug_map_from_registered<'a>(repos: &'a [Repo], project_id: &str) -> HashMap<String, &'a Repo> {
    let mut map = HashMap::new();
    for repo in repos {
        if !repo.active {
            continue;
        }
        let path = Path::new(&repo.path);
        let (cli, mismatch) = check_cli_state(path, project_id);
        if cli && mismatch {
            continue; // belongs to a different project — exclude from slug matching
        }
        let git_config = path.join(".git").join("config");
        for url in read_git_remote_urls(&git_config) {
            if let Some(slug) = normalize_github_url(&url) {
                map.insert(slug, repo);
                break;
            }
        }
    }
    map
}

/// Merges GitHub App repos and locally registered repos into a unified row list.
///
/// Match key: normalised GitHub slug (`owner/repo`, lowercase). A repo that
/// appears in both sources produces exactly one row with both flags set.
/// Rows are sorted: GitHub App rows first (alphabetical), then local-only rows
/// (alphabetical).
pub(crate) fn merge_into_rows(
    app_repos: &[GitHubAppRepo],
    registered: &[Repo],
    project_id: &str,
) -> Vec<RepoConnectionStatus> {
    let slug_map = slug_map_from_registered(registered, project_id);
    let mut rows = Vec::new();
    let mut matched_paths: std::collections::HashSet<&str> = Default::default();

    for app_repo in app_repos {
        let slug = app_repo.full_name.to_lowercase();
        let reg = slug_map.get(&slug).copied();
        if let Some(r) = reg {
            matched_paths.insert(r.path.as_str());
        }
        let (cli, mismatch) = reg
            .map(|r| check_cli_state(Path::new(&r.path), project_id))
            .unwrap_or((false, false));
        rows.push(RepoConnectionStatus {
            repo_id: reg.map(|r| r.id.clone()),
            github_full_name: Some(app_repo.full_name.clone()),
            local_path: reg.map(|r| r.path.clone()),
            display_name: app_repo.name.clone(),
            github_app_connected: true,
            folder_registered: reg.is_some(),
            cli_initialized: cli,
            project_id_mismatch: mismatch,
        });
    }

    for repo in registered {
        if !repo.active || matched_paths.contains(repo.path.as_str()) {
            continue;
        }
        let path = Path::new(&repo.path);
        if !path.exists() {
            continue; // stale registration — path was deleted or moved
        }
        let (cli, mismatch) = check_cli_state(path, project_id);
        if cli && mismatch {
            continue; // registered for a different project — not shown in this org's table
        }
        let git_config = path.join(".git").join("config");
        let github_slug = read_git_remote_urls(&git_config)
            .into_iter()
            .find_map(|u| normalize_github_url(&u));
        rows.push(RepoConnectionStatus {
            repo_id: Some(repo.id.clone()),
            github_full_name: github_slug,
            local_path: Some(repo.path.clone()),
            display_name: repo.name.clone(),
            github_app_connected: false,
            folder_registered: true,
            cli_initialized: cli,
            project_id_mismatch: mismatch,
        });
    }

    rows.sort_by_key(|r| (!r.github_app_connected, r.display_name.to_lowercase()));
    rows
}

// ── Tauri command ─────────────────────────────────────────────────────────────

/// Returns a merged list of all repos the GitHub App knows about and all repos
/// registered locally in `repos.json`, with connection-status flags for each.
///
/// The GitHub App API call is best-effort: if it fails the command still
/// returns local rows with `github_app_connected = false`.
#[tauri::command]
pub async fn get_repo_connection_status(
    project_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::app_state::AppState>,
) -> Result<Vec<RepoConnectionStatus>, String> {
    use crate::license::POSTLANE_API_BASE;
    use tauri_plugin_keyring::KeyringExt;

    let token = app
        .keyring()
        .get_password("postlane", "license")
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())?;

    let app_repos =
        crate::github_app::list_github_app_repos_impl(POSTLANE_API_BASE, &project_id, &token)
            .await
            .unwrap_or_else(|e| {
                log::warn!("[repo_status] GitHub App API failed for '{}': {}", project_id, e);
                vec![]
            });

    let registered = {
        let repos = state.lock_repos()?;
        repos.repos.iter().filter(|r| r.active).cloned().collect::<Vec<_>>()
    };

    Ok(merge_into_rows(&app_repos, &registered, &project_id))
}

#[cfg(test)]
#[path = "repo_connection_status_tests.rs"]
mod tests;
