// SPDX-License-Identifier: BUSL-1.1

//! Project registry: public types, constants, and re-exports.
//!
//! Actual commands are split across focused sub-modules:
//! - `project_config_ops` — local file read/write
//! - `project_lifecycle`  — create/delete/register
//! - `project_billing`    — status and billing gate checks
//! - `project_voice_guide` — voice guide read/write

pub use crate::project_api::{
    check_billing_gate_with_client, check_project_status_with_client,
    create_project_with_client, list_projects_with_client,
    update_project_org_login_with_client,
};

// Re-export commands from sub-modules so `lib.rs` can still reference them
// as `project_registry::*` without changing the Tauri handler list.
pub use crate::project_billing::{check_billing_gate, check_project_status};
pub use crate::project_config_ops::{
    get_repo_remote_name, read_project_id_from_path, write_project_id_to_config,
};
pub use crate::project_lifecycle::{
    create_project, delete_project, list_projects, register_repo_with_project,
    update_project_org_login,
};
pub use crate::project_voice_guide::{
    get_project_voice_guide, get_voice_guide_fields, save_project_voice_guide,
};

use serde::Deserialize;

// ── Public types ─────────────────────────────────────────────────────────────

/// Lightweight project summary returned by `list_projects`.
#[derive(serde::Serialize, Deserialize, Clone, Debug)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub workspace_type: String,
    pub tier: String,
    pub billing_active: bool,
    pub is_owner: bool,
    #[serde(default)]
    pub provider_org_login: Option<String>,
}

/// Outcome of checking whether the current user owns a project.
#[derive(Debug, PartialEq)]
pub enum ProjectStatus {
    Owned,
    NotFound,
    Offline,
}

/// Billing gate state for the current account.
#[derive(Debug, PartialEq)]
pub enum BillingGate {
    Free,
    None,
    Offline,
}

/// Errors that can occur when creating a project.
#[derive(Debug)]
pub enum CreateProjectError {
    InvalidName(String),
    InvalidWorkspaceType(String),
    NoFreeSlot,
    OrgAlreadyRegistered,
    NoLicenseToken,
    Backend(String),
}

impl std::fmt::Display for CreateProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(msg) => write!(f, "Invalid project name: {}", msg),
            Self::InvalidWorkspaceType(t) => write!(f, "Invalid workspace type: '{}'. Must be personal, organization, or client", t),
            Self::NoFreeSlot => write!(f, "No free project slot. Subscribe at postlane.dev/billing"),
            Self::OrgAlreadyRegistered => write!(f, "This GitHub/GitLab organisation is already linked to a Postlane project"),
            Self::NoLicenseToken => write!(f, "No license token — sign in at postlane.dev/login"),
            Self::Backend(msg) => write!(f, "Backend error: {}", msg),
        }
    }
}

/// Canonical error returned to the frontend when any web API command receives HTTP 401.
/// The TypeScript `src/ipc/invoke.ts` wrapper detects this string and navigates to
/// AccountSettingsView. All web API commands must use this constant — never return a
/// free-form "401" string that the wrapper won't detect.
///
/// Auth token pattern (M19):
///   - Token stored in OS keyring under service "postlane", account "license"
///   - Retrieved via `app.keyring().get_password("postlane", "license")`
///   - Passed as `Bearer {token}` via `client.bearer_auth(token)`
///   - On HTTP 401: return `Err(SESSION_EXPIRED_ERROR.to_string())`
///   - No automatic refresh in v1 (no refresh token flow)
pub const SESSION_EXPIRED_ERROR: &str = "session_expired";

/// Returns `Ok(token)` when `opt` is `Some`, otherwise returns a user-facing
/// error message prompting the user to sign in.
pub fn require_license_token(opt: Option<String>) -> Result<String, String> {
    opt.ok_or_else(|| "No license token — sign in at postlane.dev/login".to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_require_license_token_returns_err_for_none() {
        let result = require_license_token(None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("sign in"), "error should mention sign-in");
    }

    #[test]
    fn test_require_license_token_returns_token_for_some() {
        let result = require_license_token(Some("tok-123".to_string()));
        assert_eq!(result.expect("require_license_token should return Ok for Some"), "tok-123");
    }
}
