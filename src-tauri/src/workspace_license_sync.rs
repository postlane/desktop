// SPDX-License-Identifier: BUSL-1.1

//! Syncs the license-validation response's per-workspace status onto the
//! matching `WorkspaceEntry.license_status` in `~/.postlane/repos.json`
//! (checklist 24.4.8) -- runs on every successful license check so
//! approve_post (checklist 24.4.11) always has a current status to gate on.

use crate::license::validator::WorkspaceLicenseInfo;
use crate::storage::{read_repos_with_recovery, write_repos, StorageError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
static TEST_REPOS_PATH_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<PathBuf>>> =
    std::sync::OnceLock::new();

fn repos_json_path() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let maybe = TEST_REPOS_PATH_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(path) = maybe {
            return Ok(path);
        }
    }
    crate::init::postlane_dir().map(|dir| dir.join("repos.json"))
}

/// Updates `license_status` on every workspace entry whose `id` matches a
/// `project_id` in `workspaces`. Entries with no match are left untouched --
/// they may belong to a workspace the license backend doesn't know about
/// (e.g. offline-created), not necessarily a deleted one.
pub fn sync_license_statuses(workspaces: &[WorkspaceLicenseInfo]) -> Result<(), String> {
    let repos_path = repos_json_path()?;
    apply_license_statuses(&repos_path, workspaces).map_err(|e| format!("{:?}", e))
}

fn apply_license_statuses(repos_path: &Path, workspaces: &[WorkspaceLicenseInfo]) -> Result<(), StorageError> {
    let mut config = read_repos_with_recovery(repos_path)?;
    let info_by_id: HashMap<&str, &WorkspaceLicenseInfo> =
        workspaces.iter().map(|w| (w.project_id.as_str(), w)).collect();

    let mut changed = false;
    for entry in config.workspaces.iter_mut() {
        let Some(info) = info_by_id.get(entry.id.as_str()) else {
            continue;
        };
        if entry.license_status.as_deref() != Some(info.status.as_str()) {
            entry.license_status = Some(info.status.clone());
            changed = true;
        }
        if entry.is_owner != Some(info.is_owner) {
            entry.is_owner = Some(info.is_owner);
            changed = true;
        }
        if entry.status_updated_at.as_deref() != Some(info.status_updated_at.as_str()) {
            entry.status_updated_at = Some(info.status_updated_at.clone());
            changed = true;
        }
    }

    if changed {
        write_repos(repos_path, &config)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "workspace_license_sync_tests.rs"]
mod tests;
