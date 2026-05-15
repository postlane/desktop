// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for reading and writing project voice guides.

use crate::license::POSTLANE_API_BASE;
use crate::project_cache::{
    get_project_voice_guide_cached, get_voice_guide_fields_with_client,
    save_project_voice_guide_and_fields_with_client, VOICE_GUIDE_CACHE_TTL_SECS,
};
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
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

/// Tauri command: saves the voice guide text and structured fields for a project.
#[tauri::command]
pub async fn save_project_voice_guide(
    project_id: String,
    voice_guide: String,
    voice_guide_fields: Option<serde_json::Value>,
    app: tauri::AppHandle,
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
    Ok(())
}
