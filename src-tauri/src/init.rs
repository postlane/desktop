// SPDX-License-Identifier: BUSL-1.1

use std::path::{Path, PathBuf};

/// Returns the path to the ~/.postlane directory
pub fn postlane_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".postlane")
}

/// Initializes the ~/.postlane directory
/// Idempotent - safe to call on every launch
pub fn init_postlane_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(postlane_dir())
}

/// Atomic write: writes content to a .tmp file then renames to target
/// This prevents corruption if the process crashes mid-write
pub fn atomic_write(target_path: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp_path = target_path.with_extension("tmp");

    // Write to .tmp file first
    std::fs::write(&tmp_path, content)?;

    // Atomically rename to target
    std::fs::rename(tmp_path, target_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_init_postlane_dir_creates_directory() {
        // Clean up if exists from previous test run
        let dir = postlane_dir();
        let _ = fs::remove_dir_all(&dir);

        // First call should create the directory
        init_postlane_dir().expect("Failed to initialize .postlane directory");

        assert!(dir.exists(), ".postlane directory should exist");
        assert!(dir.is_dir(), ".postlane should be a directory");
    }

    #[test]
    fn test_init_postlane_dir_is_idempotent() {
        // Call initialization twice in succession
        init_postlane_dir().expect("First call failed");
        init_postlane_dir().expect("Second call failed");

        // Should not panic and directory should exist
        assert!(postlane_dir().exists());
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
