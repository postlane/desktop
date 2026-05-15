// SPDX-License-Identifier: BUSL-1.1

//! Tauri commands for checking project status and billing gate.

use crate::license::POSTLANE_API_BASE;
use crate::project_api::{check_billing_gate_with_client, check_project_status_with_client};
use crate::project_registry::{require_license_token, BillingGate, ProjectStatus};
use crate::providers::scheduling::build_client;
use tauri_plugin_keyring::KeyringExt;

/// Tauri command: checks whether the user owns the given project.
/// Returns `"owned"`, `"not_found"`, or `"offline"`.
#[tauri::command]
pub async fn check_project_status(
    project_id: String,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let status = check_project_status_with_client(&project_id, &client, POSTLANE_API_BASE, &token).await;
    Ok(match status {
        ProjectStatus::Owned => "owned".to_string(),
        ProjectStatus::NotFound => "not_found".to_string(),
        ProjectStatus::Offline => "offline".to_string(),
    })
}

/// Tauri command: checks the billing gate for the current account.
/// Returns `"free"`, `"none"`, or `"offline"`.
#[tauri::command]
pub async fn check_billing_gate(app: tauri::AppHandle) -> Result<String, String> {
    let token = require_license_token(
        app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?
    )?;
    let client = build_client();
    let gate = check_billing_gate_with_client(&client, POSTLANE_API_BASE, &token).await;
    Ok(match gate {
        BillingGate::Free => "free".to_string(),
        BillingGate::None => "none".to_string(),
        BillingGate::Offline => "offline".to_string(),
    })
}
