// SPDX-License-Identifier: BUSL-1.1
//! Tauri commands for checklist 24.4.9 — subscribing a workspace to billing,
//! opening the Stripe Billing Portal, and deactivating (pausing) a workspace's
//! subscription, from the Settings -- Account tab.

use crate::license::POSTLANE_API_BASE;
use crate::license::validator::WorkspaceLicenseInfo;
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
use serde::{Deserialize, Serialize};
use tauri_plugin_keyring::KeyringExt;

#[derive(Deserialize)]
struct SubscribeResponse {
    checkout_url: Option<String>,
}

#[derive(Deserialize)]
struct PortalResponse {
    url: String,
}

/// Response shape from `GET /v1/projects/{project_id}/billing-status`
/// (checklist 24.3.6a) -- `owner` is present only for `status: "collaborator"`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BillingStatusResponse {
    pub status: String,
    #[serde(default)]
    pub owner: Option<String>,
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

pub async fn get_billing_status_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<BillingStatusResponse, String> {
    let url = format!("{}/v1/projects/{}/billing-status", base_url, project_id);
    let resp = client.get(&url).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

/// Maps a billing-status response onto the shape `workspace_license_sync`
/// expects. `owner` is not included -- `is_owner` is derived from `status`
/// instead (the endpoint's only signal: `collaborator` means not-owner,
/// every other status means owner), since `GET billing-status` doesn't
/// return an explicit boolean the way `POST /v1/license/validate` does.
pub fn billing_status_to_license_info(
    project_id: &str,
    response: &BillingStatusResponse,
    now_rfc3339: &str,
) -> WorkspaceLicenseInfo {
    WorkspaceLicenseInfo {
        project_id: project_id.to_string(),
        name: String::new(),
        status: response.status.clone(),
        is_owner: response.status != "collaborator",
        status_updated_at: now_rfc3339.to_string(),
    }
}

/// Fires `workspace_upgrade_prompted` (checklist 24.3.6a, pulled forward from
/// 24.4.11c -- see CLAUDE.md Security Rule 6's exception list) when a wizard
/// completion's billing-status check comes back `paid_required`. `workspace_
/// upgraded`, the other half of 24.4.11c, is a separate follow-up.
pub(crate) fn record_workspace_upgrade_prompted(state: &crate::app_state::AppState, consent: bool, project_id: &str) {
    state.telemetry.record(consent, "workspace_upgrade_prompted", serde_json::json!({ "project_id": project_id }));
}

/// Fires `workspace_upgraded` (checklist 24.4.11c, the other half of
/// `workspace_upgrade_prompted` above) when 24.4.2's subscribe flow
/// succeeds.
pub(crate) fn record_workspace_upgraded(state: &crate::app_state::AppState, consent: bool, project_id: &str) {
    state.telemetry.record(consent, "workspace_upgraded", serde_json::json!({ "project_id": project_id }));
}

/// Re-fetches billing-status for the workspace named in a `postlane://
/// billing-complete` deep link (24.4.5a) and fires `workspace_upgraded` when
/// it now reads `paid_owned`. The deep link fires on both Checkout success
/// and cancel (Stripe is given the same return URL for both) and on the
/// third+-workspace direct-quantity-update path, which never opens a
/// browser at all -- a fresh status check is the only reliable signal that
/// distinguishes "just upgraded" from "checkout cancelled, nothing
/// changed," since neither the deep link URL nor the original subscribe
/// response carries that outcome.
pub async fn record_billing_complete_upgrade_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    state: &crate::app_state::AppState,
    consent: bool,
) -> Result<(), String> {
    let response = get_billing_status_with_client(project_id, client, base_url, token).await?;
    if response.status == "paid_owned" {
        record_workspace_upgraded(state, consent, project_id);
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

/// Called by `WorkspaceSetupWizard`'s Step 6 immediately after `setup_workspace`
/// succeeds (checklist 24.3.6a) -- a separate IPC round-trip so a billing-status
/// failure never masks an otherwise-successful workspace creation, and can be
/// retried on its own. Syncs the result onto the new workspace's `license_status`
/// and fires `workspace_upgrade_prompted` when the workspace needs to be paid for.
#[tauri::command]
pub async fn get_workspace_billing_status(
    project_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<BillingStatusResponse, String> {
    let token = resolve_token(&app).await?;
    let response = get_billing_status_with_client(&project_id, &build_client(), POSTLANE_API_BASE, &token).await?;

    let now = chrono::Utc::now().to_rfc3339();
    let info = billing_status_to_license_info(&project_id, &response, &now);
    crate::workspace_license_sync::sync_license_statuses(std::slice::from_ref(&info))?;

    if response.status == "paid_required" {
        let consent = crate::app_state::read_app_state().telemetry_consent;
        record_workspace_upgrade_prompted(&state, consent, &project_id);
    }

    Ok(response)
}

/// Called by the frontend's `postlane://billing-complete` deep-link handler
/// (24.4.5a, `ProjectsProvider.tsx`) once it has the `project_id` from the
/// URL. See `record_billing_complete_upgrade_with_client` for why this needs
/// its own fresh status check rather than reusing the deep link's existing
/// workspace-list refresh.
#[tauri::command]
pub async fn record_billing_complete_upgrade(
    project_id: String,
    state: tauri::State<'_, crate::app_state::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    let consent = crate::app_state::read_app_state().telemetry_consent;
    record_billing_complete_upgrade_with_client(
        &project_id,
        &build_client(),
        POSTLANE_API_BASE,
        &token,
        &state,
        consent,
    )
    .await
}

#[cfg(test)]
#[path = "workspace_billing_tests.rs"]
mod tests;
