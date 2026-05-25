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

// 21.3.0 — Decision: `.postlane/voice_guide.md` is COMMITTED to the repo.
// Rationale: team members who clone the repo get the voice guide automatically.
// Every app-side save creates an uncommitted change the user must notice and push,
// which is acceptable — they would want to share the update with their team.

/// Writes `voice_guide` to `{repo_path}/.postlane/voice_guide.md` atomically.
/// Creates `.postlane/` if it does not exist (21.3.3).
pub(crate) fn write_voice_guide_to_repo(
    repo_path: &std::path::Path,
    voice_guide: &str,
) -> std::io::Result<()> {
    let postlane_dir = repo_path.join(".postlane");
    std::fs::create_dir_all(&postlane_dir)?;
    let target = postlane_dir.join("voice_guide.md");
    let tmp = postlane_dir.join("voice_guide.md.tmp");
    std::fs::write(&tmp, voice_guide)?;
    std::fs::rename(&tmp, &target)
}

/// Writes `voice_guide` to `.postlane/voice_guide.md` in every repo registered under
/// `project_id`. Repos whose path no longer exists on disk are skipped with a warning.
/// Returns a `SyncStatus` with the successfully-written paths and the total registered count.
pub(crate) fn sync_voice_guide_to_repos_impl(
    project_id: &str,
    voice_guide: &str,
    state: &AppState,
) -> SyncStatus {
    let repo_paths =
        crate::credential_repo_sync::collect_matching_repo_paths(project_id, state);
    let registered = repo_paths.len();
    if voice_guide.trim().is_empty() {
        return SyncStatus { synced: Vec::new(), registered };
    }
    let mut synced = Vec::new();
    for repo_path in repo_paths {
        if !repo_path.exists() {
            log::warn!(
                "[sync_voice_guide_to_repos] repo path no longer exists: {}",
                repo_path.display()
            );
            continue;
        }
        match write_voice_guide_to_repo(&repo_path, voice_guide) {
            Ok(_) => synced.push(repo_path.display().to_string()),
            Err(e) => log::warn!(
                "[sync_voice_guide_to_repos] write to {}: {}",
                repo_path.display(),
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

    /// 21.3.10 — saves voice guide to all registered repos under the project
    #[test]
    fn test_sync_writes_to_matching_repo_and_returns_path() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-abc"}"#)
            .expect("write config.json");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-abc", "Write with clarity.", &state);

        let written = std::fs::read_to_string(postlane.join("voice_guide.md")).expect("read");
        assert_eq!(written, "Write with clarity.");
        assert_eq!(result.synced.len(), 1);
        assert_eq!(result.registered, 1);
        assert!(result.synced[0].contains(dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_sync_skips_non_matching_project() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"other-proj"}"#)
            .expect("write config.json");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-abc", "Write with clarity.", &state);

        assert!(!postlane.join("voice_guide.md").exists(), "non-matching repo must not be written");
        assert!(result.synced.is_empty());
        assert_eq!(result.registered, 0);
    }

    /// 21.3.13 — overwrites, does not append
    #[test]
    fn test_sync_overwrites_existing_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-xyz"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("voice_guide.md"), "old content").expect("write old");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        sync_voice_guide_to_repos_impl("proj-xyz", "new content", &state);

        let written = std::fs::read_to_string(postlane.join("voice_guide.md")).expect("read");
        assert_eq!(written, "new content");
    }

    #[test]
    fn test_sync_no_repos_returns_empty_vec() {
        let state = make_state(vec![]);
        let result = sync_voice_guide_to_repos_impl("proj-abc", "content", &state);
        assert!(result.synced.is_empty());
        assert_eq!(result.registered, 0);
    }

    /// 21.3.11 — missing path skipped; other repo still written
    #[test]
    fn test_sync_skips_nonexistent_path_but_writes_existing() {
        let existing = tempfile::TempDir::new().expect("create existing dir");
        let postlane = existing.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-multi"}"#)
            .expect("write config.json");

        let missing_path = "/tmp/postlane_nonexistent_repo_path_for_test_21_3_11";
        // Ensure the path really doesn't exist
        let _ = std::fs::remove_dir_all(missing_path);

        let state = make_state(vec![
            Repo {
                id: "repo-missing".to_string(),
                name: "missing".to_string(),
                path: missing_path.to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            },
            Repo {
                id: "repo-existing".to_string(),
                name: "existing".to_string(),
                path: existing.path().to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            },
        ]);
        let repos = state.repos.lock().unwrap().clone();
        crate::storage::write_repos(&state.repos_path, &repos).expect("write repos.json for test");

        let result = sync_voice_guide_to_repos_impl("proj-multi", "guide text", &state);

        assert!(postlane.join("voice_guide.md").exists(), "existing repo must be written");
        assert_eq!(result.synced.len(), 1, "only the existing repo path should be in synced list");
        assert_eq!(result.registered, 1, "only the repo with config.json counts as registered");
    }

    /// 21.3.12 — atomic write; no .tmp file left after success
    #[test]
    fn test_sync_no_tmp_file_after_successful_write() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-atomic"}"#)
            .expect("write config.json");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        sync_voice_guide_to_repos_impl("proj-atomic", "atomic content", &state);

        assert!(!postlane.join("voice_guide.md.tmp").exists(), ".tmp file must not exist after write");
        assert!(postlane.join("voice_guide.md").exists(), "final file must exist");
    }

    /// 21.3.17 — write_voice_guide_to_repo creates .postlane/ when it does not exist
    #[test]
    fn test_write_creates_postlane_dir_and_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // No .postlane/ directory pre-created — exercises the create_dir_all path
        write_voice_guide_to_repo(dir.path(), "Be direct.").expect("write should succeed");
        assert!(dir.path().join(".postlane").is_dir(), ".postlane/ must be created");
        let content = std::fs::read_to_string(dir.path().join(".postlane/voice_guide.md"))
            .expect("read voice_guide.md");
        assert_eq!(content, "Be direct.");
    }

    /// 21.3.14 — on-disk content exactly matches the input string
    #[test]
    fn test_sync_content_readback_matches_input() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-readback"}"#)
            .expect("write config.json");
        let state = make_state_with_repo(dir.path().to_str().unwrap());
        let input = "Voice guide content.\nLine two.\n";
        let result = sync_voice_guide_to_repos_impl("proj-readback", input, &state);
        assert_eq!(result.synced.len(), 1);
        let on_disk = std::fs::read_to_string(postlane.join("voice_guide.md")).expect("read");
        assert_eq!(on_disk, input, "on-disk bytes must exactly match input");
    }

    /// 21.3.16 — whitespace-only voice guide does not write a file
    #[test]
    fn test_sync_skips_whitespace_only_voice_guide() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-ws"}"#)
            .expect("write config.json");
        let state = make_state_with_repo(dir.path().to_str().unwrap());
        let result = sync_voice_guide_to_repos_impl("proj-ws", "   \n  ", &state);
        assert!(result.synced.is_empty(), "whitespace-only guide must return empty vec");
        assert_eq!(result.registered, 1, "repo is still registered even when guide is blank");
        assert!(!postlane.join("voice_guide.md").exists(), "no file must be written for whitespace input");
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
