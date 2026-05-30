// SPDX-License-Identifier: BUSL-1.1

//! The 8-step migration execution (22.5.5) and crash-recovery resume (22.5.5b).
//!
//! Imported from `workspace_migration` — internal module, not in the public API surface.

use std::path::{Path, PathBuf};
use crate::workspace_migration::{
    LegacyRepoInfo, MigrationJournal, MigrationJournalEntry, MigrationResult,
    RepoMigrationResult, RepoMigrationStatus, read_repo_project_id,
    read_migration_journal, write_migration_journal,
};

/// Stats returned by `resume_migration_journal` for telemetry (22.9.11).
#[derive(Debug, Default)]
pub struct RecoveryStats {
    pub repos_cleaned: usize,
    pub repos_skipped: usize,
}

// ── File copy utilities ───────────────────────────────────────────────────────

/// Returns total byte size of all regular files under `dir` recursively.
pub fn count_dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0; };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += count_dir_bytes(&path);
        } else if let Ok(meta) = std::fs::metadata(&path) {
            total += meta.len();
        }
    }
    total
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("create_dir_all {}: {}", dst.display(), e))?;
    for entry in std::fs::read_dir(src)
        .map_err(|e| format!("read_dir {}: {}", src.display(), e))?.flatten()
    {
        let sp = entry.path();
        let dp = dst.join(entry.file_name());
        if sp.is_dir() {
            copy_dir_recursive(&sp, &dp)?;
        } else {
            std::fs::copy(&sp, &dp)
                .map_err(|e| format!("copy {}: {}", sp.display(), e))?;
        }
    }
    Ok(())
}

// ── Per-repo Steps 1–3 (copy + verify + project_id guard) ────────────────────

struct RepoStepResult {
    posts_dir: String,
}

fn migrate_repo_steps_1_to_3(
    repo: &LegacyRepoInfo,
    workspace_posts: &Path,
    workspace_project_id: &str,
    used_dirs: &[String],
) -> Result<RepoStepResult, RepoMigrationStatus> {
    let repo_path = PathBuf::from(&repo.path);
    let src_posts = repo_path.join(".postlane").join("posts");

    // Step 3: project_id guard (before touching the filesystem)
    if let Some(repo_pid) = read_repo_project_id(&repo_path) {
        if repo_pid != workspace_project_id {
            return Err(RepoMigrationStatus::ProjectIdMismatch);
        }
    }

    let posts_dir = crate::workspace_repos::assign_posts_dir(&repo_path, &{
        used_dirs.iter().map(|d| crate::workspace_repos::RepoEntry {
            id: String::new(), name: String::new(), path: String::new(),
            posts_dir: d.clone(), active: true, added_at: String::new(),
        }).collect::<Vec<_>>()
    });

    let dst_posts = workspace_posts.join(&posts_dir);

    // Step 1: copy
    if let Err(e) = copy_dir_recursive(&src_posts, &dst_posts) {
        return Err(RepoMigrationStatus::VerificationFailed {
            error: format!("PL-MIG-001: copy failed: {}", e),
        });
    }

    // Step 2: byte-count verification
    let src_bytes = count_dir_bytes(&src_posts);
    let dst_bytes = count_dir_bytes(&dst_posts);
    if src_bytes != dst_bytes {
        let _ = std::fs::remove_dir_all(&dst_posts);
        return Err(RepoMigrationStatus::VerificationFailed {
            error: format!("PL-MIG-001: byte count mismatch (src={}, dst={})", src_bytes, dst_bytes),
        });
    }

    Ok(RepoStepResult { posts_dir })
}

// ── Registry helpers (Steps 5 & resume) ──────────────────────────────────────

pub(crate) fn move_repos_to_workspace(
    repo_paths: &[String],
    posts_dirs: &[(String, String)],
    repos_path: &Path,
    workspace_repos_path: &Path,
) -> Result<(), String> {
    let mut global = crate::storage::read_repos_with_recovery(repos_path)
        .unwrap_or_default();
    let mut ws = crate::workspace_repos::read_workspace_repos(workspace_repos_path)
        .unwrap_or(crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: vec![] });

    let dir_map: std::collections::HashMap<_, _> = posts_dirs.iter().cloned().collect();
    let (to_move, to_keep): (Vec<_>, Vec<_>) =
        global.repos.into_iter().partition(|r| repo_paths.contains(&r.path));
    global.repos = to_keep;

    for repo in to_move {
        let posts_dir = dir_map.get(&repo.path).cloned().unwrap_or_else(|| repo.name.clone());
        if ws.repos.iter().any(|r| r.path == repo.path) { continue; }
        ws.repos.push(crate::workspace_repos::RepoEntry {
            id: repo.id, name: repo.name, path: repo.path,
            posts_dir, active: repo.active, added_at: repo.added_at,
        });
    }

    crate::storage::write_repos(repos_path, &global)
        .map_err(|e| format!("Failed to update global repos.json: {:?}", e))?;
    crate::workspace_repos::write_workspace_repos(workspace_repos_path, &ws)?;
    Ok(())
}

// ── Core migration (all 8 steps) ─────────────────────────────────────────────

/// Executes the full 8-step migration. Per-repo failures are isolated.
pub fn execute_migration(
    qualifying_repos: &[LegacyRepoInfo],
    workspace_path: &Path,
    workspace_project_id: &str,
    repos_path: &Path,
) -> MigrationResult {
    let workspace_posts = workspace_path.join("posts");
    std::fs::create_dir_all(&workspace_posts).ok();

    let existing_entries = crate::workspace_repos::read_workspace_repos(
        &workspace_path.join("repos.json"),
    ).unwrap_or(crate::workspace_repos::WorkspaceReposConfig { version: 1, repos: vec![] });
    let mut used_dirs: Vec<String> = existing_entries.repos.iter().map(|r| r.posts_dir.clone()).collect();

    let mut results = vec![];
    let mut journal_entries: Vec<MigrationJournalEntry> = vec![];
    let mut posts_dir_assignments: Vec<(String, String)> = vec![];

    // Steps 1–3 per repo
    for repo in qualifying_repos {
        match migrate_repo_steps_1_to_3(repo, &workspace_posts, workspace_project_id, &used_dirs) {
            Ok(step_result) => {
                used_dirs.push(step_result.posts_dir.clone());
                posts_dir_assignments.push((repo.path.clone(), step_result.posts_dir.clone()));
                journal_entries.push(MigrationJournalEntry {
                    repo_path: repo.path.clone(),
                    posts_dir: step_result.posts_dir.clone(),
                    registry_updated: false,
                    originals_deleted: false,
                });
                results.push(RepoMigrationResult {
                    repo_path: repo.path.clone(), repo_name: repo.name.clone(),
                    status: RepoMigrationStatus::Success { posts_dir: step_result.posts_dir },
                });
            }
            Err(status) => {
                results.push(RepoMigrationResult {
                    repo_path: repo.path.clone(), repo_name: repo.name.clone(), status,
                });
            }
        }
    }

    if journal_entries.is_empty() {
        return MigrationResult { results };
    }

    execute_steps_4_to_7(
        &mut results, journal_entries, posts_dir_assignments, workspace_path, repos_path,
    )
}

fn execute_steps_4_to_7(
    results: &mut Vec<RepoMigrationResult>,
    journal_entries: Vec<MigrationJournalEntry>,
    posts_dirs: Vec<(String, String)>,
    workspace_path: &Path,
    repos_path: &Path,
) -> MigrationResult {
    let journal_path = workspace_path.join(".migration-journal.json");
    let workspace_posts = workspace_path.join("posts");

    // Step 4: write journal (point of no return marker)
    let journal = MigrationJournal { entries: journal_entries.clone(), dismiss_count: 0 };
    if let Err(e) = write_migration_journal(&journal_path, &journal) {
        // Abort — roll back copies
        for entry in &journal_entries {
            let _ = std::fs::remove_dir_all(workspace_posts.join(&entry.posts_dir));
        }
        let aborted: Vec<_> = results.drain(..)
            .map(|r| match r.status {
                RepoMigrationStatus::Success { .. } => RepoMigrationResult {
                    repo_path: r.repo_path, repo_name: r.repo_name,
                    status: RepoMigrationStatus::VerificationFailed {
                        error: format!("PL-MIG-002: {}", e),
                    },
                },
                _ => r,
            }).collect();
        return MigrationResult { results: aborted };
    }

    // Step 5: registry update
    let paths: Vec<String> = journal_entries.iter().map(|e| e.repo_path.clone()).collect();
    let ws_repos_path = workspace_path.join("repos.json");
    if move_repos_to_workspace(&paths, &posts_dirs, repos_path, &ws_repos_path).is_ok() {
        update_journal_flags(&journal_path, true, false);
    }

    // Step 6: delete originals
    for entry in &journal_entries {
        let _ = std::fs::remove_dir_all(
            PathBuf::from(&entry.repo_path).join(".postlane").join("posts"),
        );
    }
    update_journal_flags(&journal_path, true, true);

    // Step 7: clear journal
    let _ = std::fs::remove_file(&journal_path);

    MigrationResult { results: std::mem::take(results) }
}

fn update_journal_flags(path: &Path, registry_updated: bool, originals_deleted: bool) {
    let Ok(content) = std::fs::read_to_string(path) else { return; };
    let Ok(mut j) = serde_json::from_str::<MigrationJournal>(&content) else { return; };
    for e in &mut j.entries {
        if registry_updated { e.registry_updated = true; }
        if originals_deleted { e.originals_deleted = true; }
    }
    write_migration_journal(path, &j).ok();
}

// ── Crash recovery (22.5.5b) ─────────────────────────────────────────────────

/// Resumes an interrupted migration from the journal.
/// Returns `RecoveryStats` for telemetry: how many repos were cleaned vs already done.
pub fn resume_migration_journal(workspace_path: &Path, repos_path: &Path) -> Result<RecoveryStats, String> {
    let journal_path = workspace_path.join(".migration-journal.json");
    let mut journal = read_migration_journal(&journal_path)
        .ok_or_else(|| format!("No journal found at {}", journal_path.display()))?;

    let ws_repos_path = workspace_path.join("repos.json");
    let mut stats = RecoveryStats::default();

    for entry in &mut journal.entries {
        if entry.originals_deleted { stats.repos_skipped += 1; continue; }
        if !entry.registry_updated {
            let already = crate::workspace_repos::read_workspace_repos(&ws_repos_path)
                .map(|ws| ws.repos.iter().any(|r| r.path == entry.repo_path))
                .unwrap_or(false);
            if !already {
                let paths = vec![entry.repo_path.clone()];
                let dirs = vec![(entry.repo_path.clone(), entry.posts_dir.clone())];
                move_repos_to_workspace(&paths, &dirs, repos_path, &ws_repos_path)?;
            }
            entry.registry_updated = true;
        }
        let _ = std::fs::remove_dir_all(
            PathBuf::from(&entry.repo_path).join(".postlane").join("posts"),
        );
        entry.originals_deleted = true;
        stats.repos_cleaned += 1;
    }

    if journal.entries.iter().all(|e| e.originals_deleted) {
        let _ = std::fs::remove_file(&journal_path);
    } else {
        write_migration_journal(&journal_path, &journal)?;
    }
    Ok(stats)
}
