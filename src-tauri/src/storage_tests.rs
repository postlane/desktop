// SPDX-License-Identifier: BUSL-1.1
// Tests for storage.rs — extracted to keep storage.rs under 400 lines.

use super::*;
use crate::workspace_entry::WorkspaceEntry;
use std::fs;

#[test]
fn test_read_repos_missing_file_returns_empty() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let result = read_repos_with_recovery(&repos_path).expect("Should return empty config");
    assert_eq!(result.version, REPOS_CONFIG_VERSION);
    assert_eq!(result.repos.len(), 0);
}

#[test]
fn test_read_repos_malformed_json_creates_backup() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let bak_path = dir.path().join("repos.json.bak");
    fs::write(&repos_path, "{ this is not valid json }").expect("Failed to write malformed JSON");

    let result = read_repos_with_recovery(&repos_path).expect("Should recover from corruption");
    assert_eq!(result.version, REPOS_CONFIG_VERSION);
    assert_eq!(result.repos.len(), 0, "Should return empty repos list");
    assert!(bak_path.exists(), "Backup file should exist");
    assert!(!repos_path.exists(), "Original should be renamed");
}

#[test]
fn test_read_repos_valid_json_parses_correctly() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let config = ReposConfig {
        version: 1,
        workspaces: vec![],
        repos: vec![Repo {
            id: "test-id".to_string(),
            name: "Test Repo".to_string(),
            path: "/path/to/repo".to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }],
    };
    let json = serde_json::to_string_pretty(&config).expect("Failed to serialize");
    fs::write(&repos_path, json).expect("Failed to write JSON");

    let result = read_repos_with_recovery(&repos_path).expect("Should parse valid JSON");
    assert_eq!(result.version, 1);
    assert_eq!(result.repos.len(), 1);
    assert_eq!(result.repos[0].id, "test-id");
}

#[test]
fn test_read_repos_version_mismatch_returns_error() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let json = r#"{"version": 999, "repos": []}"#;
    fs::write(&repos_path, json).expect("Failed to write JSON");

    let result = read_repos_with_recovery(&repos_path);
    assert!(result.is_err(), "Should return error on version mismatch");

    match result {
        Err(StorageError::VersionMismatch { found, expected }) => {
            assert_eq!(found, 999);
            assert_eq!(expected, REPOS_CONFIG_VERSION);
        }
        _ => panic!("Expected VersionMismatch error"),
    }
}

#[test]
fn test_repos_round_trip_with_version() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let config = ReposConfig {
        version: 1,
        workspaces: vec![],
        repos: vec![
            Repo {
                id: "repo1-id".to_string(),
                name: "Repo One".to_string(),
                path: "/path/to/repo1".to_string(),
                active: true,
                added_at: "2024-01-01T00:00:00Z".to_string(),
            },
            Repo {
                id: "repo2-id".to_string(),
                name: "Repo Two".to_string(),
                path: "/path/to/repo2".to_string(),
                active: false,
                added_at: "2024-01-02T00:00:00Z".to_string(),
            },
        ],
    };
    write_repos(&repos_path, &config).expect("Failed to write repos");
    let loaded = read_repos_with_recovery(&repos_path).expect("Failed to read repos");
    assert_eq!(loaded.version, 1, "Version should be preserved");
    assert_eq!(loaded.repos.len(), 2, "Should have 2 repos");
    assert_eq!(loaded.repos[0].id, "repo1-id");
    assert_eq!(loaded.repos[0].name, "Repo One");
    assert!(loaded.repos[0].active);
    assert_eq!(loaded.repos[1].id, "repo2-id");
    assert!(!loaded.repos[1].active);
}

#[test]
fn test_concurrent_write_protection() {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = Arc::new(dir.path().join("repos.json"));
    let write_lock = Arc::new(Mutex::new(()));
    let mut handles = vec![];

    for i in 0..5 {
        let path = Arc::clone(&repos_path);
        let lock = Arc::clone(&write_lock);
        let handle = thread::spawn(move || {
            let _guard = lock.lock().unwrap();
            let config = ReposConfig {
                version: 1,
                workspaces: vec![],
                repos: vec![Repo {
                    id: format!("repo-{}", i),
                    name: format!("Repo {}", i),
                    path: format!("/path/to/repo{}", i),
                    active: true,
                    added_at: "2024-01-01T00:00:00Z".to_string(),
                }],
            };
            write_repos(&path, &config).expect("Write failed");
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
    let final_state = read_repos_with_recovery(&repos_path).expect("Failed to read");
    assert_eq!(final_state.version, 1);
    assert_eq!(final_state.repos.len(), 1);
}

#[test]
fn test_storage_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let storage_err: StorageError = io_err.into();
    match storage_err {
        StorageError::IoError(_) => {}
        _ => panic!("Expected IoError variant"),
    }
}

#[test]
fn test_storage_error_from_json_error() {
    let json_result: Result<ReposConfig, serde_json::Error> =
        serde_json::from_str("{ invalid json }");
    let json_err = json_result.unwrap_err();
    let storage_err: StorageError = json_err.into();
    match storage_err {
        StorageError::ParseError(_) => {}
        _ => panic!("Expected ParseError variant"),
    }
}

#[test]
fn test_read_repos_malformed_backup_rename_fails() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let bak_path = dir.path().join("repos.json.bak");
    fs::create_dir_all(&bak_path).expect("Failed to create bak dir");
    fs::write(&repos_path, "{ not valid json }").expect("Failed to write malformed JSON");

    let result = read_repos_with_recovery(&repos_path);
    assert!(result.is_ok(), "Should return Ok despite backup failure");
    let config = result.unwrap();
    assert_eq!(config.version, REPOS_CONFIG_VERSION);
    assert_eq!(config.repos.len(), 0, "Should return empty config");
}

// ── Corruption flag tests ──────────────────────────────────────────────────

#[test]
fn test_read_repos_returns_false_flag_for_clean_file() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let config = ReposConfig { version: 1, repos: vec![], workspaces: vec![] };
    let json = serde_json::to_string(&config).unwrap();
    fs::write(&repos_path, json).unwrap();

    let (_, was_corrupted) = read_repos_checked(&repos_path).expect("should succeed");
    assert!(!was_corrupted, "clean file must not set was_corrupted");
}

#[test]
fn test_read_repos_returns_true_flag_for_corrupted_file() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    fs::write(&repos_path, "{ bad json }").unwrap();

    let (config, was_corrupted) = read_repos_checked(&repos_path).expect("should recover");
    assert!(was_corrupted, "corrupt file must set was_corrupted");
    assert_eq!(config.repos.len(), 0, "should return empty config after recovery");
}

#[test]
fn test_read_repos_returns_false_flag_for_missing_file() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let (_, was_corrupted) = read_repos_checked(&repos_path).expect("should succeed");
    assert!(!was_corrupted, "missing file is not corruption");
}

#[test]
fn test_write_repos_preserves_previous_content_in_bak() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let make_config = |id: &str| ReposConfig {
        version: 1,
        workspaces: vec![],
        repos: vec![Repo {
            id: id.to_string(),
            name: id.to_string(),
            path: "/p".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }],
    };
    write_repos(&repos_path, &make_config("first")).expect("first write");
    write_repos(&repos_path, &make_config("second")).expect("second write");

    let bak_path = repos_path.with_extension("json.bak");
    assert!(bak_path.exists(), "repos.json.bak must exist after second write");
    let bak_content = fs::read_to_string(&bak_path).expect("read bak");
    let bak_config: ReposConfig = serde_json::from_str(&bak_content).expect("parse bak");
    assert_eq!(bak_config.repos[0].id, "first", "bak must contain the pre-second-write state");
}

#[test]
fn test_write_repos_no_bak_on_first_write() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    write_repos(&repos_path, &ReposConfig { version: 1, repos: vec![], workspaces: vec![] })
        .expect("first write");
    let bak_path = repos_path.with_extension("json.bak");
    assert!(!bak_path.exists(), "no bak should exist when there was nothing to back up");
}

// ── v2 schema (workspaces array) ──────────────────────────────────────────

/// 22.1.1 — v2 ReposConfig with workspaces array roundtrips correctly.
#[test]
fn test_repos_config_v2_with_workspaces_roundtrips() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    let config = ReposConfig {
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
    write_repos(&repos_path, &config).expect("write v2");
    let loaded = read_repos_with_recovery(&repos_path).expect("read v2");
    assert_eq!(loaded.version, 2);
    assert_eq!(loaded.workspaces.len(), 1);
    assert_eq!(loaded.workspaces[0].id, "ws-1");
}

/// 22.1.1 — v2 file without workspaces field (missing key) defaults to empty vec.
#[test]
fn test_repos_config_missing_workspaces_field_defaults_to_empty() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let repos_path = dir.path().join("repos.json");
    fs::write(&repos_path, r#"{"version":2,"repos":[]}"#).expect("write");
    let loaded = read_repos_with_recovery(&repos_path).expect("read");
    assert_eq!(loaded.version, 2);
    assert!(
        loaded.workspaces.is_empty(),
        "missing workspaces key must default to empty vec"
    );
}
