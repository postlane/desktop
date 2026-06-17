// SPDX-License-Identifier: BUSL-1.1
// Tests for config_local_write.rs — extracted to keep the main file under 400 lines.

use super::*;
use std::fs;

#[test]
fn test_write_provider_creates_local_config_when_absent() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
}

#[test]
fn test_write_provider_updates_existing_local_config_preserving_other_fields() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"profile_id":"abc","scheduler":{"provider":""}}"#)
        .expect("write initial");
    write_scheduler_provider_to_local_config(dir.path(), "publer").expect("write");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some("publer"));
    assert_eq!(v["profile_id"].as_str(), Some("abc"), "profile_id must be preserved");
}

#[test]
fn test_write_second_provider_creates_fallback_order() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"upload_post"}}"#)
        .expect("write initial");
    write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    let order = v["scheduler"]["fallback_order"].as_array().expect("fallback_order array");
    assert_eq!(order[0].as_str(), Some("upload_post"));
    assert_eq!(order[1].as_str(), Some("zernio"));
    assert!(v["scheduler"]["provider"].is_null(), "single provider field removed when fallback_order present");
}

#[test]
fn test_write_duplicate_provider_is_not_appended() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
        .expect("write initial");
    write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"), "single provider kept");
    assert!(v["scheduler"]["fallback_order"].is_null(), "no fallback_order for single provider");
}

#[test]
fn test_remove_only_provider_clears_to_empty() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
        .expect("write initial");
    remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some(""), "provider must be cleared");
}

#[test]
fn test_remove_provider_from_fallback_downgrades_to_single() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"fallback_order":["zernio","buffer"]}}"#)
        .expect("write initial");
    remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some("buffer"));
    assert!(v["scheduler"]["fallback_order"].is_null());
}

#[test]
fn test_remove_provider_from_three_keeps_fallback_order() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"fallback_order":["zernio","buffer","publer"]}}"#)
        .expect("write initial");
    remove_scheduler_provider_from_local_config(dir.path(), "buffer").expect("remove");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    let order = v["scheduler"]["fallback_order"].as_array().expect("fallback_order");
    assert_eq!(order.len(), 2);
    assert_eq!(order[0].as_str(), Some("zernio"));
    assert_eq!(order[1].as_str(), Some("publer"));
}

#[test]
fn test_remove_nonexistent_provider_is_noop() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
        .expect("write initial");
    remove_scheduler_provider_from_local_config(dir.path(), "buffer").expect("remove");
    let written = fs::read_to_string(postlane.join("config.local.json")).expect("read");
    let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
    assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
}

#[test]
fn test_remove_from_missing_file_is_ok() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).expect("create .postlane");
    let result = remove_scheduler_provider_from_local_config(dir.path(), "zernio");
    assert!(result.is_ok());
}

// ── 22.1 workspace config.local.json ──────────────────────────────────────

/// 22.1.16 — workspace-root config.local.json found and used when present.
#[test]
fn test_resolve_local_config_path_returns_workspace_root_when_present() {
    let ws = tempfile::TempDir::new().unwrap();
    let child = ws.path().join("repo-a");
    fs::create_dir_all(&child).unwrap();
    fs::write(ws.path().join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#).unwrap();

    let path = super::resolve_local_config_path(&child, ws.path());
    assert_eq!(path, ws.path().join("config.local.json"), "workspace root config.local.json must be preferred");
}

/// 22.1.16 — per-repo fallback used when workspace-root file absent.
#[test]
fn test_resolve_local_config_path_falls_back_to_per_repo() {
    let ws = tempfile::TempDir::new().unwrap();
    let child = ws.path().join("repo-a");
    let postlane = child.join(".postlane");
    fs::create_dir_all(&postlane).unwrap();
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"buffer"}}"#).unwrap();

    let path = super::resolve_local_config_path(&child, ws.path());
    assert_eq!(path, postlane.join("config.local.json"), "must fall back to per-repo config.local.json");
}

/// 22.1.6 — .gitignore at workspace root gets config.local.json entry appended.
#[test]
fn test_append_config_local_to_gitignore_creates_entry() {
    let ws = tempfile::TempDir::new().unwrap();
    super::append_config_local_to_gitignore(ws.path()).expect("append");
    let content = fs::read_to_string(ws.path().join(".gitignore")).expect("read .gitignore");
    assert!(content.contains("config.local.json"), ".gitignore must contain config.local.json");
}

/// 22.1.6 — append is idempotent; running twice does not duplicate the entry.
#[test]
fn test_append_config_local_to_gitignore_is_idempotent() {
    let ws = tempfile::TempDir::new().unwrap();
    super::append_config_local_to_gitignore(ws.path()).expect("first append");
    super::append_config_local_to_gitignore(ws.path()).expect("second append");
    let content = fs::read_to_string(ws.path().join(".gitignore")).expect("read .gitignore");
    let count = content.matches("config.local.json").count();
    assert_eq!(count, 1, "config.local.json must appear exactly once after two appends");
}

/// 22.1.6 — pre-existing .gitignore content is not truncated.
#[test]
fn test_append_config_local_to_gitignore_preserves_existing_content() {
    let ws = tempfile::TempDir::new().unwrap();
    fs::write(ws.path().join(".gitignore"), "node_modules/\n.env\n").unwrap();
    super::append_config_local_to_gitignore(ws.path()).expect("append");
    let content = fs::read_to_string(ws.path().join(".gitignore")).expect("read .gitignore");
    assert!(content.contains("node_modules/"), "existing content must be preserved");
    assert!(content.contains(".env"), "existing content must be preserved");
    assert!(content.contains("config.local.json"), "new entry must be present");
}

/// 22.1.6a / 22.1.6b — workspace config.local.json created with 0600 permissions on Unix.
#[cfg(unix)]
#[test]
fn test_write_workspace_local_config_uses_0600_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let ws = tempfile::TempDir::new().unwrap();
    super::write_workspace_local_config(ws.path(), r#"{"scheduler":{"provider":""}}"#)
        .expect("write");
    let path = ws.path().join("config.local.json");
    let perms = fs::metadata(&path).expect("stat").permissions();
    let mode = perms.mode() & 0o777;
    assert_eq!(mode, 0o600, "config.local.json must have 0600 permissions, got {:#o}", mode);
}

/// write_scheduler_provider_to_local_config must create config.local.json with 0600 permissions.
#[cfg(unix)]
#[test]
fn test_write_scheduler_provider_uses_0600_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::TempDir::new().unwrap();
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).unwrap();
    super::write_scheduler_provider_to_local_config(dir.path(), "zernio").expect("write");
    let path = postlane.join("config.local.json");
    let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "config.local.json must be 0600, got {:#o}", mode);
}

/// remove_scheduler_provider_from_local_config must preserve 0600 permissions when rewriting.
#[cfg(unix)]
#[test]
fn test_remove_scheduler_provider_preserves_0600_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::TempDir::new().unwrap();
    let postlane = dir.path().join(".postlane");
    fs::create_dir_all(&postlane).unwrap();
    fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":"zernio"}}"#)
        .unwrap();
    super::remove_scheduler_provider_from_local_config(dir.path(), "zernio").expect("remove");
    let path = postlane.join("config.local.json");
    let mode = fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "config.local.json must be 0600 after remove, got {:#o}", mode);
}
