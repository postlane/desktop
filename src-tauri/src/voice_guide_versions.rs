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

    fn test_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("postlane_test_vgv_{}", name));
        fs::create_dir_all(&dir).unwrap();
        dir.join("versions.json")
    }

    #[test]
    fn test_lookup_at_returns_none_when_file_absent() {
        let path = std::env::temp_dir().join("postlane_test_vgv_absent_dir/versions.json");
        assert!(lookup_version_at("proj-1", &path).is_none());
    }

    #[test]
    fn test_record_at_writes_project_id() {
        let path = test_path("writes");
        let _ = fs::remove_file(&path);
        record_version_at("proj-write", &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("proj-write"), "expected proj-write in: {}", content);
    }

    #[test]
    fn test_lookup_at_returns_stored_timestamp() {
        let path = test_path("stored");
        let _ = fs::remove_file(&path);
        record_version_at("proj-stored", &path).unwrap();
        let result = lookup_version_at("proj-stored", &path);
        assert!(result.is_some(), "expected Some but got None");
        let ts = result.unwrap();
        assert!(ts.contains('T'), "expected RFC3339 timestamp, got: {}", ts);
    }

    #[test]
    fn test_lookup_at_unknown_project_returns_none() {
        let path = test_path("unknown");
        let _ = fs::remove_file(&path);
        record_version_at("proj-known", &path).unwrap();
        assert!(
            lookup_version_at("proj-unknown", &path).is_none(),
            "unknown project should return None"
        );
    }

    #[test]
    fn test_record_at_preserves_other_entries() {
        let path = test_path("preserve");
        let _ = fs::remove_file(&path);
        record_version_at("proj-a", &path).unwrap();
        record_version_at("proj-b", &path).unwrap();
        assert!(
            lookup_version_at("proj-a", &path).is_some(),
            "proj-a should still exist after recording proj-b"
        );
    }

    #[test]
    fn test_record_at_updates_same_project() {
        let path = test_path("update");
        let _ = fs::remove_file(&path);
        record_version_at("proj-dup", &path).unwrap();
        record_version_at("proj-dup", &path).unwrap();
        let versions: HashMap<String, String> =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let count = versions.keys().filter(|k| *k == "proj-dup").count();
        assert_eq!(count, 1, "expected exactly one entry for proj-dup, got {}", count);
    }
}
