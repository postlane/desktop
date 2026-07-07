// SPDX-License-Identifier: BUSL-1.1
//! Tauri commands for checklist 24.4.15 — owner-initiated ownership
//! transfer and the 14-day departure window, both surfaced from the
//! account-deletion 409 resolution UI (checklist 24.4.15a).

use crate::license::POSTLANE_API_BASE;
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
use tauri_plugin_keyring::KeyringExt;

async fn resolve_token(app: &tauri::AppHandle) -> Result<String, String> {
    require_license_token(app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?)
}

pub async fn transfer_to_admin_with_client(
    project_id: &str,
    target_user_id: &str,
    idempotency_key: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/billing/transfer/{}", base_url, project_id);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "idempotency_key": idempotency_key, "target_user_id": target_user_id }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}

pub async fn initiate_departure_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/billing/initiate-departure/{}", base_url, project_id);
    let resp = client.post(&url).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}

#[tauri::command]
pub async fn transfer_workspace_to_admin(
    project_id: String,
    target_user_id: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    transfer_to_admin_with_client(&project_id, &target_user_id, &idempotency_key, &build_client(), POSTLANE_API_BASE, &token)
        .await
}

#[tauri::command]
pub async fn initiate_ownership_departure(project_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    initiate_departure_with_client(&project_id, &build_client(), POSTLANE_API_BASE, &token).await
}

#[cfg(test)]
#[path = "ownership_transfer_tests.rs"]
mod tests;
