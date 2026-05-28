// SPDX-License-Identifier: BUSL-1.1
//! One-time migration: copies bare `"{provider}"` keyring keys to project-scoped
//! `"{provider}/{project_id}"` keys, then deletes the originals.
//!
//! Idempotency: guarded by `credential_migration_v1` in `app_state.json`.
//! Concurrency: `tauri-plugin-single-instance` enforces single-instance; concurrent
//! launches are impossible, so no additional file lock is needed.

/// Immutable context passed into `run_v1_impl` to reduce argument count.
pub struct MigrationContext<'a> {
    pub already_migrated: bool,
    pub lock_acquired: bool,
    pub providers: &'a [&'a str],
}

/// Core migration logic with injected I/O operations for testability.
///
/// - `ctx.already_migrated`: if true, returns `Ok(false)` immediately.
/// - `ctx.lock_acquired`: false if another process holds the advisory lock.
/// - `get_bare_key(provider)`: returns the bare credential value, or `None` if absent.
/// - `get_projects_with_provider(provider)`: project IDs whose `config.local.json` lists this provider.
/// - `has_scoped_key(provider, project_id)`: true if `"{provider}/{project_id}"` already exists.
/// - `write_scoped_key(provider, project_id, value)`: writes `"{provider}/{project_id}"`.
/// - `delete_bare_key(provider)`: deletes the bare `"{provider}"` key.
///
/// Returns `Ok(true)` if migration ran; `Ok(false)` if skipped.
pub fn run_v1_impl<BareKey, ProjectsFor, HasScoped, WriteScoped, DeleteBare>(
    ctx: MigrationContext<'_>,
    get_bare_key: BareKey,
    get_projects_with_provider: ProjectsFor,
    has_scoped_key: HasScoped,
    write_scoped_key: WriteScoped,
    delete_bare_key: DeleteBare,
) -> Result<bool, String>
where
    BareKey: Fn(&str) -> Option<String>,
    ProjectsFor: Fn(&str) -> Vec<String>,
    HasScoped: Fn(&str, &str) -> bool,
    WriteScoped: Fn(&str, &str, &str) -> Result<(), String>,
    DeleteBare: Fn(&str) -> Result<(), String>,
{
    if ctx.already_migrated || !ctx.lock_acquired {
        return Ok(false);
    }
    for &provider in ctx.providers {
        let Some(bare_value) = get_bare_key(provider) else {
            continue;
        };
        let project_ids = get_projects_with_provider(provider);
        for project_id in &project_ids {
            if has_scoped_key(provider, project_id) {
                continue;
            }
            write_scoped_key(provider, project_id, &bare_value)?;
        }
        delete_bare_key(provider)?;
    }
    Ok(true)
}

fn has_provider_in_local_config(repo_path: &std::path::Path, provider: &str) -> bool {
    let config_path = repo_path.join(".postlane/config.local.json");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return false;
    };
    let Ok(config) = serde_json::from_str::<serde_json::Value>(&content) else {
        return false;
    };
    if config["scheduler"]["provider"].as_str() == Some(provider) {
        return true;
    }
    if let Some(arr) = config["scheduler"]["fallback_order"].as_array() {
        return arr.iter().any(|v| v.as_str() == Some(provider));
    }
    false
}

fn read_project_id_from_repo(repo_path: &std::path::Path) -> Option<String> {
    let config_path = repo_path.join(".postlane/config.json");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    config["project_id"].as_str().map(str::to_string)
}

fn get_project_ids_for_provider(
    provider: &str,
    state: &crate::app_state::AppState,
) -> Vec<String> {
    let repos = match state.repos.lock() {
        Ok(g) => g.clone(),
        Err(_) => return vec![],
    };
    let mut project_ids: Vec<String> = Vec::new();
    for repo in &repos.repos {
        let repo_path = std::path::PathBuf::from(&repo.path);
        if !has_provider_in_local_config(&repo_path, provider) {
            continue;
        }
        if let Some(pid) = read_project_id_from_repo(&repo_path) {
            if !project_ids.contains(&pid) {
                project_ids.push(pid);
            }
        }
    }
    project_ids
}

/// Runs the v1 credential migration against the real keyring.
///
/// Single-instance is enforced by `tauri-plugin-single-instance`, so no
/// additional file lock is needed. Writes `credential_migration_v1 = true`
/// to `app_state.json` only after all copies succeed. Migration runs silently.
pub fn run_v1(
    app: &tauri::AppHandle,
    state: &crate::app_state::AppState,
) -> Result<bool, String> {
    use tauri_plugin_keyring::KeyringExt;

    let current = crate::app_state::read_app_state();
    let app_handle = app.clone();

    let result = run_v1_impl(
        MigrationContext {
            already_migrated: current.credential_migration_v1,
            // Single-instance enforced; concurrent launches are impossible.
            lock_acquired: true,
            providers: &crate::scheduler_credentials::VALID_PROVIDERS,
        },
        |provider| match app_handle.keyring().get_password("postlane", provider) {
            Ok(Some(v)) => Some(v),
            _ => None,
        },
        |provider| get_project_ids_for_provider(provider, state),
        |provider, project_id| {
            let key = crate::scheduler_credentials::get_credential_keyring_key(provider, project_id);
            matches!(app_handle.keyring().get_password("postlane", &key), Ok(Some(_)))
        },
        |provider, project_id, value| {
            let key = crate::scheduler_credentials::get_credential_keyring_key(provider, project_id);
            app_handle
                .keyring()
                .set_password("postlane", &key, value)
                .map_err(|e| format!("Failed to write scoped credential {}: {}", key, e))
        },
        |provider| {
            app_handle
                .keyring()
                .delete_password("postlane", provider)
                .map_err(|e| format!("Failed to delete bare credential {}: {}", provider, e))
        },
    );

    let ran = result?;
    if ran {
        let mut updated = crate::app_state::read_app_state();
        updated.credential_migration_v1 = true;
        if let Err(e) = crate::app_state::write_app_state(&updated) {
            log::warn!("[credential_migration] failed to write migration flag: {}", e);
        }
    }

    Ok(ran)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    fn write_json(path: &std::path::Path, json: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(json.as_bytes()).unwrap();
    }

    // has_provider_in_local_config — provider matches scheduler.provider
    #[test]
    fn test_has_provider_in_local_config_matches_primary() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.local.json");
        write_json(&cfg_path, r#"{"scheduler":{"provider":"zernio"}}"#);
        assert!(has_provider_in_local_config(dir.path(), "zernio"));
        assert!(!has_provider_in_local_config(dir.path(), "outstand"));
    }

    // has_provider_in_local_config — provider is in fallback_order array
    #[test]
    fn test_has_provider_in_local_config_matches_fallback_order() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.local.json");
        write_json(&cfg_path, r#"{"scheduler":{"provider":"zernio","fallback_order":["outstand","buffer"]}}"#);
        assert!(has_provider_in_local_config(dir.path(), "outstand"));
        assert!(has_provider_in_local_config(dir.path(), "buffer"));
        assert!(!has_provider_in_local_config(dir.path(), "ayrshare"));
    }

    // has_provider_in_local_config — file does not exist → false
    #[test]
    fn test_has_provider_in_local_config_returns_false_when_file_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(!has_provider_in_local_config(dir.path(), "zernio"));
    }

    // has_provider_in_local_config — file is invalid JSON → false
    #[test]
    fn test_has_provider_in_local_config_returns_false_on_bad_json() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.local.json");
        write_json(&cfg_path, "not json");
        assert!(!has_provider_in_local_config(dir.path(), "zernio"));
    }

    // read_project_id_from_repo — returns Some when project_id present
    #[test]
    fn test_read_project_id_from_repo_returns_id_when_present() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.json");
        write_json(&cfg_path, r#"{"project_id":"proj-abc"}"#);
        assert_eq!(
            read_project_id_from_repo(dir.path()),
            Some("proj-abc".to_string())
        );
    }

    // read_project_id_from_repo — file absent → None
    #[test]
    fn test_read_project_id_from_repo_returns_none_when_file_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        assert_eq!(read_project_id_from_repo(dir.path()), None);
    }

    // read_project_id_from_repo — project_id field missing → None
    #[test]
    fn test_read_project_id_from_repo_returns_none_when_field_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.json");
        write_json(&cfg_path, r#"{"some_other_field":"value"}"#);
        assert_eq!(read_project_id_from_repo(dir.path()), None);
    }

    // read_project_id_from_repo — invalid JSON → None
    #[test]
    fn test_read_project_id_from_repo_returns_none_on_bad_json() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg_path = dir.path().join(".postlane/config.json");
        write_json(&cfg_path, "{ bad json }");
        assert_eq!(read_project_id_from_repo(dir.path()), None);
    }

    // get_project_ids_for_provider — empty repos → empty vec
    #[test]
    fn test_get_project_ids_for_provider_returns_empty_when_no_repos() {
        use crate::app_state::AppState;
        use crate::storage::ReposConfig;
        let dir = tempfile::TempDir::new().unwrap();
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![] },
            dir.path().join("repos.json"),
        );
        assert!(get_project_ids_for_provider("zernio", &state).is_empty());
    }

    // get_project_ids_for_provider — repo with matching provider + project_id → returned
    #[test]
    fn test_get_project_ids_for_provider_returns_id_for_matching_repo() {
        use crate::app_state::AppState;
        use crate::storage::{Repo, ReposConfig};
        let dir = tempfile::TempDir::new().unwrap();
        let repo_dir = dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        write_json(&repo_dir.join(".postlane/config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#);
        write_json(&repo_dir.join(".postlane/config.json"), r#"{"project_id":"proj-111"}"#);
        let repo = Repo {
            id: "r1".into(), name: "my-repo".into(),
            path: repo_dir.to_str().unwrap().to_string(),
            active: true, added_at: "2026-01-01T00:00:00Z".into(),
        };
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![repo] },
            dir.path().join("repos.json"),
        );
        let ids = get_project_ids_for_provider("zernio", &state);
        assert_eq!(ids, vec!["proj-111"]);
    }

    // get_project_ids_for_provider — repo with wrong provider → skipped
    #[test]
    fn test_get_project_ids_for_provider_skips_non_matching_repos() {
        use crate::app_state::AppState;
        use crate::storage::{Repo, ReposConfig};
        let dir = tempfile::TempDir::new().unwrap();
        let repo_dir = dir.path().join("my-repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        write_json(&repo_dir.join(".postlane/config.local.json"), r#"{"scheduler":{"provider":"buffer"}}"#);
        write_json(&repo_dir.join(".postlane/config.json"), r#"{"project_id":"proj-222"}"#);
        let repo = Repo {
            id: "r1".into(), name: "my-repo".into(),
            path: repo_dir.to_str().unwrap().to_string(),
            active: true, added_at: "2026-01-01T00:00:00Z".into(),
        };
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos: vec![repo] },
            dir.path().join("repos.json"),
        );
        let ids = get_project_ids_for_provider("zernio", &state);
        assert!(ids.is_empty(), "non-matching provider must be skipped");
    }

    // get_project_ids_for_provider — same project_id in two repos → deduplicated
    #[test]
    fn test_get_project_ids_for_provider_deduplicates_project_ids() {
        use crate::app_state::AppState;
        use crate::storage::{Repo, ReposConfig};
        let dir = tempfile::TempDir::new().unwrap();
        let make_repo_dir = |name: &str| -> std::path::PathBuf {
            let repo_dir = dir.path().join(name);
            std::fs::create_dir_all(&repo_dir).unwrap();
            write_json(&repo_dir.join(".postlane/config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#);
            write_json(&repo_dir.join(".postlane/config.json"), r#"{"project_id":"proj-shared"}"#);
            repo_dir
        };
        let rd1 = make_repo_dir("repo-a");
        let rd2 = make_repo_dir("repo-b");
        let repos = vec![
            Repo { id: "r1".into(), name: "a".into(), path: rd1.to_str().unwrap().into(), active: true, added_at: "2026-01-01T00:00:00Z".into() },
            Repo { id: "r2".into(), name: "b".into(), path: rd2.to_str().unwrap().into(), active: true, added_at: "2026-01-01T00:00:00Z".into() },
        ];
        let state = AppState::new_with_path(
            ReposConfig { version: 1, workspaces: vec![], repos },
            dir.path().join("repos.json"),
        );
        let ids = get_project_ids_for_provider("zernio", &state);
        assert_eq!(ids.len(), 1, "same project_id in two repos must be deduplicated");
        assert_eq!(ids[0], "proj-shared");
    }

    // 21.5.12 — migration already flagged → skipped entirely; no keyring changes
    #[test]
    fn test_migration_skipped_when_already_flagged() {
        let write_called = std::cell::Cell::new(false);
        let result = run_v1_impl(
            MigrationContext { already_migrated: true, lock_acquired: true, providers: &["zernio"] },
            |_| Some("secret".to_string()),
            |_| vec!["proj-1".to_string()],
            |_, _| false,
            |_, _, _| {
                write_called.set(true);
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
        assert!(!result, "must return false when already migrated");
        assert!(!write_called.get(), "must not write any scoped key when already migrated");
    }

    // 21.5.13 — lock not acquired → skipped entirely; keyring not modified
    #[test]
    fn test_migration_skipped_when_lock_not_acquired() {
        let write_called = std::cell::Cell::new(false);
        let result = run_v1_impl(
            MigrationContext { already_migrated: false, lock_acquired: false, providers: &["zernio"] },
            |_| Some("secret".to_string()),
            |_| vec!["proj-1".to_string()],
            |_, _| false,
            |_, _, _| {
                write_called.set(true);
                Ok(())
            },
            |_| Ok(()),
        )
        .unwrap();
        assert!(!result, "must return false when lock not acquired");
        assert!(!write_called.get(), "must not write any scoped key when lock not acquired");
    }

    // 21.5.10 — bare key copied to matching project only; bare key deleted after copy
    #[test]
    fn test_migration_copies_bare_key_to_matching_project_only() {
        let written: std::cell::RefCell<Vec<(String, String, String)>> =
            std::cell::RefCell::new(vec![]);
        let deleted = std::cell::Cell::new(false);

        let result = run_v1_impl(
            MigrationContext { already_migrated: false, lock_acquired: true, providers: &["zernio"] },
            |p| {
                if p == "zernio" { Some("zernio-secret".to_string()) } else { None }
            },
            // proj-a has zernio; proj-b does not (not returned for zernio)
            |p| {
                if p == "zernio" { vec!["proj-a".to_string()] } else { vec![] }
            },
            |_, _| false,
            |prov, proj, val| {
                written
                    .borrow_mut()
                    .push((prov.to_string(), proj.to_string(), val.to_string()));
                Ok(())
            },
            |_| {
                deleted.set(true);
                Ok(())
            },
        )
        .unwrap();

        assert!(result, "migration must have run");
        let keys = written.borrow();
        assert_eq!(keys.len(), 1, "only one project should receive the key");
        assert_eq!(keys[0].0, "zernio");
        assert_eq!(keys[0].1, "proj-a");
        assert_eq!(keys[0].2, "zernio-secret");
        assert!(deleted.get(), "bare key must be deleted after copying");
    }

    // 21.5.11 — scoped key already present → no copy; bare key still cleaned up
    #[test]
    fn test_migration_skips_copy_when_scoped_key_exists() {
        let write_called = std::cell::Cell::new(false);
        let delete_called = std::cell::Cell::new(false);

        let result = run_v1_impl(
            MigrationContext { already_migrated: false, lock_acquired: true, providers: &["zernio"] },
            |_| Some("zernio-secret".to_string()), // bare key exists
            |_| vec!["proj-1".to_string()],
            |_, _| true, // scoped key already exists
            |_, _, _| {
                write_called.set(true);
                Ok(())
            },
            |_| {
                delete_called.set(true);
                Ok(())
            },
        )
        .unwrap();

        assert!(result, "migration must have run");
        assert!(!write_called.get(), "must not overwrite existing scoped key");
        assert!(
            delete_called.get(),
            "bare key must still be deleted when scoped key already exists"
        );
    }

    // 21.5.10 edge case — provider with no bare key is skipped; delete not called
    #[test]
    fn test_migration_skips_provider_with_no_bare_key() {
        let delete_called = std::cell::Cell::new(false);

        let result = run_v1_impl(
            MigrationContext { already_migrated: false, lock_acquired: true, providers: &["zernio"] },
            |_| None, // no bare key
            |_| vec!["proj-1".to_string()],
            |_, _| false,
            |_, _, _| Ok(()),
            |_| {
                delete_called.set(true);
                Ok(())
            },
        )
        .unwrap();

        assert!(result, "migration ran (nothing to migrate)");
        assert!(!delete_called.get(), "delete must not be called when no bare key exists");
    }
}
