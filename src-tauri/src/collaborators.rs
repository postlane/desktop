// SPDX-License-Identifier: BUSL-1.1
//! Tauri commands for checklist 24.4.14/24.4.14a — Manage Collaborators
//! (list/promote/demote/remove a workspace's project_collaborators rows).

use crate::license::POSTLANE_API_BASE;
use crate::project_registry::require_license_token;
use crate::providers::scheduling::build_client;
use serde::{Deserialize, Serialize};
use tauri_plugin_keyring::KeyringExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollaboratorInfo {
    pub user_id: String,
    pub role: String,
    pub added_at: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct ListCollaboratorsResponse {
    collaborators: Vec<CollaboratorInfo>,
}

async fn resolve_token(app: &tauri::AppHandle) -> Result<String, String> {
    require_license_token(app.keyring().get_password("postlane", "license").map_err(|e| e.to_string())?)
}

pub async fn list_project_collaborators_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<CollaboratorInfo>, String> {
    let url = format!("{}/v1/projects/{}/collaborators", base_url, project_id);
    let resp = client.get(&url).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    let body: ListCollaboratorsResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(body.collaborators)
}

pub async fn update_collaborator_role_with_client(
    project_id: &str,
    user_id: &str,
    role: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/projects/{}/collaborators/{}", base_url, project_id, user_id);
    let resp = client
        .patch(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "role": role }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}

pub async fn remove_collaborator_with_client(
    project_id: &str,
    user_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    let url = format!("{}/v1/projects/{}/collaborators/{}", base_url, project_id, user_id);
    let resp = client.delete(&url).bearer_auth(token).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server returned {}", resp.status()));
    }
    Ok(())
}

#[tauri::command]
pub async fn list_project_collaborators(project_id: String, app: tauri::AppHandle) -> Result<Vec<CollaboratorInfo>, String> {
    let token = resolve_token(&app).await?;
    list_project_collaborators_with_client(&project_id, &build_client(), POSTLANE_API_BASE, &token).await
}

#[tauri::command]
pub async fn update_collaborator_role(
    project_id: String,
    user_id: String,
    role: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    update_collaborator_role_with_client(&project_id, &user_id, &role, &build_client(), POSTLANE_API_BASE, &token).await
}

#[tauri::command]
pub async fn remove_project_collaborator(project_id: String, user_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let token = resolve_token(&app).await?;
    remove_collaborator_with_client(&project_id, &user_id, &build_client(), POSTLANE_API_BASE, &token).await
}

#[cfg(test)]
#[path = "collaborators_tests.rs"]
mod tests;
