// SPDX-License-Identifier: BUSL-1.1

//! Orchestrates background and on-demand refresh of scheduler account profiles.
//!
//! `collect_sync_tasks` is a pure function that determines what needs syncing.
//! `refresh_scheduler_accounts_impl` executes those tasks against the provider API.
//! The Tauri command `refresh_scheduler_accounts` exposes this to the frontend.

use std::collections::HashMap;
use std::path::PathBuf;

/// Result returned to the frontend after a refresh run.
#[derive(Debug, serde::Serialize, Clone)]
pub struct RefreshResult {
    /// Provider names (e.g. "zernio") that were successfully contacted.
    pub providers_synced: Vec<String>,
    /// Human-readable error messages from providers that could not be reached.
    pub errors: Vec<String>,
}

/// Groups active repos by project_id (read from config.json).
/// Falls back to `repo.id` when config.json is absent or lacks `project_id`.
fn group_repos_by_project(repos: &[crate::storage::Repo]) -> HashMap<String, Vec<PathBuf>> {
    let mut map: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for repo in repos.iter().filter(|r| r.active) {
        let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
        let project_id = crate::connected_platforms::read_project_id_from_config(&config_path)
            .unwrap_or_else(|| repo.id.clone());
        map.entry(project_id).or_default().push(PathBuf::from(&repo.path));
    }
    map
}

/// Returns the list of `(provider, api_key, repo_paths)` tasks that should run.
///
/// Pure function -- reads config.json files but makes no network calls.
/// `get_credential(provider, project_id)` returns `Some(key)` when a credential exists.
pub fn collect_sync_tasks(
    repos: &[crate::storage::Repo],
    providers: &[&str],
    get_credential: &dyn Fn(&str, &str) -> Option<String>,
) -> Vec<(String, String, Vec<PathBuf>)> {
    let by_project = group_repos_by_project(repos);
    let mut tasks = Vec::new();
    for (project_id, repo_paths) in &by_project {
        for &provider in providers {
            if let Some(key) = get_credential(provider, project_id) {
                tasks.push((provider.to_string(), key, repo_paths.clone()));
            }
        }
    }
    tasks
}

/// Runs account sync for all providers that have credentials, across all active repos.
/// Errors from individual providers are collected and returned -- they do not abort other syncs.
pub async fn refresh_scheduler_accounts_impl(
    repos: &[crate::storage::Repo],
    get_credential: &(dyn Fn(&str, &str) -> Option<String> + Sync),
) -> RefreshResult {
    let providers = crate::scheduler_credentials::VALID_PROVIDERS.to_vec();
    let tasks = collect_sync_tasks(repos, &providers, get_credential);
    let mut providers_synced = Vec::new();
    let mut errors = Vec::new();
    for (provider, api_key, repo_paths) in tasks {
        match crate::account_config::sync_accounts_for_provider(&provider, &api_key, repo_paths).await {
            Ok(_) => providers_synced.push(provider),
            Err(e) => errors.push(format!("{}: {}", provider, e)),
        }
    }
    RefreshResult { providers_synced, errors }
}

/// Verifies `api_key` is accepted by `provider_name` without saving anything.
/// Returns `Err` if the provider is unknown or the API rejects the key.
pub async fn validate_provider_credential(
    provider_name: &str,
    api_key: &str,
) -> Result<(), String> {
    let provider = crate::scheduling::credential_router::build_provider(provider_name, api_key.to_string())
        .map_err(|e| format!("build provider {}: {}", provider_name, e))?;
    provider.test_connection().await
        .map_err(|e| format!("{}: {}", provider_name, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Repo;
    use std::fs;

    fn make_repo(id: &str, path: &str, active: bool) -> Repo {
        Repo {
            id: id.to_string(),
            name: "test".to_string(),
            path: path.to_string(),
            active,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn write_config_with_project(dir: &std::path::Path, project_id: &str) {
        let d = dir.join(".postlane");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("config.json"),
            format!(r#"{{"version":1,"project_id":"{}"}}"#, project_id),
        )
        .unwrap();
    }

    // ── collect_sync_tasks ────────────────────────────────────────────────────

    #[test]
    fn collect_sync_tasks_returns_empty_for_no_repos() {
        let tasks = collect_sync_tasks(&[], &["zernio"], &|_, _| None);
        assert!(tasks.is_empty());
    }

    #[test]
    fn collect_sync_tasks_returns_empty_when_no_credential() {
        let dir = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir.path(), "proj-1");
        let repo = make_repo("r1", dir.path().to_str().unwrap(), true);
        let tasks = collect_sync_tasks(&[repo], &["zernio"], &|_, _| None);
        assert!(tasks.is_empty(), "no credential → no tasks");
    }

    #[test]
    fn collect_sync_tasks_returns_task_when_credential_present() {
        let dir = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir.path(), "proj-1");
        let repo = make_repo("r1", dir.path().to_str().unwrap(), true);
        let tasks = collect_sync_tasks(
            &[repo],
            &["zernio"],
            &|p, pid| (p == "zernio" && pid == "proj-1").then(|| "key-abc".to_string()),
        );
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].0, "zernio");
        assert_eq!(tasks[0].1, "key-abc");
        assert_eq!(tasks[0].2.len(), 1);
    }

    #[test]
    fn collect_sync_tasks_groups_repos_in_same_project_into_one_task() {
        let dir1 = tempfile::TempDir::new().unwrap();
        let dir2 = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir1.path(), "proj-shared");
        write_config_with_project(dir2.path(), "proj-shared");
        let repos = vec![
            make_repo("r1", dir1.path().to_str().unwrap(), true),
            make_repo("r2", dir2.path().to_str().unwrap(), true),
        ];
        let tasks = collect_sync_tasks(&repos, &["zernio"], &|_, _| Some("k".to_string()));
        assert_eq!(tasks.len(), 1, "one project → one task");
        assert_eq!(tasks[0].2.len(), 2, "both repo paths included");
    }

    #[test]
    fn collect_sync_tasks_produces_separate_tasks_for_different_projects() {
        let dir1 = tempfile::TempDir::new().unwrap();
        let dir2 = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir1.path(), "proj-A");
        write_config_with_project(dir2.path(), "proj-B");
        let repos = vec![
            make_repo("r1", dir1.path().to_str().unwrap(), true),
            make_repo("r2", dir2.path().to_str().unwrap(), true),
        ];
        let tasks = collect_sync_tasks(&repos, &["zernio"], &|_, _| Some("k".to_string()));
        assert_eq!(tasks.len(), 2, "two projects → two tasks");
    }

    #[test]
    fn collect_sync_tasks_skips_inactive_repos() {
        let dir = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir.path(), "proj-1");
        let repo = make_repo("r1", dir.path().to_str().unwrap(), false); // inactive
        let tasks = collect_sync_tasks(&[repo], &["zernio"], &|_, _| Some("k".to_string()));
        assert!(tasks.is_empty(), "inactive repo must not produce a task");
    }

    #[test]
    fn collect_sync_tasks_falls_back_to_repo_id_when_config_absent() {
        let dir = tempfile::TempDir::new().unwrap(); // no config.json
        let repo = make_repo("fallback-id", dir.path().to_str().unwrap(), true);
        let tasks = collect_sync_tasks(
            &[repo],
            &["zernio"],
            &|_, pid| (pid == "fallback-id").then(|| "k".to_string()),
        );
        assert_eq!(tasks.len(), 1, "repo.id used as fallback project_id");
    }

    #[test]
    fn collect_sync_tasks_produces_one_task_per_provider_with_credential() {
        let dir = tempfile::TempDir::new().unwrap();
        write_config_with_project(dir.path(), "proj-1");
        let repo = make_repo("r1", dir.path().to_str().unwrap(), true);
        let tasks = collect_sync_tasks(
            &[repo],
            &["zernio", "publer"],
            &|_, _| Some("k".to_string()),
        );
        assert_eq!(tasks.len(), 2, "two providers → two tasks");
        let providers: Vec<&str> = tasks.iter().map(|t| t.0.as_str()).collect();
        assert!(providers.contains(&"zernio"));
        assert!(providers.contains(&"publer"));
    }

    // ── validate_provider_credential ─────────────────────────────────────────

    #[tokio::test]
    async fn validate_provider_credential_returns_err_for_unknown_provider() {
        let result = validate_provider_credential("not_a_real_provider", "any_key").await;
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("not_a_real_provider") || msg.contains("build provider"), "got: {}", msg);
    }

    #[tokio::test]
    async fn validate_provider_credential_returns_err_for_unreachable_webhook() {
        // Webhook test_connection POSTs to the URL; unreachable host must surface as Err.
        let result = validate_provider_credential("webhook", "https://does-not-exist.example.invalid/hook").await;
        assert!(result.is_err(), "unreachable webhook must fail validation");
    }
}
