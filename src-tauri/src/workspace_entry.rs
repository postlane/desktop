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
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: WorkspaceEntry = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.active);
    }
}
