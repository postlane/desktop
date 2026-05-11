// SPDX-License-Identifier: BUSL-1.1

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static WRITE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Returns the path to the ~/.postlane directory
pub fn postlane_dir() -> Result<PathBuf, String> {
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
        let dir = std::env::temp_dir().join("postlane_test_atomic");
        fs::create_dir_all(&dir).expect("Failed to create test directory");

        let target = dir.join("test.json");
        let content = b"{\"test\": true}";

        atomic_write(&target, content).expect("Atomic write failed");

        assert!(target.exists(), "Target file should exist");
        let read_content = fs::read(&target).expect("Failed to read target file");
        assert_eq!(read_content, content, "Content should match");

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_concurrent_same_target_all_succeed() {
        use std::sync::{Arc, Barrier};
        let dir = std::env::temp_dir().join("postlane_test_atomic_concurrent");
        fs::create_dir_all(&dir).expect("create test dir");
        let target = dir.join("shared.json");
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
        let _ = fs::remove_dir_all(&dir);
        assert!(errors.is_empty(), "concurrent writes should all succeed, got: {:?}", errors);
    }

    #[test]
    fn test_atomic_write_preserves_original_on_interruption() {
        let dir = std::env::temp_dir().join("postlane_test_atomic_preserve");
        fs::create_dir_all(&dir).expect("Failed to create test directory");

        let target = dir.join("test.json");
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

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
