// SPDX-License-Identifier: BUSL-1.1

//! `~/.postlane/repos.json` schema migration: v1 → v2.
//!
//! v1 schema: `{ "version": 1, "repos": [...] }`
//! v2 schema: `{ "version": 2, "workspaces": [], "repos": [...] }`
//!
//! Call `migrate_repos_to_v2` once at app startup before `read_repos_with_recovery`.
//! After a successful rewrite the `repos_schema_v2: true` idempotency flag is written
//! to `app_state.json` so the migration is skipped on subsequent launches.

use crate::storage::{read_repos_with_recovery, write_repos, ReposConfig, REPOS_CONFIG_VERSION};
use std::path::Path;

/// Rewrites `repos_path` from v1 → v2 schema atomically if the file is v1.
/// If the file is already v2, or absent, this is a no-op.
///
/// On a successful rewrite, writes `repos_schema_v2: true` to `app_state_path`
/// so the migration is skipped on all subsequent launches.
pub fn migrate_repos_to_v2(repos_path: &Path, app_state_path: &Path) -> Result<(), String> {
    if !repos_path.exists() {
        return Ok(());
    }

    let config = read_repos_with_recovery(repos_path)
        .map_err(|_| "failed to read repos.json for migration".to_string())?;

    if config.version == REPOS_CONFIG_VERSION {
        return Ok(());
    }

    // version 1 — rewrite as v2 preserving the repos array, adding empty workspaces
    let migrated = ReposConfig {
        version: REPOS_CONFIG_VERSION,
        workspaces: vec![],
        repos: config.repos,
    };
    write_repos(repos_path, &migrated)
        .map_err(|_| "failed to write migrated repos.json".to_string())?;

    // Write idempotency flag to app_state.json — read-merge-write to preserve other fields
    set_repos_schema_v2_flag(app_state_path)?;

    log::info!("[repos_migration] repos.json migrated from v1 to v2");
    Ok(())
}

/// Reads `app_state_path`, sets `repos_schema_v2: true`, and writes back atomically.
/// Creates the file if absent (using defaults for all other fields).
fn set_repos_schema_v2_flag(app_state_path: &Path) -> Result<(), String> {
    let mut state = if app_state_path.exists() {
        crate::init::read_json_file::<serde_json::Value>(app_state_path)?
    } else {
        let default = crate::app_state_types::AppStateFile::default();
        serde_json::to_value(&default)
            .map_err(|e| format!("failed to serialise default app state: {}", e))?
    };

    state["repos_schema_v2"] = serde_json::json!(true);

    crate::init::write_json_file(app_state_path, &state)
}

#[cfg(test)]
mod tests {
    use crate::app_state_types::AppStateFile;
    use crate::storage::{read_repos_with_recovery, write_repos, ReposConfig, Repo, REPOS_CONFIG_VERSION};
    use crate::workspace_entry::WorkspaceEntry;
    use std::fs;
    use tempfile::TempDir;

    fn write_v1_config(path: &std::path::Path, repos: Vec<Repo>) {
        let v1 = serde_json::json!({
            "version": 1,
            "repos": repos.iter().map(|r| serde_json::json!({
                "id": r.id,
                "name": r.name,
                "path": r.path,
                "active": r.active,
                "added_at": r.added_at,
            })).collect::<Vec<_>>()
        });
        fs::write(path, serde_json::to_string_pretty(&v1).unwrap()).unwrap();
    }

    /// 22.1.11 — v1 repos.json on launch → rewritten as v2 with empty workspaces;
    /// existing repos array entries preserved verbatim; repos_schema_v2: true written
    /// to app_state.json.
    #[test]
    fn test_v1_repos_json_migrated_to_v2_on_launch() {
        let dir = TempDir::new().unwrap();
        let repos_path = dir.path().join("repos.json");
        let app_state_path = dir.path().join("app_state.json");

        let original_repo = Repo {
            id: "r-1".to_string(),
            name: "frontend".to_string(),
            path: "/code/org/frontend".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        };
        write_v1_config(&repos_path, vec![original_repo.clone()]);

        // pre-condition: file is v1
        let before = serde_json::from_str::<serde_json::Value>(
            &fs::read_to_string(&repos_path).unwrap()
        ).unwrap();
        assert_eq!(before["version"], 1);

        super::migrate_repos_to_v2(&repos_path, &app_state_path)
            .expect("migration must succeed");

        // post-condition: file is v2
        let config = read_repos_with_recovery(&repos_path).expect("read v2");
        assert_eq!(config.version, REPOS_CONFIG_VERSION, "version must be 2 after migration");
        assert!(config.workspaces.is_empty(), "workspaces must be empty on first migration");
        assert_eq!(config.repos.len(), 1, "repos array must be preserved");
        assert_eq!(config.repos[0].id, "r-1");
        assert_eq!(config.repos[0].name, "frontend");

        // post-condition: app_state flag written
        let raw = fs::read_to_string(&app_state_path).expect("app_state.json written");
        let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            state["repos_schema_v2"],
            serde_json::json!(true),
            "repos_schema_v2 flag must be true in app_state.json after migration"
        );
    }

    /// 22.1.12 — v2 repos.json on launch → no migration runs; flag unchanged.
    #[test]
    fn test_v2_repos_json_skips_migration() {
        let dir = TempDir::new().unwrap();
        let repos_path = dir.path().join("repos.json");
        let app_state_path = dir.path().join("app_state.json");

        let config_v2 = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: "ws-1".to_string(),
                name: "myorg".to_string(),
                workspace_path: "/code/myorg".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        write_repos(&repos_path, &config_v2).expect("write v2");

        // Write an existing app_state that already has repos_schema_v2: true
        let existing_state = AppStateFile {
            repos_schema_v2: true,
            ..AppStateFile::default()
        };
        let state_json = serde_json::to_string_pretty(&existing_state).unwrap();
        fs::write(&app_state_path, state_json).unwrap();

        super::migrate_repos_to_v2(&repos_path, &app_state_path)
            .expect("noop migration must succeed");

        // repos.json unchanged
        let after = read_repos_with_recovery(&repos_path).expect("read");
        assert_eq!(after.version, 2);
        assert_eq!(after.workspaces.len(), 1, "workspaces must be untouched");

        // app_state flag not changed
        let raw = fs::read_to_string(&app_state_path).unwrap();
        let state: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(state["repos_schema_v2"], serde_json::json!(true));
    }

    /// Migration is idempotent: running twice on a v1 file only migrates once.
    #[test]
    fn test_migration_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let repos_path = dir.path().join("repos.json");
        let app_state_path = dir.path().join("app_state.json");

        write_v1_config(&repos_path, vec![]);

        super::migrate_repos_to_v2(&repos_path, &app_state_path).unwrap();
        super::migrate_repos_to_v2(&repos_path, &app_state_path).unwrap();

        let config = read_repos_with_recovery(&repos_path).unwrap();
        assert_eq!(config.version, REPOS_CONFIG_VERSION);
    }

    /// Missing repos.json is a no-op (no error, no file created).
    #[test]
    fn test_migration_noop_when_repos_json_absent() {
        let dir = TempDir::new().unwrap();
        let repos_path = dir.path().join("repos.json");
        let app_state_path = dir.path().join("app_state.json");

        // File doesn't exist
        super::migrate_repos_to_v2(&repos_path, &app_state_path)
            .expect("absent file must be a no-op");

        assert!(!repos_path.exists(), "repos.json must not be created when absent");
    }
}
