// SPDX-License-Identifier: BUSL-1.1

use std::path::PathBuf;

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
}
