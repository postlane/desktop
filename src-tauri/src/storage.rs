// SPDX-License-Identifier: BUSL-1.1

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub active: bool,
    pub added_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReposConfig {
    pub version: u32,
    pub repos: Vec<Repo>,
}

#[derive(Debug)]
pub enum StorageError {
    IoError(std::io::Error),
    ParseError(serde_json::Error),
    VersionMismatch { found: u32, expected: u32 },
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::IoError(err)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        StorageError::ParseError(err)
    }
}

const REPOS_CONFIG_VERSION: u32 = 1;

/// Reads repos.json with corruption recovery
/// Returns Ok(ReposConfig) on success, or Err if file is missing
/// On corruption: creates .bak file and returns empty config
pub fn read_repos_with_recovery(repos_path: &Path) -> Result<ReposConfig, StorageError> {
    // If file doesn't exist, return empty config
    if !repos_path.exists() {
        return Ok(ReposConfig {
            version: REPOS_CONFIG_VERSION,
            repos: vec![],
        });
    }

    // Try to read and parse
    let content = std::fs::read_to_string(repos_path)?;

    match serde_json::from_str::<ReposConfig>(&content) {
        Ok(config) => {
            // Check version
            if config.version != REPOS_CONFIG_VERSION {
                log::warn!(
                    "Version mismatch in repos.json: found {}, expected {}",
                    config.version,
                    REPOS_CONFIG_VERSION
                );
                return Err(StorageError::VersionMismatch {
                    found: config.version,
                    expected: REPOS_CONFIG_VERSION,
                });
            }
            Ok(config)
        }
        Err(e) => {
            // Corruption detected - log full error
            log::error!("Failed to parse repos.json: {}", e);
            log::error!("Full parse error: {:?}", e);

            // Rename bad file to .bak
            let bak_path = repos_path.with_extension("json.bak");
            if let Err(rename_err) = std::fs::rename(repos_path, &bak_path) {
                log::error!("Failed to create backup: {}", rename_err);
            } else {
                log::info!("Corrupted repos.json backed up to {:?}", bak_path);
            }

            // Return empty config - do not panic
            Ok(ReposConfig {
                version: REPOS_CONFIG_VERSION,
                repos: vec![],
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_read_repos_missing_file_returns_empty() {
        let dir = std::env::temp_dir().join("postlane_test_repos_missing");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let repos_path = dir.join("repos.json");

        let result = read_repos_with_recovery(&repos_path).expect("Should return empty config");
        assert_eq!(result.version, 1);
        assert_eq!(result.repos.len(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_repos_malformed_json_creates_backup() {
        let dir = std::env::temp_dir().join("postlane_test_repos_corrupt");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let repos_path = dir.join("repos.json");
        let bak_path = dir.join("repos.json.bak");

        // Write malformed JSON
        fs::write(&repos_path, "{ this is not valid json }").expect("Failed to write malformed JSON");

        // Should not panic, should return empty config
        let result = read_repos_with_recovery(&repos_path).expect("Should recover from corruption");
        assert_eq!(result.version, 1);
        assert_eq!(result.repos.len(), 0, "Should return empty repos list");

        // Backup should exist
        assert!(bak_path.exists(), "Backup file should exist");
        assert!(!repos_path.exists(), "Original should be renamed");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_repos_valid_json_parses_correctly() {
        let dir = std::env::temp_dir().join("postlane_test_repos_valid");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let repos_path = dir.join("repos.json");

        let config = ReposConfig {
            version: 1,
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

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_repos_version_mismatch_returns_error() {
        let dir = std::env::temp_dir().join("postlane_test_repos_version");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let repos_path = dir.join("repos.json");

        // Write config with wrong version
        let json = r#"{"version": 999, "repos": []}"#;
        fs::write(&repos_path, json).expect("Failed to write JSON");

        let result = read_repos_with_recovery(&repos_path);
        assert!(result.is_err(), "Should return error on version mismatch");

        match result {
            Err(StorageError::VersionMismatch { found, expected }) => {
                assert_eq!(found, 999);
                assert_eq!(expected, 1);
            }
            _ => panic!("Expected VersionMismatch error"),
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
