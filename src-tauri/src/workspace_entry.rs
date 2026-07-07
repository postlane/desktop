// SPDX-License-Identifier: BUSL-1.1

//! `WorkspaceEntry` — the per-workspace record stored in the `workspaces` array
//! of `~/.postlane/repos.json` (v2 schema).
//!
//! Distinct from `RepoEntry` (`{workspace}/repos.json`) which is owned by the CLI
//! and carries the per-workspace child-repo list with `posts_dir` fields.

use serde::{Deserialize, Serialize};

/// A workspace registration in `~/.postlane/repos.json` (v2 schema).
///
/// One workspace = one billing unit. The `id` matches the `project_id` in
/// `{workspace}/config.json` so the desktop can correlate the two.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkspaceEntry {
    pub id: String,
    pub name: String,
    /// Absolute path to the workspace root directory on disk.
    pub workspace_path: String,
    pub active: bool,
    pub added_at: String,
    /// v2.0 billing status (checklist 24.4.8): one of free_owned/paid_owned/
    /// paid_required/collaborator/inactive/payment_failed/owner_departing/
    /// unlicensed. `None` until the first successful license check after
    /// this field was introduced. Updated by `workspace_license_sync` on
    /// every successful `POST /v1/license/validate` call.
    #[serde(default)]
    pub license_status: Option<String>,
    /// Whether the current user owns this workspace vs. collaborates on it
    /// (checklist 24.4.8/24.4.11) -- `license_status` alone can't distinguish
    /// this for a collaborator on an unhealthy workspace, since that status
    /// passes through unmasked so the approval-block CTA (24.4.11) knows
    /// what's actually wrong. `None` until the first successful license
    /// check after this field was introduced.
    #[serde(default)]
    pub is_owner: Option<bool>,
    /// ISO 8601 timestamp of when `license_status` last changed -- used to
    /// compute the days-remaining countdown in the payment_failed approval
    /// block CTA (checklist 24.4.11). `None` until the first successful
    /// license check after this field was introduced.
    #[serde(default)]
    pub status_updated_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_entry_roundtrips_through_json() {
        let entry = WorkspaceEntry {
            id: "ws-id-abc".to_string(),
            name: "postlane".to_string(),
            workspace_path: "/Users/hugo/code/myorg/postlane".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
            license_status: None,
            is_owner: None,
            status_updated_at: None,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: WorkspaceEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, entry);
    }

    #[test]
    fn test_workspace_entry_deserializes_from_minimal_json() {
        let json = r#"{
            "id": "ws-1",
            "name": "myorg",
            "workspace_path": "/code/myorg",
            "active": true,
            "added_at": "2026-05-01T00:00:00Z"
        }"#;
        let entry: WorkspaceEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.id, "ws-1");
        assert_eq!(entry.name, "myorg");
        assert_eq!(entry.workspace_path, "/code/myorg");
        assert!(entry.active);
    }

    #[test]
    fn test_workspace_entry_inactive_roundtrips() {
        let entry = WorkspaceEntry {
            id: "ws-2".to_string(),
            name: "archived".to_string(),
            workspace_path: "/old/path".to_string(),
            active: false,
            added_at: "2026-01-01T00:00:00Z".to_string(),
            license_status: None,
            is_owner: None,
            status_updated_at: None,
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: WorkspaceEntry = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.active);
    }

    #[test]
    fn test_workspace_entry_deserializes_without_license_status_field() {
        // Pre-24.4.8 repos.json files never had this field — must default to
        // None rather than fail to parse (no absent-field fallback applies
        // to the API response, not to reading an older on-disk file).
        let json = r#"{
            "id": "ws-3",
            "name": "myorg",
            "workspace_path": "/code/myorg",
            "active": true,
            "added_at": "2026-05-01T00:00:00Z"
        }"#;
        let entry: WorkspaceEntry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(entry.license_status, None);
    }

    #[test]
    fn test_workspace_entry_roundtrips_with_license_status_set() {
        let entry = WorkspaceEntry {
            id: "ws-4".to_string(),
            name: "postlane".to_string(),
            workspace_path: "/code/postlane".to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
            license_status: Some("payment_failed".to_string()),
            is_owner: Some(true),
            status_updated_at: Some("2026-01-01T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: WorkspaceEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.license_status.as_deref(), Some("payment_failed"));
    }
}
