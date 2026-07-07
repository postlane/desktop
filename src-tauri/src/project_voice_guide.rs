// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for reading and writing project voice guides.

use crate::app_state::AppState;
use crate::license::POSTLANE_API_BASE;
use crate::project_cache::{
    get_project_voice_guide_cached, get_voice_guide_fields_with_client,
    save_project_voice_guide_and_fields_with_client, VOICE_GUIDE_CACHE_TTL_SECS,
};
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
use tauri::State;
use tauri_plugin_keyring::KeyringExt;

/// Tauri command: returns the voice guide text for a project, using the cache when fresh.
#[tauri::command]
pub async fn get_project_voice_guide(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    get_project_voice_guide_cached(&project_id, &client, POSTLANE_API_BASE, &token, VOICE_GUIDE_CACHE_TTL_SECS).await
}

/// Tauri command: returns the structured voice guide fields for a project.
#[tauri::command]
pub async fn get_voice_guide_fields(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<Option<serde_json::Value>, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    get_voice_guide_fields_with_client(&project_id, &client, POSTLANE_API_BASE, &token).await
}

/// Result returned by `save_project_voice_guide` and `sync_voice_guide_to_repos`.
/// `synced` lists the paths that were written; `registered` is the total number of
/// repos matched to this project (including those whose paths no longer exist on disk).
/// The frontend uses these two counts to distinguish the two "0 repos synced" states
/// (21.3.8): no repos configured at all vs repos configured but paths missing.
#[derive(serde::Serialize)]
pub struct SyncStatus {
    pub synced: Vec<String>,
    pub registered: usize,
}

// 22.1.0 — voice_guide.md moves to {workspace}/voice_guide.md exclusively (22.1.9).
// Per-repo .postlane/voice_guide.md files are no longer written. Rationale:
// per-repo copies create unsolicited committed files in child repos, can diverge
// from the workspace canonical version on conflict, and contradict the
// "no Postlane files in child repos" promise introduced in v1.4.

/// Writes `voice_guide` to `{workspace}/voice_guide.md` for every workspace entry
/// in `state.repos.workspaces` where `entry.id == project_id` (22.1.9).
///
/// Workspaces whose path no longer exists on disk are skipped with a warning.
/// Returns a `SyncStatus` with the successfully-written workspace paths and
/// the count of matching workspace registrations.
pub(crate) fn sync_voice_guide_to_repos_impl(
    project_id: &str,
    voice_guide: &str,
    state: &AppState,
) -> SyncStatus {
    let repos = match state.repos.lock() {
        Ok(r) => r,
        Err(e) => {
            log::error!("[sync_voice_guide] failed to lock repos: {}", e);
            return SyncStatus { synced: vec![], registered: 0 };
        }
    };

    let matching: Vec<_> = repos
        .workspaces
        .iter()
        .filter(|w| w.active && w.id == project_id)
        .collect();

    let registered = matching.len();

    if voice_guide.trim().is_empty() {
        return SyncStatus { synced: vec![], registered };
    }

    let mut synced = Vec::new();
    for workspace in matching {
        let ws_path = std::path::Path::new(&workspace.workspace_path);
        if !ws_path.exists() {
            log::warn!(
                "[sync_voice_guide] workspace path no longer exists: {}",
                workspace.workspace_path
            );
            continue;
        }
        let target = ws_path.join("voice_guide.md");
        let tmp = ws_path.join("voice_guide.md.tmp");
        match std::fs::write(&tmp, voice_guide).and_then(|_| std::fs::rename(&tmp, &target)) {
            Ok(_) => synced.push(workspace.workspace_path.clone()),
            Err(e) => log::warn!(
                "[sync_voice_guide] write to {}: {}",
                workspace.workspace_path,
                e
            ),
        }
    }
    SyncStatus { synced, registered }
}

/// Tauri command: syncs a voice guide to all repos registered under a project.
/// Returns a `SyncStatus` with synced paths and total registered count.
#[tauri::command]
pub fn sync_voice_guide_to_repos(
    project_id: String,
    voice_guide: String,
    state: tauri::State<'_, AppState>,
) -> SyncStatus {
    sync_voice_guide_to_repos_impl(&project_id, &voice_guide, &state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Repo;
    use crate::test_fixtures::make_state;

    fn make_state_with_repo(repo_path: &str) -> AppState {
        let state = make_state(vec![Repo {
            id: "test-repo-id".to_string(),
            name: "test".to_string(),
            path: repo_path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }]);
        let repos = state.repos.lock().unwrap().clone();
        crate::storage::write_repos(&state.repos_path, &repos).expect("write repos.json for test");
        state
    }

    /// 22.1.9 — saves voice guide to workspace root, not per-repo .postlane/
    #[test]
    fn test_sync_writes_to_matching_workspace_and_returns_path() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("proj-abc", ws.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-abc", "Write with clarity.", &state);

        let expected = ws.path().join("voice_guide.md");
        assert!(expected.exists(), "voice_guide.md must exist at workspace root");
        let written = std::fs::read_to_string(&expected).expect("read");
        assert_eq!(written, "Write with clarity.");
        assert_eq!(result.synced.len(), 1);
        assert_eq!(result.registered, 1);
        assert!(result.synced[0].contains(ws.path().to_str().unwrap()));
    }

    #[test]
    fn test_sync_skips_non_matching_project() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("other-proj", ws.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-abc", "Write with clarity.", &state);

        assert!(!ws.path().join("voice_guide.md").exists(), "non-matching workspace must not be written");
        assert!(result.synced.is_empty());
        assert_eq!(result.registered, 0);
    }

    /// 22.1.9 — overwrites existing workspace voice_guide.md, does not append
    #[test]
    fn test_sync_overwrites_existing_file() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        std::fs::write(ws.path().join("voice_guide.md"), "old content").expect("write old");
        let state = make_state_with_workspace("proj-xyz", ws.path().to_str().unwrap());
        sync_voice_guide_to_repos_impl("proj-xyz", "new content", &state);

        let written = std::fs::read_to_string(ws.path().join("voice_guide.md")).expect("read");
        assert_eq!(written, "new content");
    }

    #[test]
    fn test_sync_no_workspaces_returns_empty_vec() {
        let state = make_state(vec![]);
        let result = sync_voice_guide_to_repos_impl("proj-abc", "content", &state);
        assert!(result.synced.is_empty());
        assert_eq!(result.registered, 0);
    }

    /// 22.1.9 — nonexistent workspace path skipped; existing workspace still written
    #[test]
    fn test_sync_skips_nonexistent_path_but_writes_existing() {
        use crate::workspace_entry::WorkspaceEntry;
        let existing = tempfile::TempDir::new().expect("create existing dir");
        let missing_path = "/tmp/postlane_nonexistent_workspace_for_test_22_1";
        let _ = std::fs::remove_dir_all(missing_path);

        let config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![
                WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                    id: "proj-multi".to_string(),
                    name: "missing".to_string(),
                    workspace_path: missing_path.to_string(),
                    active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
                WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                    id: "proj-multi".to_string(),
                    name: "existing".to_string(),
                    workspace_path: existing.path().to_str().unwrap().to_string(),
                    active: true,
                    added_at: "2026-01-01T00:00:00Z".to_string(),
                },
            ],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!(
            "postlane_vg_multi_test_{}.json", std::process::id()
        ));
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = sync_voice_guide_to_repos_impl("proj-multi", "guide text", &state);

        assert!(existing.path().join("voice_guide.md").exists(), "existing workspace must be written");
        assert_eq!(result.synced.len(), 1, "only the existing workspace path in synced list");
        assert_eq!(result.registered, 2, "both registrations count as registered");
    }

    /// 22.1.9 — atomic write; no .tmp file left after success
    #[test]
    fn test_sync_no_tmp_file_after_successful_write() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("proj-atomic", ws.path().to_str().unwrap());
        sync_voice_guide_to_repos_impl("proj-atomic", "atomic content", &state);

        assert!(!ws.path().join("voice_guide.md.tmp").exists(), ".tmp file must not exist after write");
        assert!(ws.path().join("voice_guide.md").exists(), "final file must exist");
    }

    /// 22.1.9 — on-disk content at workspace root exactly matches the input string
    #[test]
    fn test_sync_content_readback_matches_input() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("proj-readback", ws.path().to_str().unwrap());
        let input = "Voice guide content.\nLine two.\n";
        let result = sync_voice_guide_to_repos_impl("proj-readback", input, &state);
        assert_eq!(result.synced.len(), 1);
        let on_disk = std::fs::read_to_string(ws.path().join("voice_guide.md")).expect("read");
        assert_eq!(on_disk, input, "on-disk bytes must exactly match input");
    }

    // ── 22.1.9 workspace-root voice guide ────────────────────────────────────

    fn make_state_with_workspace(project_id: &str, workspace_path: &str) -> AppState {
        use crate::workspace_entry::WorkspaceEntry;
        let config = crate::storage::ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
    license_status: None,
    is_owner: None,
    status_updated_at: None,
                id: project_id.to_string(),
                name: "test-ws".to_string(),
                workspace_path: workspace_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!(
            "postlane_vg_ws_test_{}_{}.json",
            std::process::id(),
            workspace_path.len()
        ));
        crate::app_state::AppState::new_with_path(config, repos_path)
    }

    /// 22.1.9 — voice guide written to {workspace}/voice_guide.md, not per-repo.
    #[test]
    fn test_sync_writes_voice_guide_to_workspace_root() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("proj-ws-1", ws.path().to_str().unwrap());

        let result = sync_voice_guide_to_repos_impl("proj-ws-1", "Write with clarity.", &state);

        let expected_path = ws.path().join("voice_guide.md");
        assert!(expected_path.exists(), "voice_guide.md must exist at workspace root");
        let written = std::fs::read_to_string(&expected_path).expect("read");
        assert_eq!(written, "Write with clarity.");
        assert_eq!(result.synced.len(), 1);
        assert_eq!(result.registered, 1);
    }

    /// 22.1.9 — per-repo legacy registrations do NOT get voice guide written.
    #[test]
    fn test_sync_does_not_write_to_per_repo_paths() {
        let dir = tempfile::TempDir::new().expect("create repo dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-legacy"}"#)
            .expect("write per-repo config");

        // State with ONLY a per-repo registration (legacy `repos` array)
        let state = make_state_with_repo(dir.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-legacy", "should not be written", &state);

        assert!(
            !postlane.join("voice_guide.md").exists(),
            "voice_guide.md must NOT be written to per-repo paths in v1.4"
        );
        assert_eq!(result.synced.len(), 0, "per-repo paths yield no synced entries");
        assert_eq!(result.registered, 0, "per-repo paths yield 0 registered");
    }

    /// 22.1.9 — write failure in workspace is logged; workspace not in synced list
    #[test]
    fn test_sync_logs_warning_and_excludes_path_on_write_failure() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        // Block the tmp file by placing a directory at that path
        std::fs::create_dir_all(ws.path().join("voice_guide.md.tmp"))
            .expect("create dir at tmp path");

        let state = make_state_with_workspace("proj-writefail", ws.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-writefail", "should not be written", &state);

        assert!(result.synced.is_empty(), "failed write must not appear in synced list");
        assert_eq!(result.registered, 1, "workspace is still registered");
        assert!(
            !ws.path().join("voice_guide.md").exists(),
            "voice_guide.md must not exist after write failure"
        );
    }

    /// 22.1.9 — whitespace-only voice guide does not write a file
    #[test]
    fn test_sync_skips_whitespace_only_voice_guide() {
        let ws = tempfile::TempDir::new().expect("create workspace dir");
        let state = make_state_with_workspace("proj-ws", ws.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-ws", "   \n  ", &state);
        assert!(result.synced.is_empty(), "whitespace-only guide must return empty vec");
        assert_eq!(result.registered, 1, "workspace is still registered even when guide is blank");
        assert!(!ws.path().join("voice_guide.md").exists(), "no file must be written for whitespace input");
    }
}

/// Tauri command: saves the voice guide text and structured fields for a project.
/// Returns a `SyncStatus` with synced repo paths and the total registered count.
#[tauri::command]
pub async fn save_project_voice_guide(
    project_id: String,
    voice_guide: String,
    voice_guide_fields: Option<serde_json::Value>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<SyncStatus, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    save_project_voice_guide_and_fields_with_client(
        &project_id,
        &voice_guide,
        voice_guide_fields.as_ref(),
        &client,
        POSTLANE_API_BASE,
        &token,
    )
    .await?;
    let _ = crate::voice_guide_versions::record_version(&project_id);
    Ok(sync_voice_guide_to_repos_impl(&project_id, &voice_guide, &state))
}
