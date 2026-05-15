// SPDX-License-Identifier: BUSL-1.1

use crate::project_registry::{
    BillingGate, CreateProjectError, ProjectStatus, ProjectSummary, SESSION_EXPIRED_ERROR,
};
use crate::project_validation::validate_project_id;
use serde::Deserialize;

// ── Response shapes ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProjectStatusResponse {
    status: String,
}

#[derive(Deserialize)]
struct BillingGateResponse {
    slot: String,
}

#[derive(Deserialize)]
struct CreateProjectResponse {
    project_id: String,
    name: String,
    workspace_type: String,
}

#[derive(Deserialize)]
struct ListProjectsResponse {
    projects: Vec<ProjectSummary>,
}

const VALID_WORKSPACE_TYPES: &[&str] = &["personal", "organization", "client"];

// ── HTTP client functions ─────────────────────────────────────────────────────

/// Calls `GET {base_url}/v1/projects/{project_id}` and maps the response to a `ProjectStatus`.
pub async fn check_project_status_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> ProjectStatus {
    if validate_project_id(project_id).is_err() {
        return ProjectStatus::NotFound;
    }
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let resp = client.get(&url).bearer_auth(token).send().await;
    match resp {
        Ok(r) if r.status().is_success() => {
            match r.json::<ProjectStatusResponse>().await {
                Ok(body) if body.status == "owned" => ProjectStatus::Owned,
                _ => ProjectStatus::NotFound,
            }
        }
        Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 404 => {
            ProjectStatus::NotFound
        }
        _ => ProjectStatus::Offline,
    }
}

/// Calls `GET {base_url}/v1/projects/gate` and maps the response to a `BillingGate`.
pub async fn check_billing_gate_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> BillingGate {
    let url = format!("{}/v1/projects/gate", base_url);
    let resp = client.get(&url).bearer_auth(token).send().await;
    match resp {
        Ok(r) if r.status().is_success() => match r.json::<BillingGateResponse>().await {
            Ok(body) if body.slot == "free" => BillingGate::Free,
            Ok(_) => BillingGate::None,
            Err(_) => BillingGate::Offline,
        },
        _ => BillingGate::Offline,
    }
}

/// Calls `POST {base_url}/v1/projects` with name, workspace_type, and optional org identifiers.
/// Returns `(project_id, name, workspace_type)` on success.
/// When `provider_org_login` is supplied, name may be empty — the API derives it from the login.
pub async fn create_project_with_client(
    name: &str,
    workspace_type: &str,
    provider_org_login: Option<&str>,
    provider_group_path: Option<&str>,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(String, String, String), CreateProjectError> {
    let trimmed = name.trim();
    if provider_org_login.is_none() {
        if trimmed.is_empty() {
            return Err(CreateProjectError::InvalidName(
                "name cannot be empty".to_string(),
            ));
        }
        if trimmed.len() > 100 {
            return Err(CreateProjectError::InvalidName(
                "name cannot exceed 100 characters".to_string(),
            ));
        }
    }
    if !VALID_WORKSPACE_TYPES.contains(&workspace_type) {
        return Err(CreateProjectError::InvalidWorkspaceType(
            workspace_type.to_string(),
        ));
    }
    let mut body = serde_json::json!({ "name": trimmed, "workspace_type": workspace_type });
    if let Some(login) = provider_org_login {
        body["provider_org_login"] = serde_json::json!(login);
    }
    if let Some(path) = provider_group_path {
        body["provider_group_path"] = serde_json::json!(path);
    }
    let url = format!("{}/v1/projects", base_url);
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| CreateProjectError::Backend(e.to_string()))?;
    match resp.status().as_u16() {
        200 => {
            let parsed: CreateProjectResponse = resp
                .json()
                .await
                .map_err(|e| CreateProjectError::Backend(e.to_string()))?;
            Ok((parsed.project_id, parsed.name, parsed.workspace_type))
        }
        402 => Err(CreateProjectError::NoFreeSlot),
        409 => Err(CreateProjectError::OrgAlreadyRegistered),
        _ => Err(CreateProjectError::Backend(format!(
            "unexpected status {}",
            resp.status()
        ))),
    }
}

/// Calls `GET {base_url}/v1/projects` and returns the project list.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn list_projects_with_client(
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Vec<ProjectSummary>, String> {
    let url = format!("{}/v1/projects", base_url);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Backend error: {}", e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(SESSION_EXPIRED_ERROR.to_string());
    }
    if !status.is_success() {
        return Err(format!("Backend returned {}", status));
    }
    resp.json::<ListProjectsResponse>()
        .await
        .map(|r| r.projects)
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Calls `PATCH {base_url}/v1/projects/{project_id}` to set `provider_org_login` on an
/// existing project. Used by the v1.2 upgrade flow for users who created their project
/// before the org-picker step existed.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn update_project_org_login_with_client(
    project_id: &str,
    org_login: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    if validate_project_id(project_id).is_err() {
        return Err(format!("Invalid project_id: {}", project_id));
    }
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let body = serde_json::json!({ "provider_org_login": org_login });
    let resp = client
        .patch(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Backend error: {}", e))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(SESSION_EXPIRED_ERROR.to_string());
    }
    if !status.is_success() {
        return Err(format!("Backend returned {}", status));
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod status_gate_tests;
#[cfg(test)]
mod create_tests;
#[cfg(test)]
mod list_update_tests;
