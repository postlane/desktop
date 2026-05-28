// SPDX-License-Identifier: BUSL-1.1

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static VERSIONS_WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn read_versions_at(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_versions_at(path: &Path, versions: &HashMap<String, String>) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(versions)
        .map_err(|e| format!("Failed to serialize voice guide versions: {}", e))?;
    crate::init::atomic_write(path, &json).map_err(|e| {
        format!("Failed to write voice guide versions to {}: {}", path.display(), e)
    })
}

pub(crate) fn record_version_at(project_id: &str, path: &Path) -> Result<(), String> {
    let _guard = VERSIONS_WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    let mut versions = read_versions_at(path);
    versions.insert(
        project_id.to_string(),
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    );
    write_versions_at(path, &versions)
}

pub fn record_version(project_id: &str) -> Result<(), String> {
    let path = crate::init::postlane_dir()?.join("voice_guide_versions.json");
    record_version_at(project_id, &path)
}

pub(crate) fn lookup_version_at(project_id: &str, path: &Path) -> Option<String> {
    read_versions_at(path).get(project_id).cloned()
}

pub fn lookup_version(project_id: &str) -> Option<String> {
    let path = crate::init::postlane_dir().ok()?.join("voice_guide_versions.json");
    lookup_version_at(project_id, &path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_path(_name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("versions.json");
        (dir, path)
    }

    #[test]
    fn test_lookup_at_returns_none_when_file_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("nonexistent/versions.json");
        assert!(lookup_version_at("proj-1", &path).is_none());
    }

    #[test]
    fn test_record_at_writes_project_id() {
        let (_dir, path) = test_path("writes");
        record_version_at("proj-write", &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("proj-write"), "expected proj-write in: {}", content);
    }

    #[test]
    fn test_lookup_at_returns_stored_timestamp() {
        let (_dir, path) = test_path("stored");
        record_version_at("proj-stored", &path).unwrap();
        let result = lookup_version_at("proj-stored", &path);
        assert!(result.is_some(), "expected Some but got None");
        let ts = result.unwrap();
        assert!(ts.contains('T'), "expected RFC3339 timestamp, got: {}", ts);
    }

    #[test]
    fn test_lookup_at_unknown_project_returns_none() {
        let (_dir, path) = test_path("unknown");
        record_version_at("proj-known", &path).unwrap();
        assert!(
            lookup_version_at("proj-unknown", &path).is_none(),
            "unknown project should return None"
        );
    }

    #[test]
    fn test_record_at_preserves_other_entries() {
        let (_dir, path) = test_path("preserve");
        record_version_at("proj-a", &path).unwrap();
        record_version_at("proj-b", &path).unwrap();
        assert!(
            lookup_version_at("proj-a", &path).is_some(),
            "proj-a should still exist after recording proj-b"
        );
    }

    #[test]
    fn test_record_at_updates_same_project() {
        let (_dir, path) = test_path("update");
        record_version_at("proj-dup", &path).unwrap();
        record_version_at("proj-dup", &path).unwrap();
        let versions: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let count = versions.keys().filter(|k| *k == "proj-dup").count();
        assert_eq!(count, 1, "expected exactly one entry for proj-dup, got {}", count);
    }

    #[test]
    fn test_write_versions_fails_on_bad_path() {
        let bad_path = std::path::PathBuf::from("/nonexistent_dir/versions.json");
        let versions = HashMap::new();
        let result = write_versions_at(&bad_path, &versions);
        assert!(result.is_err(), "write to non-existent dir must fail");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("nonexistent_dir") || msg.contains("versions.json"),
            "error message must mention the path, got: {}",
            msg
        );
    }

    #[test]
    fn test_lookup_at_returns_none_on_bad_json() {
        let (_dir, path) = test_path("bad-json");
        std::fs::write(&path, b"this is not json").expect("write bad json");
        // serde_json::from_str fails → .ok() → None → unwrap_or_default → empty HashMap
        assert!(
            lookup_version_at("any-project", &path).is_none(),
            "bad JSON must produce an empty map, so lookup returns None"
        );
    }

    #[test]
    fn test_record_and_lookup_version_via_public_api() {
        // Covers the 8-line record_version / lookup_version public wrappers that
        // call through to postlane_dir().
        let uid = format!("coverage-test-proj-{}", std::process::id());
        record_version(&uid).expect("record_version must succeed on a reachable home dir");
        let result = lookup_version(&uid);
        assert!(result.is_some(), "lookup_version must return Some after record_version");
        let ts = result.unwrap();
        assert!(ts.contains('T'), "expected RFC3339 timestamp, got: {}", ts);
        // lookup a project that was never recorded via the public API
        assert!(
            lookup_version("nonexistent-project-coverage-test").is_none(),
            "lookup_version must return None for an unrecorded project"
        );
    }
}
