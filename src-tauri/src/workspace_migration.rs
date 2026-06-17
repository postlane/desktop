// SPDX-License-Identifier: BUSL-1.1

//! Workspace migration: moves per-repo `.postlane/posts/` content to a central workspace.
//!
//! Types, pure query functions, and journal helpers live here.
//! Execution logic (the 8-step process) lives in `workspace_migration_execute`.
//! Tauri commands live in `workspace_migration_commands`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── Public types ──────────────────────────────────────────────────────────────

/// A repo entry from the legacy `repos` array that has posts on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRepoInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Returned by `get_migration_status` to determine banner and Settings button visibility.
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationStatus {
    /// Repos with `.postlane/posts/` on disk — controls the banner (22.5.2).
    pub qualifying_repos: Vec<LegacyRepoInfo>,
    /// All repos in legacy array — controls the Settings button (22.5.9).
    pub total_legacy_repos: Vec<LegacyRepoInfo>,
    pub dismissed: bool,
}

/// A config field that differs between a legacy repo and the workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConflict {
    pub field_key: String,
    pub label: String,
    pub repo_value: String,
    pub workspace_value: String,
}

/// Per-repo state tracked in `.migration-journal.json` (22.5.5 Step 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationJournalEntry {
    pub repo_path: String,
    pub posts_dir: String,
    pub registry_updated: bool,
    pub originals_deleted: bool,
}

/// The crash-recovery journal written atomically at `{workspace}/.migration-journal.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationJournal {
    pub entries: Vec<MigrationJournalEntry>,
    /// Incremented each time the user clicks "Dismiss" on the recovery banner.
    /// Non-dismissible after 3 dismissals (22.5.5c).
    #[serde(default)]
    pub dismiss_count: u32,
}

/// Outcome for a single repo after migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tag", rename_all = "snake_case")]
pub enum RepoMigrationStatus {
    Success { posts_dir: String },
    VerificationFailed { error: String },
    ProjectIdMismatch,
    Skipped,
}

/// Migration outcome for one repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMigrationResult {
    pub repo_path: String,
    pub repo_name: String,
    pub status: RepoMigrationStatus,
}

/// Aggregated result returned to the frontend after `execute_migration`.
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationResult {
    pub results: Vec<RepoMigrationResult>,
}

/// Describes a pending journal found on a workspace at startup.
#[derive(Debug, Serialize, Deserialize)]
pub struct JournalStatus {
    pub workspace_id: String,
    pub workspace_path: String,
    pub pending_entries: Vec<MigrationJournalEntry>,
    pub dismiss_count: u32,
}

// ── Human-readable field labels for the conflict diff view (22.5.6) ──────────

pub(crate) const FIELD_LABELS: &[(&str, &str)] = &[
    ("llm.provider", "AI provider"),
    ("llm.model", "AI model"),
    ("repo_type", "Repository type"),
    ("style", "Writing style"),
    ("utm_campaign", "UTM campaign"),
    ("author", "Author name"),
];

// ── Pure query functions ──────────────────────────────────────────────────────

/// Returns repos in the legacy `repos` array whose `{path}/.postlane/posts/` exists on disk.
pub fn find_qualifying_legacy_repos(repos_path: &Path) -> Vec<LegacyRepoInfo> {
    let global = match crate::storage::read_repos_with_recovery(repos_path) {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    global.repos.into_iter()
        .filter(|r| {
            let posts = PathBuf::from(&r.path).join(".postlane").join("posts");
            posts.is_dir()
        })
        .map(|r| LegacyRepoInfo { id: r.id, name: r.name, path: r.path })
        .collect()
}

/// Returns `true` if `workspace_migration_dismissed` is `true` in `app_state.json`.
pub fn check_migration_dismissed(app_state_path: &Path) -> bool {
    let Ok(val) = crate::init::read_json_file::<serde_json::Value>(app_state_path) else { return false; };
    val.get("workspace_migration_dismissed").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Sets `workspace_migration_dismissed: true` in `app_state.json` atomically.
pub fn set_migration_dismissed(app_state_path: &Path) -> Result<(), String> {
    let mut state = read_app_state_from(app_state_path);
    state.workspace_migration_dismissed = true;
    crate::init::write_json_file(app_state_path, &state)
}

/// Returns migration status: qualifying repos + whether banner is dismissed.
/// `total_legacy_repos` is always populated (Settings button uses it; ignores dismissed flag).
pub fn get_migration_status_impl(repos_path: &Path, app_state_path: &Path) -> MigrationStatus {
    let dismissed = check_migration_dismissed(app_state_path);
    let qualifying_repos = if dismissed { vec![] } else { find_qualifying_legacy_repos(repos_path) };
    let total_legacy_repos = find_all_legacy_repos(repos_path);
    MigrationStatus { qualifying_repos, total_legacy_repos, dismissed }
}

/// Reads `project_id` from `{repo}/.postlane/config.json`. Returns `None` when absent.
pub fn read_repo_project_id(repo_path: &Path) -> Option<String> {
    let config = repo_path.join(".postlane").join("config.json");
    let val: serde_json::Value = crate::init::read_json_file(&config).ok()?;
    val.get("project_id")?.as_str().map(|s| s.to_string())
}

/// Returns fields that differ between `{repo}/.postlane/config.json` and `{workspace}/config.json`.
/// Never includes `project_id` or `schema_version`.
pub fn detect_config_conflicts(repo_path: &Path, workspace_path: &Path) -> Vec<FieldConflict> {
    let repo_cfg = read_flat_config(repo_path.join(".postlane").join("config.json"));
    let ws_cfg = read_flat_config(workspace_path.join("config.json"));
    let mut conflicts = vec![];
    for (field_key, label) in FIELD_LABELS {
        let rv = repo_cfg.get(*field_key).cloned().unwrap_or_default();
        let wv = ws_cfg.get(*field_key).cloned().unwrap_or_default();
        if rv != wv && !rv.is_empty() && !wv.is_empty() {
            conflicts.push(FieldConflict {
                field_key: field_key.to_string(),
                label: label.to_string(),
                repo_value: rv,
                workspace_value: wv,
            });
        }
    }
    conflicts
}

// ── Journal read/write ────────────────────────────────────────────────────────

/// Writes the migration journal atomically with 0600 permissions (22.5 Step 4).
pub fn write_migration_journal(path: &Path, journal: &MigrationJournal) -> Result<(), String> {
    let json = serde_json::to_string_pretty(journal)
        .map_err(|e| format!("PL-MIG-002: failed to serialise journal: {}", e))?;
    let tmp_path = path.with_extension("json.tmp");
    write_with_0600(&tmp_path, json.as_bytes())
        .map_err(|e| format!("PL-MIG-002: {}", e))?;
    std::fs::rename(&tmp_path, path)
        .map_err(|e| format!("PL-MIG-002: failed to rename journal: {}", e))
}

/// Reads the journal from disk. Returns `None` when absent or corrupt.
pub fn read_migration_journal(path: &Path) -> Option<MigrationJournal> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

// ── Crash-recovery journal scan ───────────────────────────────────────────────

/// Scans all workspaces for `.migration-journal.json` files with pending entries.
pub fn check_migration_journals(repos_path: &Path) -> Vec<JournalStatus> {
    let global = match crate::storage::read_repos_with_recovery(repos_path) {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    let mut statuses = vec![];
    for ws in &global.workspaces {
        let journal_path = PathBuf::from(&ws.workspace_path).join(".migration-journal.json");
        if let Some(journal) = read_migration_journal(&journal_path) {
            let pending: Vec<_> = journal.entries.iter()
                .filter(|e| !e.originals_deleted).cloned().collect();
            if !pending.is_empty() {
                statuses.push(JournalStatus {
                    workspace_id: ws.id.clone(),
                    workspace_path: ws.workspace_path.clone(),
                    pending_entries: pending,
                    dismiss_count: journal.dismiss_count,
                });
            }
        }
    }
    statuses
}

/// Increments the dismiss count in the journal (22.5.5c: non-dismissible after 3).
pub fn dismiss_migration_journal_session(workspace_path: &Path) -> Result<(), String> {
    let journal_path = workspace_path.join(".migration-journal.json");
    let mut journal = read_migration_journal(&journal_path)
        .ok_or_else(|| "No journal found".to_string())?;
    journal.dismiss_count = journal.dismiss_count.saturating_add(1);
    write_migration_journal(&journal_path, &journal)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Flattens a `{workspace}/config.json` to a `key → value` map using dot-notation.
pub(crate) fn read_flat_config(path: PathBuf) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let Ok(val) = crate::init::read_json_file::<serde_json::Value>(&path) else { return map; };
    flatten_json_value(&val, "", &mut map);
    map
}

fn flatten_json_value(
    val: &serde_json::Value,
    prefix: &str,
    out: &mut std::collections::HashMap<String, String>,
) {
    match val {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let key = if prefix.is_empty() { k.clone() } else { format!("{}.{}", prefix, k) };
                flatten_json_value(v, &key, out);
            }
        }
        serde_json::Value::String(s) => { out.insert(prefix.to_string(), s.clone()); }
        other => { out.insert(prefix.to_string(), other.to_string()); }
    }
}

fn write_with_0600(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = OpenOptions::new()
            .write(true).create(true).truncate(true).mode(0o600)
            .open(path)
            .map_err(|e| format!("open failed: {}", e))?;
        f.write_all(bytes).map_err(|e| format!("write failed: {}", e))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod failed: {}", e))?;
        Ok(())
    }
    #[cfg(not(unix))]
    std::fs::write(path, bytes).map_err(|e| format!("write failed: {}", e))
}

/// Path-parameterised `read_app_state` for isolated testing.
pub(crate) fn read_app_state_from(path: &Path) -> crate::app_state_types::AppStateFile {
    if !path.exists() {
        return crate::app_state_types::AppStateFile::default();
    }
    let Ok(content) = std::fs::read_to_string(path) else {
        return crate::app_state_types::AppStateFile::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

// ── 22.5.9: all repos in legacy array (button tracks entries, not on-disk posts) ────

/// Returns all repos in the legacy `repos` array regardless of whether posts exist.
/// Used for the "Migrate to workspace..." Settings button (22.5.9).
pub fn find_all_legacy_repos(repos_path: &Path) -> Vec<LegacyRepoInfo> {
    let global = match crate::storage::read_repos_with_recovery(repos_path) {
        Ok(g) => g,
        Err(_) => return vec![],
    };
    global.repos.into_iter()
        .map(|r| LegacyRepoInfo { id: r.id, name: r.name, path: r.path })
        .collect()
}

// ── 22.5.6: conflict detection ────────────────────────────────────────────────

/// Per-repo config conflicts returned to the frontend before migration starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConflicts {
    pub repo_path: String,
    pub repo_name: String,
    pub conflicts: Vec<FieldConflict>,
}

/// Returns config field conflicts for each qualifying repo (22.5.6).
/// Only repos whose `project_id` matches the workspace are checked.
pub fn get_migration_conflicts_impl(
    qualifying_repos: &[LegacyRepoInfo],
    workspace_path: &Path,
    workspace_project_id: &str,
) -> Vec<RepoConflicts> {
    qualifying_repos.iter().filter_map(|repo| {
        let repo_path = PathBuf::from(&repo.path);
        if let Some(pid) = read_repo_project_id(&repo_path) {
            if pid != workspace_project_id { return None; }
        }
        let conflicts = detect_config_conflicts(&repo_path, workspace_path);
        if conflicts.is_empty() { return None; }
        Some(RepoConflicts { repo_path: repo.path.clone(), repo_name: repo.name.clone(), conflicts })
    }).collect()
}

#[cfg(test)]
#[path = "workspace_migration_tests.rs"]
mod tests;
