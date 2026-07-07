// SPDX-License-Identifier: BUSL-1.1
// Tests for the approval-block gate (checklist 24.4.11): approve_post_impl
// rejects when the owning workspace's license_status is inactive/
// payment_failed/unlicensed, with CTA info that differs by status and by
// whether the caller is the workspace owner or a collaborator.

use super::*;
use crate::storage::ReposConfig;
use crate::workspace_entry::WorkspaceEntry;
use crate::workspace_repos::{write_workspace_repos, RepoEntry, WorkspaceReposConfig};

fn make_state_with_license(
    workspace_path: &str,
    child_path: &str,
    posts_dir: &str,
    license_status: Option<&str>,
    is_owner: Option<bool>,
    status_updated_at: Option<&str>,
) -> AppState {
    let ws_path = std::path::Path::new(workspace_path);
    let ws_repos = WorkspaceReposConfig {
        version: 1,
        repos: vec![RepoEntry {
            id: "r1".to_string(),
            name: "frontend".to_string(),
            path: child_path.to_string(),
            posts_dir: posts_dir.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
        }],
    };
    write_workspace_repos(&ws_path.join("repos.json"), &ws_repos).expect("write ws repos");

    let config = ReposConfig {
        version: 2,
        workspaces: vec![WorkspaceEntry {
            id: "ws-1".to_string(),
            name: "myorg".to_string(),
            workspace_path: workspace_path.to_string(),
            active: true,
            added_at: "2026-01-01T00:00:00Z".to_string(),
            license_status: license_status.map(|s| s.to_string()),
            is_owner,
            status_updated_at: status_updated_at.map(|s| s.to_string()),
        }],
        repos: vec![],
    };
    let repos_path = std::env::temp_dir().join(format!("repos_license_gate_{}_{}.json", std::process::id(), rand_suffix()));
    AppState::new_with_path(config, repos_path)
}

// Avoids colliding repos_path files across parallel tests without depending
// on a real RNG crate -- a monotonically-increasing counter is sufficient.
fn rand_suffix() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn write_post(workspace_path: &std::path::Path, posts_dir: &str, post_folder: &str) {
    let post_path = workspace_path.join("posts").join(posts_dir).join(post_folder);
    std::fs::create_dir_all(&post_path).expect("create post dir");
    std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
    std::fs::write(post_path.join("x.md"), "content").expect("write x.md");
}

struct Setup {
    _ws: tempfile::TempDir,
    canonical_child: std::path::PathBuf,
    state: AppState,
}

fn setup(license_status: Option<&str>, is_owner: Option<bool>, status_updated_at: Option<&str>) -> Setup {
    let ws = tempfile::TempDir::new().expect("create ws dir");
    let child = ws.path().join("frontend");
    std::fs::create_dir_all(&child).expect("create child dir");
    let canonical_child = std::fs::canonicalize(&child).expect("canonicalize child");
    let canonical_ws = std::fs::canonicalize(ws.path()).expect("canonicalize ws");

    let state = make_state_with_license(
        canonical_ws.to_str().unwrap(),
        canonical_child.to_str().unwrap(),
        "frontend",
        license_status,
        is_owner,
        status_updated_at,
    );
    write_post(&canonical_ws, "frontend", "my-post");
    Setup { _ws: ws, canonical_child, state }
}

async fn approve(setup: &Setup) -> Result<(), ApproveError> {
    approve_post_impl(setup.canonical_child.to_str().unwrap(), "my-post", "x", &setup.state, None, false).await
}

#[tokio::test]
async fn test_inactive_workspace_blocks_approvals() {
    let s = setup(Some("inactive"), Some(true), None);
    let result = approve(&s).await;
    assert!(matches!(result, Err(ApproveError::Blocked { ref status, .. }) if status == "inactive"));
}

#[tokio::test]
async fn test_inactive_reactivate_cta_owner_only() {
    let owner = setup(Some("inactive"), Some(true), None);
    match approve(&owner).await {
        Err(ApproveError::Blocked { is_owner, .. }) => assert!(is_owner, "owner must see is_owner: true"),
        other => panic!("expected Blocked, got {:?}", other),
    }

    let collaborator = setup(Some("inactive"), Some(false), None);
    match approve(&collaborator).await {
        Err(ApproveError::Blocked { is_owner, .. }) => assert!(!is_owner, "collaborator must see is_owner: false"),
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_collaborator_detection() {
    // is_owner: None (never synced yet) must not be treated as owner --
    // defaults closed (no reactivate/manage-billing CTA) rather than open.
    let s = setup(Some("inactive"), None, None);
    match approve(&s).await {
        Err(ApproveError::Blocked { is_owner, .. }) => assert!(!is_owner, "unknown ownership must default to non-owner"),
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_unlicensed_shows_transfer_cta() {
    let s = setup(Some("unlicensed"), Some(false), None);
    let result = approve(&s).await;
    assert!(matches!(result, Err(ApproveError::Blocked { ref status, .. }) if status == "unlicensed"));
}

#[tokio::test]
async fn test_payment_failed_owner_copy_discloses_days_remaining() {
    let updated_at = (chrono::Utc::now() - chrono::Duration::days(3)).to_rfc3339();
    let s = setup(Some("payment_failed"), Some(true), Some(&updated_at));
    match approve(&s).await {
        Err(ApproveError::Blocked { status, is_owner, days_remaining }) => {
            assert_eq!(status, "payment_failed");
            assert!(is_owner);
            assert_eq!(days_remaining, Some(11), "14 - 3 elapsed days = 11 remaining");
        }
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_payment_failed_days_remaining_clamped_to_zero_past_deadline() {
    let updated_at = (chrono::Utc::now() - chrono::Duration::days(20)).to_rfc3339();
    let s = setup(Some("payment_failed"), Some(true), Some(&updated_at));
    match approve(&s).await {
        Err(ApproveError::Blocked { days_remaining, .. }) => assert_eq!(days_remaining, Some(0)),
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_days_remaining_is_none_for_non_payment_failed_statuses() {
    let s = setup(Some("inactive"), Some(true), Some("2026-06-01T00:00:00Z"));
    match approve(&s).await {
        Err(ApproveError::Blocked { days_remaining, .. }) => assert_eq!(days_remaining, None),
        other => panic!("expected Blocked, got {:?}", other),
    }
}

#[tokio::test]
async fn test_healthy_workspace_status_does_not_block_approval() {
    for status in ["free_owned", "paid_owned", "paid_required", "owner_departing", "collaborator"] {
        let s = setup(Some(status), Some(true), None);
        let result = approve(&s).await;
        assert!(result.is_ok(), "status {} must not block approval, got {:?}", status, result);
    }
}

#[tokio::test]
async fn test_no_license_status_yet_does_not_block_approval() {
    // Workspace entry predates 24.4.8, or no license check has run yet.
    let s = setup(None, None, None);
    let result = approve(&s).await;
    assert!(result.is_ok(), "missing license_status must not block approval: {:?}", result);
}

#[tokio::test]
async fn test_legacy_repo_is_never_blocked_by_license_status() {
    // Legacy (pre-workspace) repos have no license_status concept at all.
    let dir = tempfile::TempDir::new().expect("tmp dir");
    let repo_dir = dir.path().join("legacy-repo");
    std::fs::create_dir_all(&repo_dir).expect("create repo dir");
    let canonical = std::fs::canonicalize(&repo_dir).expect("canonicalize");
    let canonical_str = canonical.to_str().unwrap().to_string();

    let state = crate::test_fixtures::make_state(vec![crate::storage::Repo {
        id: "r1".to_string(),
        name: "legacy".to_string(),
        path: canonical_str.clone(),
        active: true,
        added_at: "2026-01-01T00:00:00Z".to_string(),
    }]);
    let post_path = canonical.join(".postlane/posts").join("my-post");
    std::fs::create_dir_all(&post_path).expect("create post dir");
    std::fs::write(post_path.join("meta.json"), "{}").expect("write meta");
    std::fs::write(post_path.join("x.md"), "content").expect("write x.md");

    let result = approve_post_impl(&canonical_str, "my-post", "x", &state, None, false).await;
    assert!(result.is_ok(), "legacy repos must never be gated on license_status: {:?}", result);
}
