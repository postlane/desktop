// SPDX-License-Identifier: BUSL-1.1

use crate::workspace_entry::WorkspaceEntry;
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

/// Global desktop registry stored at `~/.postlane/repos.json`.
///
/// Version history:
///   v1 — `{ "version": 1, "repos": [...] }` — individual repo registrations only
///   v2 — `{ "version": 2, "workspaces": [...], "repos": [...] }` — adds workspace
///         support; `repos` array is the legacy per-repo list, preserved for back-compat
///
/// Migration: `repos_migration::migrate_repos_to_v2` rewrites v1 → v2 at app startup.
/// New installations always write v2. New registrations go into `workspaces`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ReposConfig {
    pub version: u32,
    /// Workspace registrations (v2+). Empty on migrated v1 installs until the user
    /// runs "Add workspace". Never written in new per-repo registrations.
    #[serde(default)]
    pub workspaces: Vec<WorkspaceEntry>,
    /// Legacy per-repo registrations from v1. Preserved after migration; new
    /// registrations never add to this array — they go into `workspaces`.
    pub repos: Vec<Repo>,
}

impl Default for ReposConfig {
    fn default() -> Self {
        Self {
            version: REPOS_CONFIG_VERSION,
            workspaces: vec![],
            repos: vec![],
        }
    }
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

/// Write version for new and migrated configs.
pub const REPOS_CONFIG_VERSION: u32 = 2;

/// Versions this binary can read without error. v1 is accepted so that
/// `repos_migration::migrate_repos_to_v2` can run before any other startup code.
const REPOS_SUPPORTED_VERSIONS: &[u32] = &[1, 2];

/// Reads repos.json with corruption recovery.
/// Returns `(config, was_corrupted)` — `was_corrupted` is true when the file
/// existed but was unparseable and had to be replaced with an empty config.
///
/// Accepts both v1 and v2 files. Call `repos_migration::migrate_repos_to_v2`
/// before this function at startup to normalise v1 files to v2.
pub fn read_repos_checked(repos_path: &Path) -> Result<(ReposConfig, bool), StorageError> {
    if !repos_path.exists() {
        return Ok((ReposConfig::default(), false));
    }

    let content = std::fs::read_to_string(repos_path)?;

    match serde_json::from_str::<ReposConfig>(&content) {
        Ok(config) => {
            if !REPOS_SUPPORTED_VERSIONS.contains(&config.version) {
                log::warn!(
                    "Unsupported version in repos.json: found {}, supported {:?}",
                    config.version,
                    REPOS_SUPPORTED_VERSIONS
                );
                return Err(StorageError::VersionMismatch {
                    found: config.version,
                    expected: REPOS_CONFIG_VERSION,
                });
            }
            Ok((config, false))
        }
        Err(e) => {
            log::error!("Failed to parse repos.json: {}", e);
            log::error!("Full parse error: {:?}", e);

            let bak_path = repos_path.with_extension("json.bak");
            if let Err(rename_err) = std::fs::rename(repos_path, &bak_path) {
                log::error!("Failed to create backup: {}", rename_err);
            } else {
                log::info!("Corrupted repos.json backed up to {:?}", bak_path);
            }

            Ok((ReposConfig::default(), true))
        }
    }
}

/// Reads repos.json with corruption recovery.
/// Returns Ok(ReposConfig) on success, or Err on hard errors.
/// On corruption: creates .bak file and returns empty config.
pub fn read_repos_with_recovery(repos_path: &Path) -> Result<ReposConfig, StorageError> {
    read_repos_checked(repos_path).map(|(config, _)| config)
}

/// Writes repos.json atomically. Copies the current file to `repos.json.bak`
/// before overwriting so the previous state survives accidental corruption.
pub fn write_repos(repos_path: &Path, config: &ReposConfig) -> Result<(), StorageError> {
    if repos_path.exists() {
        let bak_path = repos_path.with_extension("json.bak");
        let _ = std::fs::copy(repos_path, &bak_path);
    }
    let json = serde_json::to_string_pretty(config)?;
    crate::init::atomic_write(repos_path, json.as_bytes())?;
    Ok(())
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
