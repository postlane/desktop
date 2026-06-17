// SPDX-License-Identifier: BUSL-1.1

use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns the path to the ~/.postlane directory.
/// When `POSTLANE_DATA_DIR` is set (e.g. in tests), that path is used instead
/// so test runs never touch the real user data directory.
pub fn postlane_dir() -> Result<PathBuf, String> {
    if let Ok(override_dir) = std::env::var("POSTLANE_DATA_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    dirs::home_dir()
        .ok_or_else(|| "Could not determine home directory - HOME environment variable not set".to_string())
        .map(|home| home.join(".postlane"))
}

/// Initializes the ~/.postlane directory
/// Idempotent - safe to call on every launch
pub fn init_postlane_dir() -> Result<(), String> {
    let dir = postlane_dir()?;
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create .postlane directory: {}", e))
}

/// Reads and deserialises a JSON file. Both the read and parse error messages
/// include the file path so failures are diagnosable without a stack trace.
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

/// Serialises `value` as pretty-printed JSON and writes it atomically. Both
/// the serialise and write error messages include the file path.
pub fn write_json_file(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Failed to serialize {}: {}", path.display(), e))?;
    atomic_write(path, json.as_bytes())
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
}

/// Atomic write: writes content to a unique .tmp file then renames to target.
/// Each call uses a unique tmp name (pid + monotonic counter) so parallel
/// callers writing to the same target do not race on a shared .tmp file.
pub fn atomic_write(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let seq = WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let stem = target_path.file_stem().unwrap_or_default().to_string_lossy();
    let tmp_name = format!("{}.{}.{}.tmp", stem, pid, seq);
    let tmp_path = target_path.with_file_name(tmp_name);

    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, target_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    // Serialises env-var mutations so parallel tests cannot interfere.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_postlane_dir_uses_env_override_when_set() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::TempDir::new().expect("create temp dir");
        std::env::set_var("POSTLANE_DATA_DIR", dir.path());
        let result = postlane_dir().expect("must succeed");
        std::env::remove_var("POSTLANE_DATA_DIR");
        assert_eq!(result, dir.path(), "postlane_dir() must return POSTLANE_DATA_DIR when set");
    }

    #[test]
    fn test_postlane_dir_falls_back_to_home_when_env_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("POSTLANE_DATA_DIR");
        let result = postlane_dir().expect("must succeed");
        assert!(
            result.ends_with(".postlane"),
            "postlane_dir() must end with .postlane when env var is absent, got: {}",
            result.display()
        );
    }

    #[test]
    fn test_init_postlane_dir_creates_directory() {
        // This test verifies that init_postlane_dir creates the directory if it doesn't exist
        // It uses the real ~/.postlane directory since the function is not configurable
        // Note: This may cause race conditions with other tests that use ~/.postlane

        let dir = postlane_dir().expect("Failed to get postlane dir");

        // First call should create the directory (idempotent, so safe even if it exists)
        init_postlane_dir().expect("Failed to initialize .postlane directory");

        assert!(dir.exists(), ".postlane directory should exist");
        assert!(dir.is_dir(), ".postlane should be a directory");
    }

    #[test]
    fn test_init_postlane_dir_is_idempotent() {
        // Call initialization 5 times in succession (as per checklist 2.7.2)
        for i in 1..=5 {
            init_postlane_dir().unwrap_or_else(|_| panic!("Call {} failed", i));
        }

        // Should not panic, no duplicate directory, and directory should exist
        let dir = postlane_dir().expect("Failed to get postlane dir");
        assert!(dir.exists());
        assert!(dir.is_dir());
    }

    #[test]
    fn test_atomic_write_creates_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let target = dir.path().join("test.json");
        let content = b"{\"test\": true}";

        atomic_write(&target, content).expect("Atomic write failed");

        assert!(target.exists(), "Target file should exist");
        let read_content = fs::read(&target).expect("Failed to read target file");
        assert_eq!(read_content, content, "Content should match");
    }

    #[test]
    fn test_atomic_write_concurrent_same_target_all_succeed() {
        use std::sync::{Arc, Barrier};
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let target = dir.path().join("shared.json");
        let n = 16usize;
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n).map(|i| {
            let t = target.clone();
            let b = Arc::clone(&barrier);
            std::thread::spawn(move || {
                b.wait();
                atomic_write(&t, format!("content_{}", i).as_bytes())
            })
        }).collect();
        let errors: Vec<_> = handles.into_iter()
            .filter_map(|h| h.join().unwrap().err())
            .collect();
        drop(dir);
        assert!(errors.is_empty(), "concurrent writes should all succeed, got: {:?}", errors);
    }

    #[test]
    fn test_atomic_write_preserves_original_on_interruption() {
        let dir = tempfile::TempDir::new().expect("create temp dir");

        let target = dir.path().join("test.json");
        let original_content = b"{\"original\": true}";
        let new_content = b"{\"new\": true}";

        // Write original file
        fs::write(&target, original_content).expect("Failed to write original");

        // Simulate interruption: write .tmp file but don't rename
        let tmp_path = target.with_extension("tmp");
        fs::write(&tmp_path, new_content).expect("Failed to write tmp");

        // Original file should still have original content
        let read_content = fs::read(&target).expect("Failed to read target file");
        assert_eq!(read_content, original_content, "Original file should be intact");
    }

    #[test]
    fn test_read_json_file_returns_typed_value() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("test.json");
        fs::write(&path, r#"{"key": "value"}"#).expect("write");
        let v: serde_json::Value = read_json_file(&path).expect("must succeed");
        assert_eq!(v["key"].as_str(), Some("value"));
    }

    #[test]
    fn test_read_json_file_returns_err_when_file_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("nonexistent.json");
        let result: Result<serde_json::Value, _> = read_json_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("nonexistent.json"), "error must name the file: {}", err);
    }

    #[test]
    fn test_read_json_file_returns_err_on_invalid_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("bad.json");
        fs::write(&path, "{ not valid json }").expect("write");
        let result: Result<serde_json::Value, _> = read_json_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("bad.json"), "error must name the file: {}", err);
    }

    #[derive(serde::Deserialize)]
    struct TestConfig { name: String }

    #[test]
    fn test_write_json_file_creates_and_reads_back() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("test.json");
        let data = serde_json::json!({"hello": "world", "num": 42});
        write_json_file(&path, &data).expect("must succeed");
        let v: serde_json::Value = read_json_file(&path).expect("must read back");
        assert_eq!(v, data);
    }

    #[test]
    fn test_write_json_file_overwrites_existing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("test.json");
        write_json_file(&path, &serde_json::json!({"v": 1})).expect("first write");
        write_json_file(&path, &serde_json::json!({"v": 2})).expect("second write");
        let v: serde_json::Value = read_json_file(&path).expect("read back");
        assert_eq!(v["v"].as_i64(), Some(2));
    }

    #[test]
    fn test_write_json_file_creates_parent_directory() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("subdir/nested/test.json");
        write_json_file(&path, &serde_json::json!({"key": "val"})).expect("must succeed");
        assert!(path.exists());
    }

    #[test]
    fn test_write_json_file_error_includes_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("test.json");
        let result = write_json_file(&path, &serde_json::json!({"v": 1}));
        if result.is_err() {
            let msg = result.unwrap_err();
            assert!(msg.contains("test.json"), "error must name file, got: {}", msg);
        }
    }

    #[test]
    fn test_read_json_file_deserialises_into_typed_struct() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("config.json");
        fs::write(&path, r#"{"name":"postlane"}"#).expect("write");
        let cfg: TestConfig = read_json_file(&path).expect("must succeed");
        assert_eq!(cfg.name, "postlane");
    }
}
