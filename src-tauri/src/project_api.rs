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
mod tests {
    use super::*;
    use crate::project_registry::{
        BillingGate, CreateProjectError, ProjectStatus, SESSION_EXPIRED_ERROR,
    };
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;

    fn build_test_client() -> reqwest::Client {
        build_client()
    }

    // ── check_project_status ─────────────────────────────────────────────────

    #[tokio::test]
    async fn test_returns_owned_for_200_owned_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/proj-123");
            then.status(200)
                .json_body(serde_json::json!({ "status": "owned", "tier": "free" }));
        });

        let status = check_project_status_with_client(
            "proj-123",
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert_eq!(status, ProjectStatus::Owned);
    }

    #[tokio::test]
    async fn test_returns_not_found_for_404_response() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/proj-456");
            then.status(404)
                .json_body(serde_json::json!({ "id": "proj-456", "status": "not_found" }));
        });

        let status = check_project_status_with_client(
            "proj-456",
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert_eq!(status, ProjectStatus::NotFound);
    }

    #[tokio::test]
    async fn test_returns_offline_on_network_error() {
        let status = check_project_status_with_client(
            "proj-789",
            &build_test_client(),
            "http://127.0.0.1:19998",
            "tok",
        )
        .await;
        assert_eq!(status, ProjectStatus::Offline);
    }

    // ── check_billing_gate ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_gate_returns_free_for_new_user() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/gate");
            then.status(200)
                .json_body(serde_json::json!({ "slot": "free" }));
        });

        let gate =
            check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(gate, BillingGate::Free);
    }

    #[tokio::test]
    async fn test_gate_returns_none_when_no_free_slot() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects/gate");
            then.status(200)
                .json_body(serde_json::json!({ "slot": "none" }));
        });

        let gate =
            check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert_eq!(gate, BillingGate::None);
    }

    #[tokio::test]
    async fn test_gate_returns_offline_on_network_error() {
        let gate =
            check_billing_gate_with_client(&build_test_client(), "http://127.0.0.1:19997", "tok")
                .await;
        assert_eq!(gate, BillingGate::Offline);
    }

    // ── create_project ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_creates_project_returns_id() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "project_id": "new-uuid-abc", "name": "My Project", "tier": "free",
                "workspace_type": "personal"
            }));
        });

        let result = create_project_with_client(
            "My Project",
            "personal",
            None,
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        let (id, name, _wt) =
            result.expect("create_project_with_client should succeed for 200 response");
        assert_eq!(id, "new-uuid-abc");
        assert_eq!(name, "My Project");
    }

    #[tokio::test]
    async fn test_create_project_rejects_empty_name_before_network_call() {
        let result = create_project_with_client(
            "",
            "personal",
            None,
            None,
            &build_test_client(),
            "http://127.0.0.1:19996",
            "tok",
        )
        .await;
        assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
    }

    #[tokio::test]
    async fn test_create_project_rejects_whitespace_only_name() {
        let result = create_project_with_client(
            "   ",
            "personal",
            None,
            None,
            &build_test_client(),
            "http://127.0.0.1:19996",
            "tok",
        )
        .await;
        assert!(matches!(result, Err(CreateProjectError::InvalidName(_))));
    }

    #[tokio::test]
    async fn test_create_project_returns_error_on_402() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(402)
                .json_body(serde_json::json!({ "error": "no_free_slot" }));
        });

        let result = create_project_with_client(
            "Second Project",
            "personal",
            None,
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(matches!(result, Err(CreateProjectError::NoFreeSlot)));
    }

    #[tokio::test]
    async fn test_create_project_passes_workspace_type() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1/projects")
                .body_contains("\"workspace_type\":\"organization\"");
            then.status(200).json_body(serde_json::json!({
                "project_id": "org-uuid-abc", "name": "Acme", "tier": "free",
                "workspace_type": "organization"
            }));
        });

        let result = create_project_with_client(
            "Acme",
            "organization",
            None,
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        let (id, name, wt) =
            result.expect("create_project with organization workspace_type should succeed");
        assert_eq!(id, "org-uuid-abc");
        assert_eq!(name, "Acme");
        assert_eq!(wt, "organization");
    }

    #[tokio::test]
    async fn test_create_project_rejects_invalid_workspace_type() {
        let result = create_project_with_client(
            "Acme",
            "enterprise",
            None,
            None,
            &build_test_client(),
            "http://127.0.0.1:19994",
            "tok",
        )
        .await;
        assert!(matches!(
            result,
            Err(CreateProjectError::InvalidWorkspaceType(_))
        ));
    }

    #[tokio::test]
    async fn test_create_project_sends_provider_org_login_in_body() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1/projects")
                .body_contains("\"provider_org_login\":\"postlane\"");
            then.status(200).json_body(serde_json::json!({
                "project_id": "org-proj-uuid", "name": "postlane", "tier": "free",
                "workspace_type": "organization"
            }));
        });

        let result = create_project_with_client(
            "postlane",
            "organization",
            Some("postlane"),
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(
            result.is_ok(),
            "should send provider_org_login and succeed: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_create_project_sends_provider_group_path_in_body() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .path("/v1/projects")
                .body_contains("\"provider_group_path\":\"acme-corp\"");
            then.status(200).json_body(serde_json::json!({
                "project_id": "gl-proj-uuid", "name": "acme-corp", "tier": "free",
                "workspace_type": "organization"
            }));
        });

        let result = create_project_with_client(
            "acme-corp",
            "organization",
            None,
            Some("acme-corp"),
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(
            result.is_ok(),
            "should send provider_group_path and succeed: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_create_project_returns_org_already_registered_on_409() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(409)
                .json_body(serde_json::json!({ "error": "org_already_registered" }));
        });

        let result = create_project_with_client(
            "postlane",
            "organization",
            Some("postlane"),
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(
            matches!(result, Err(CreateProjectError::OrgAlreadyRegistered)),
            "HTTP 409 must map to OrgAlreadyRegistered, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_create_project_with_org_login_does_not_require_name() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "project_id": "org-uuid", "name": "postlane", "tier": "free",
                "workspace_type": "organization"
            }));
        });

        let result = create_project_with_client(
            "",
            "organization",
            Some("postlane"),
            None,
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(
            result.is_ok(),
            "empty name with org_login should not fail validation: {:?}",
            result
        );
    }

    // ── list_projects ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_projects_returns_vec_on_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "projects": [{
                    "id": "proj-1",
                    "name": "Postlane",
                    "workspace_type": "organization",
                    "tier": "free",
                    "billing_active": true,
                    "is_owner": true
                }]
            }));
        });

        let result =
            list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
        let projects =
            result.expect("list_projects_with_client should succeed for 200 response");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "proj-1");
        assert_eq!(projects[0].name, "Postlane");
        assert_eq!(projects[0].workspace_type, "organization");
        assert_eq!(projects[0].tier, "free");
        assert!(projects[0].billing_active);
        assert!(projects[0].is_owner);
    }

    #[tokio::test]
    async fn test_list_projects_returns_error_on_http_failure() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(503);
        });

        let result =
            list_projects_with_client(&build_test_client(), &server.base_url(), "tok").await;
        assert!(result.is_err(), "HTTP 503 must return Err");
    }

    #[tokio::test]
    async fn test_list_projects_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(401);
        });

        let result =
            list_projects_with_client(&build_test_client(), &server.base_url(), "expired-tok")
                .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            SESSION_EXPIRED_ERROR,
            "HTTP 401 must return session_expired error"
        );
    }

    // ── update_project_org_login ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_org_login_returns_ok_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/projects/proj-abc");
            then.status(200).json_body(serde_json::json!({ "ok": true }));
        });

        let result = update_project_org_login_with_client(
            "proj-abc",
            "my-org",
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(result.is_ok(), "200 response must map to Ok(()), got: {:?}", result);
    }

    #[tokio::test]
    async fn test_update_org_login_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/projects/proj-abc");
            then.status(401);
        });

        let result = update_project_org_login_with_client(
            "proj-abc",
            "my-org",
            &build_test_client(),
            &server.base_url(),
            "expired-tok",
        )
        .await;
        assert_eq!(result.unwrap_err(), SESSION_EXPIRED_ERROR, "401 must return session expired");
    }

    #[tokio::test]
    async fn test_update_org_login_returns_err_on_500() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/projects/proj-abc");
            then.status(500);
        });

        let result = update_project_org_login_with_client(
            "proj-abc",
            "my-org",
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(result.is_err(), "non-200 must return Err");
    }

    #[tokio::test]
    async fn test_update_org_login_sends_provider_org_login_in_body() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH)
                .path("/v1/projects/proj-abc")
                .body_contains("\"provider_org_login\":\"acme\"");
            then.status(200).json_body(serde_json::json!({ "ok": true }));
        });

        let result = update_project_org_login_with_client(
            "proj-abc",
            "acme",
            &build_test_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(result.is_ok(), "patch body must contain provider_org_login field");
    }

    #[tokio::test]
    async fn test_update_org_login_rejects_invalid_project_id() {
        let result = update_project_org_login_with_client(
            "../../etc/passwd",
            "my-org",
            &build_test_client(),
            "http://127.0.0.1:19993",
            "tok",
        )
        .await;
        assert!(result.is_err(), "invalid project_id must be rejected before network call");
    }
}
