// SPDX-License-Identifier: BUSL-1.1

//! Syncs provider config to repos whose project_id matches a given credential.

use crate::app_state::AppState;
use std::path::{Path, PathBuf};

/// Returns the paths of all repos whose `.postlane/config.json` has the given `project_id`.
///
/// Reads `repos.json` from disk so that repos registered via the CLI after app
/// startup are included — in-memory state is not updated by CLI writes.
pub(crate) fn collect_matching_repo_paths(project_id: &str, state: &AppState) -> Vec<PathBuf> {
    let repos = match crate::storage::read_repos_with_recovery(&state.repos_path) {
        Ok(config) => config,
        Err(e) => {
            log::warn!(
                "[credential_repo_sync] failed to read repos.json at {:?}: {:?}",
                &state.repos_path, e
            );
            return vec![];
        }
    };
    repos
        .repos
        .iter()
        .filter_map(|repo| {
            let config_path = Path::new(&repo.path).join(".postlane/config.json");
            let config: serde_json::Value = crate::init::read_json_file(&config_path).ok()?;
            if config["project_id"].as_str() == Some(project_id) {
                Some(PathBuf::from(&repo.path))
            } else {
                None
            }
        })
        .collect()
}

/// Returns the `{workspace}/config.json` paths for all active workspaces whose
/// `id` matches `project_id`. Workspace configs are at the workspace root — no
/// `.postlane/` subdirectory — so the paths are already fully resolved.
pub(crate) fn collect_matching_workspace_config_paths(project_id: &str, state: &AppState) -> Vec<PathBuf> {
    let repos = match crate::storage::read_repos_with_recovery(&state.repos_path) {
        Ok(config) => config,
        Err(e) => {
            log::warn!(
                "[credential_repo_sync] failed to read repos.json at {:?}: {:?}",
                &state.repos_path, e
            );
            return vec![];
        }
    };
    repos
        .workspaces
        .iter()
        .filter(|w| w.active && w.id == project_id)
        .map(|w| Path::new(&w.workspace_path).join("config.json"))
        .collect()
}

/// Writes `provider` into `config.local.json` for every repo and workspace matching `project_id`.
pub(crate) fn write_provider_to_matching_repos(project_id: &str, provider: &str, state: &AppState) {
    for repo_path in collect_matching_repo_paths(project_id, state) {
        if let Err(e) =
            crate::config_merge::write_scheduler_provider_to_local_config(&repo_path, provider)
        {
            log::warn!(
                "[save_scheduler_credential] write provider to {}: {}",
                repo_path.display(),
                e
            );
        }
    }
    for ws_config_path in collect_matching_workspace_config_paths(project_id, state) {
        if let Some(ws_path) = ws_config_path.parent() {
            if let Err(e) =
                crate::config_merge::write_scheduler_provider_to_local_config(ws_path, provider)
            {
                log::warn!(
                    "[save_scheduler_credential] write provider to workspace {}: {}",
                    ws_path.display(),
                    e
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_repo, make_state};

    fn make_test_state_with_repo(repo_path: &str) -> AppState {
        make_state(vec![make_repo("test-repo-id", repo_path)])
    }

    #[test]
    fn collect_matching_repo_paths_returns_empty_when_no_repos() {
        let state = make_state(vec![]);
        let paths = collect_matching_repo_paths("proj-abc", &state);
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_matching_repo_paths_returns_empty_when_config_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let paths = collect_matching_repo_paths("proj-abc", &state);
        assert!(paths.is_empty(), "missing config.json must not match");
    }

    #[test]
    fn collect_matching_repo_paths_returns_path_when_project_id_matches() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"my-proj"}"#)
            .expect("write config.json");
        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let repos = state.repos.lock().unwrap().clone();
        crate::storage::write_repos(&state.repos_path, &repos).expect("write repos.json");
        let paths = collect_matching_repo_paths("my-proj", &state);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], dir.path());
    }

    // ── bug fix: CLI-registered repos must be found ─────────────────────────

    #[test]
    fn collect_matching_repo_paths_finds_repo_registered_via_cli_since_startup() {
        let dir_a = tempfile::TempDir::new().expect("temp dir a");
        let dir_b = tempfile::TempDir::new().expect("temp dir b");

        let postlane_b = dir_b.path().join(".postlane");
        std::fs::create_dir_all(&postlane_b).expect("mkdir .postlane b");
        std::fs::write(postlane_b.join("config.json"), r#"{"project_id":"proj-b"}"#)
            .expect("write config b");

        // App started with only repo-a in memory
        let state = make_test_state_with_repo(dir_a.path().to_str().unwrap());

        // CLI registers repo-b by writing repos.json directly — state.repos is NOT updated
        let repos_on_disk = crate::storage::ReposConfig {
            version: 1, workspaces: vec![], repos: vec![
                make_repo("repo-a", dir_a.path().to_str().unwrap()),
                make_repo("repo-b", dir_b.path().to_str().unwrap()),
            ],
        };
        crate::storage::write_repos(&state.repos_path, &repos_on_disk)
            .expect("write repos.json");

        let paths = collect_matching_repo_paths("proj-b", &state);
        assert_eq!(paths.len(), 1, "repo registered via CLI must be found from disk");
        assert_eq!(paths[0], dir_b.path());
    }

    #[test]
    fn write_provider_to_matching_repos_no_op_when_no_repos() {
        let state = make_state(vec![]);
        write_provider_to_matching_repos("proj-abc", "zernio", &state);
    }

    #[test]
    fn write_provider_to_matching_repos_writes_provider_when_project_id_matches() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-abc"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":""}}"#)
            .expect("write config.local.json");

        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let repos = state.repos.lock().unwrap().clone();
        crate::storage::write_repos(&state.repos_path, &repos).expect("write repos.json");
        write_provider_to_matching_repos("proj-abc", "zernio", &state);

        let written = std::fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
    }

    #[test]
    fn write_provider_to_matching_repos_skips_non_matching_project_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"other-proj"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":""}}"#)
            .expect("write config.local.json");

        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        write_provider_to_matching_repos("proj-abc", "zernio", &state);

        let written = std::fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(
            v["scheduler"]["provider"].as_str(),
            Some(""),
            "non-matching repo must not be modified"
        );
    }

    // ── collect_matching_workspace_config_paths ───────────────────────────────

    fn make_state_with_workspace(ws_path: &str, project_id: &str) -> AppState {
        use crate::workspace_entry::WorkspaceEntry;
        let config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: project_id.to_string(),
                name: "ws".to_string(),
                workspace_path: ws_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let state = crate::test_fixtures::make_state(vec![]);
        // Overwrite the in-memory config and write to disk so collect_matching_workspace_config_paths
        // (which reads repos_path from disk) can find the workspace entry.
        {
            let mut repos = state.repos.lock().expect("lock");
            *repos = config.clone();
        }
        crate::storage::write_repos(&state.repos_path, &config).expect("write repos.json");
        state
    }

    #[test]
    fn collect_matching_workspace_config_paths_returns_workspace_config_for_matching_project() {
        let dir = tempfile::TempDir::new().expect("tmp dir");
        let ws_path = dir.path().to_str().unwrap();
        let state = make_state_with_workspace(ws_path, "proj-ws-123");
        let paths = collect_matching_workspace_config_paths("proj-ws-123", &state);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], dir.path().join("config.json"),
            "must return {{workspace}}/config.json, not {{workspace}}/.postlane/config.json");
    }

    #[test]
    fn collect_matching_workspace_config_paths_returns_empty_for_non_matching_id() {
        let dir = tempfile::TempDir::new().expect("tmp dir");
        let state = make_state_with_workspace(dir.path().to_str().unwrap(), "proj-ws-123");
        let paths = collect_matching_workspace_config_paths("different-project", &state);
        assert!(paths.is_empty(), "non-matching project_id must return empty");
    }

    #[test]
    fn write_provider_to_matching_repos_writes_to_workspace_config_local() {
        let dir = tempfile::TempDir::new().expect("tmp dir");
        let state = make_state_with_workspace(dir.path().to_str().unwrap(), "proj-ws-abc");
        write_provider_to_matching_repos("proj-ws-abc", "upload_post", &state);
        let local = std::fs::read_to_string(dir.path().join("config.local.json"))
            .expect("config.local.json must be written");
        let v: serde_json::Value = serde_json::from_str(&local).expect("parse");
        assert_eq!(
            v["scheduler"]["provider"].as_str(),
            Some("upload_post"),
            "provider must be written to workspace config.local.json"
        );
    }

    // ── error paths: unsupported repos.json version ───────────────────────────

    /// Lines 15-16,18,20 — read_repos_with_recovery returns Err on version mismatch;
    /// collect_matching_repo_paths must log and return empty vec, not panic.
    #[test]
    fn collect_matching_repo_paths_returns_empty_when_repos_json_version_unsupported() {
        let state = make_state(vec![]);
        std::fs::write(
            &state.repos_path,
            r#"{"version":99,"workspaces":[],"repos":[]}"#,
        )
        .expect("write repos.json with unsupported version");
        let paths = collect_matching_repo_paths("proj-abc", &state);
        assert!(
            paths.is_empty(),
            "unsupported repos.json version must return empty vec, not panic"
        );
    }

    /// Lines 45-50 — same version-mismatch error path for the workspace variant.
    #[test]
    fn collect_matching_workspace_config_paths_returns_empty_when_repos_json_version_unsupported() {
        let state = make_state(vec![]);
        std::fs::write(
            &state.repos_path,
            r#"{"version":99,"workspaces":[],"repos":[]}"#,
        )
        .expect("write repos.json with unsupported version");
        let paths = collect_matching_workspace_config_paths("proj-ws-abc", &state);
        assert!(
            paths.is_empty(),
            "unsupported repos.json version must return empty vec, not panic"
        );
    }

    // ── error paths: write failures in write_provider_to_matching_repos ───────

    /// Lines 64,67,69 — write_scheduler_provider_to_local_config fails for a matching
    /// repo (config.local.json blocked by a directory); function must log and not panic.
    #[test]
    fn write_provider_to_matching_repos_continues_gracefully_on_repo_write_failure() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-repo-fail"}"#)
            .expect("write config.json");
        // Block atomic_write by placing a directory at config.local.json — rename cannot
        // overwrite a directory, so write_scheduler_provider_to_local_config returns Err.
        std::fs::create_dir_all(postlane.join("config.local.json"))
            .expect("place dir at config.local.json");

        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let repos = state.repos.lock().expect("lock repos").clone();
        crate::storage::write_repos(&state.repos_path, &repos).expect("write repos.json");

        write_provider_to_matching_repos("proj-repo-fail", "zernio", &state);

        assert!(
            postlane.join("config.local.json").is_dir(),
            "directory at config.local.json must not be replaced — write was blocked"
        );
    }

    /// Lines 76,79,81 — write_scheduler_provider_to_local_config fails for a matching
    /// workspace; function must log and not panic.
    #[test]
    fn write_provider_to_matching_repos_continues_gracefully_on_workspace_write_failure() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // Block atomic_write for the workspace-root config.local.json
        std::fs::create_dir_all(dir.path().join("config.local.json"))
            .expect("place dir at config.local.json");

        let state = make_state_with_workspace(dir.path().to_str().unwrap(), "proj-ws-fail");
        write_provider_to_matching_repos("proj-ws-fail", "zernio", &state);

        assert!(
            dir.path().join("config.local.json").is_dir(),
            "directory at config.local.json must not be replaced — write was blocked"
        );
    }
}
