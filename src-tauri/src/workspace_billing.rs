// SPDX-License-Identifier: BUSL-1.1
//! Tauri commands for checklist 24.4.9 — subscribing a workspace to billing,
//! opening the Stripe Billing Portal, and deactivating (pausing) a workspace's
//! subscription, from the Settings -- Account tab.

use crate::license::POSTLANE_API_BASE;
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
use serde::Deserialize;
use tauri_plugin_keyring::KeyringExt;

#[derive(Deserialize)]
struct SubscribeResponse {
    checkout_url: Option<String>,
}

#[derive(Deserialize)]
struct PortalResponse {
    url: String,
}

async fn resolve_token(app: &tauri::AppHandle) -> Result<String, String> {
    require_license_token(app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?)
}

pub async fn subscribe_workspace_with_client(
    project_id: &str,
    idempotency_key: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Option<String>, String> {
    let url = format!("{}/v1/billing/subscribe", base_url);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "project_id": project_id, "idempotency_key": idempotency_key }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    let body: SubscribeResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.checkout_url)
}

pub async fn open_billing_portal_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<String, String> {
    let url = format!("{}/v1/billing/portal", base_url);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "project_id": project_id }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    let body: PortalResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.url)
}

pub async fn deactivate_workspace_with_client(
    project_id: &str,
    idempotency_key: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/billing/deactivate/{}", base_url, project_id);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "idempotency_key": idempotency_key }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}

#[tauri::command]
pub async fn subscribe_workspace(project_id: String, app: tauri::AppHandle) -> Result<Option<String>, String> {
    let token = resolve_token(&app).await?;
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    subscribe_workspace_with_client(&project_id, &idempotency_key, &build_client(), POSTLANE_API_BASE, &token).await
}

#[tauri::command]
pub async fn open_billing_portal(project_id: String, app: tauri::AppHandle) -> Result<String, String> {
    let token = resolve_token(&app).await?;
    open_billing_portal_with_client(&project_id, &build_client(), POSTLANE_API_BASE, &token).await
}

#[tauri::command]
pub async fn deactivate_workspace(project_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    let idempotency_key = uuid::Uuid::new_v4().to_string();
    deactivate_workspace_with_client(&project_id, &idempotency_key, &build_client(), POSTLANE_API_BASE, &token).await
}

#[cfg(test)]
#[path = "workspace_billing_tests.rs"]
mod tests;
