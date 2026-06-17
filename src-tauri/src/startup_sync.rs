// SPDX-License-Identifier: BUSL-1.1

//! Startup sync: refreshes `connected_platforms` in every active repo's
//! `config.json` from the current keyring state on each app launch.
//!
//! Ensures the field stays accurate across restarts, including cases where
//! credentials are added or removed between sessions.

use std::path::Path;

use crate::storage::Repo;

/// Syncs `connected_platforms` in each active repo's `.postlane/config.json`.
/// Called once from `setup_app` after repos are loaded.
/// Per-repo errors are logged but do not abort the startup sequence.
///
/// `is_mastodon_active`: closure called with the repo's `project_id` — returns
/// true if a Mastodon instance credential exists in the keyring.
///
/// `has_keyring_key`: closure called with a keyring key (e.g. `"zernio/{project_id}"`)
/// — returns true if that credential exists. Scheduler credentials are stored
/// at project_id scope (not repo UUID scope), so the project_id from
/// `config.json` is used when available.
pub(crate) fn sync_all_repos_on_startup(
    repos: &[Repo],
    is_mastodon_active: &dyn Fn(&str) -> bool,
    has_keyring_key: &dyn Fn(&str) -> bool,
) {
    for repo in repos.iter().filter(|r| r.active) {
        let config_path = Path::new(&repo.path).join(".postlane").join("config.json");
        let project_id = crate::config_paths::read_project_id_from_config(&config_path);
        let mastodon_active = project_id.as_deref()
            .map(is_mastodon_active)
            .unwrap_or(false);
        let effective_id = project_id.as_deref().unwrap_or(&repo.id);
        if let Err(e) = crate::project_config_ops::sync_connected_platforms_to_config_impl(
            &config_path,
            effective_id,
            mastodon_active,
            has_keyring_key,
        ) {
            log::warn!("[startup] failed to sync platforms for repo {}: {}", repo.id, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_repo(id: &str, path: &str, active: bool) -> Repo {
        Repo {
            id: id.to_string(),
            name: "test".to_string(),
            path: path.to_string(),
            active,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn write_config(dir: &Path, json: &str) {
        let config_dir = dir.join(".postlane");
        std::fs::create_dir_all(&config_dir).expect("create .postlane");
        std::fs::write(config_dir.join("config.json"), json).expect("write config.json");
    }

    fn read_platforms(dir: &Path) -> Option<Vec<String>> {
        let content = std::fs::read_to_string(dir.join(".postlane/config.json")).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        let arr = v["connected_platforms"].as_array()?;
        Some(arr.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
    }

    #[test]
    fn test_startup_sync_writes_platforms_for_all_active_repos() {
        let dir1 = tempfile::TempDir::new().unwrap();
        let dir2 = tempfile::TempDir::new().unwrap();
        write_config(
            dir1.path(),
            r#"{"version":1,"project_id":"proj-1","scheduler":{"account_ids":{"bluesky":"b1"}}}"#,
        );
        write_config(
            dir2.path(),
            r#"{"version":1,"project_id":"proj-2","scheduler":{"account_ids":{"x":"x1"}}}"#,
        );

        let repos = vec![
            make_repo("r1", dir1.path().to_str().unwrap(), true),
            make_repo("r2", dir2.path().to_str().unwrap(), true),
        ];

        // Credentials are stored at project_id scope, not repo UUID scope
        sync_all_repos_on_startup(&repos, &|_| false, &|key| key == "zernio/proj-1" || key == "zernio/proj-2");

        let p1 = read_platforms(dir1.path()).expect("repo 1 connected_platforms must be written");
        let p2 = read_platforms(dir2.path()).expect("repo 2 connected_platforms must be written");
        assert!(p1.iter().any(|v| v == "bluesky"), "repo 1 must have bluesky");
        assert!(p2.iter().any(|v| v == "x"), "repo 2 must have x");
    }

    #[test]
    fn test_startup_sync_skips_inactive_repos() {
        let active_dir = tempfile::TempDir::new().unwrap();
        let inactive_dir = tempfile::TempDir::new().unwrap();
        write_config(
            active_dir.path(),
            r#"{"version":1,"scheduler":{"account_ids":{"bluesky":"b1"}}}"#,
        );
        write_config(
            inactive_dir.path(),
            r#"{"version":1,"scheduler":{"account_ids":{"x":"x1"}}}"#,
        );

        let repos = vec![
            make_repo("r1", active_dir.path().to_str().unwrap(), true),
            make_repo("r2", inactive_dir.path().to_str().unwrap(), false),
        ];

        sync_all_repos_on_startup(&repos, &|_| false, &|key| key == "zernio/r1" || key == "zernio/r2");

        let active_cfg: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(active_dir.path().join(".postlane/config.json")).unwrap(),
        )
        .unwrap();
        let inactive_cfg: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(inactive_dir.path().join(".postlane/config.json")).unwrap(),
        )
        .unwrap();

        assert!(active_cfg.get("connected_platforms").is_some(), "active repo must be synced");
        assert!(
            inactive_cfg.get("connected_platforms").is_none(),
            "inactive repo must not be touched"
        );
    }

    #[test]
    fn test_startup_sync_continues_when_one_repo_has_no_config() {
        let dir1 = tempfile::TempDir::new().unwrap();
        let dir2 = tempfile::TempDir::new().unwrap();
        // dir1 intentionally has no config.json — sync must log a warning and continue
        write_config(dir2.path(), r#"{"version":1,"scheduler":{"account_ids":{"bluesky":"b1"}}}"#);

        let repos = vec![
            make_repo("r1", dir1.path().to_str().unwrap(), true),
            make_repo("r2", dir2.path().to_str().unwrap(), true),
        ];

        sync_all_repos_on_startup(&repos, &|_| false, &|key| key == "zernio/r2");

        let p2 = read_platforms(dir2.path()).expect("repo 2 must be synced despite repo 1 missing config");
        assert!(p2.iter().any(|v| v == "bluesky"), "repo 2 must have bluesky");
    }

    #[test]
    fn test_startup_sync_uses_project_id_not_repo_id_for_scheduler_credentials() {
        let dir = tempfile::TempDir::new().unwrap();
        // Repo UUID is "uuid-r1"; project_id in config is "proj-abc"
        write_config(
            dir.path(),
            r#"{"version":1,"project_id":"proj-abc","scheduler":{"account_ids":{"x":"h1"}}}"#,
        );
        let repos = vec![make_repo("uuid-r1", dir.path().to_str().unwrap(), true)];
        // Credential is stored at project_id scope ("zernio/proj-abc"), not repo UUID scope
        sync_all_repos_on_startup(&repos, &|_| false, &|key| key == "zernio/proj-abc");
        let platforms = read_platforms(dir.path()).expect("platforms must be written");
        assert!(
            platforms.iter().any(|v| v == "x"),
            "x must appear when scheduler credential is stored at project_id scope (not repo UUID scope)"
        );
    }

    #[test]
    fn test_startup_sync_detects_mastodon_active_via_project_id() {
        let dir = tempfile::TempDir::new().unwrap();
        write_config(dir.path(), r#"{"version":1,"project_id":"proj-1"}"#);

        let repos = vec![make_repo("r1", dir.path().to_str().unwrap(), true)];

        // Mastodon is active for project proj-1; no scheduler credentials
        sync_all_repos_on_startup(&repos, &|pid| pid == "proj-1", &|_| false);

        let platforms = read_platforms(dir.path()).expect("platforms must be written");
        assert!(
            platforms.iter().any(|v| v == "mastodon"),
            "mastodon must appear when is_mastodon_active returns true for project"
        );
    }
}
