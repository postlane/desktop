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

/// Writes `content` to `.postlane/voice_guide.md` in every repo whose `config.json`
/// lists `project_id`. Uses an atomic write (`.tmp` → rename) per file-safety policy.
fn write_voice_guide_to_matching_repos(project_id: &str, content: &str, state: &AppState) {
    for repo_path in crate::scheduler_credentials::collect_matching_repo_paths(project_id, state) {
        let target = repo_path.join(".postlane/voice_guide.md");
        let tmp = repo_path.join(".postlane/voice_guide.md.tmp");
        if let Err(e) = std::fs::write(&tmp, content)
            .and_then(|_| std::fs::rename(&tmp, &target))
        {
            log::warn!("[save_project_voice_guide] write voice_guide.md to {}: {}", repo_path.display(), e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Repo;
    use crate::test_fixtures::make_state;

    fn make_state_with_repo(repo_path: &str) -> AppState {
        make_state(vec![Repo {
            id: "test-repo-id".to_string(),
            name: "test".to_string(),
            path: repo_path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }])
    }

    #[test]
    fn test_write_voice_guide_creates_file_when_project_matches() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-abc"}"#)
            .expect("write config.json");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        write_voice_guide_to_matching_repos("proj-abc", "Write with clarity.", &state);

        let written = std::fs::read_to_string(postlane.join("voice_guide.md")).expect("read");
        assert_eq!(written, "Write with clarity.");
    }

    #[test]
    fn test_write_voice_guide_skips_non_matching_project() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"other-proj"}"#)
            .expect("write config.json");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        write_voice_guide_to_matching_repos("proj-abc", "Write with clarity.", &state);

        assert!(
            !postlane.join("voice_guide.md").exists(),
            "non-matching repo must not be written"
        );
    }

    #[test]
    fn test_write_voice_guide_overwrites_existing_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-xyz"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("voice_guide.md"), "old content").expect("write old");

        let state = make_state_with_repo(dir.path().to_str().unwrap());
        write_voice_guide_to_matching_repos("proj-xyz", "new content", &state);

        let written = std::fs::read_to_string(postlane.join("voice_guide.md")).expect("read");
        assert_eq!(written, "new content");
    }

    #[test]
    fn test_write_voice_guide_no_repos_is_noop() {
        let state = make_state(vec![]);
        // Should not panic with an empty repo list
        write_voice_guide_to_matching_repos("proj-abc", "content", &state);
    }
}

/// Tauri command: saves the voice guide text and structured fields for a project.
#[tauri::command]
pub async fn save_project_voice_guide(
    project_id: String,
    voice_guide: String,
    voice_guide_fields: Option<serde_json::Value>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
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
    write_voice_guide_to_matching_repos(&project_id, &voice_guide, &state);
    Ok(())
}
