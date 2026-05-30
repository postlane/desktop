// SPDX-License-Identifier: BUSL-1.1

//! §22.7.4 — Account deletion implementation.
//!
//! Steps execute in order; each is idempotent:
//!   Pre-flight → 0 (snapshot) → 1 (projects) → 2 (GitHub) → 3 (GitLab)
//!   → 4 (keyring, done by command) → 5 (account/delete)
//!   → 6 (repos.json) → 7–8 (state files) → 9 (workspace dirs, optional)
//!   → 10 (clear session, done by command)

use std::path::{Path, PathBuf};
use crate::workspace_entry::WorkspaceEntry;

// ── Input / output types ──────────────────────────────────────────────────────

pub struct DeleteAccountParams {
    pub postlane_dir: PathBuf,
    pub api_base: String,
    pub token: String,
    pub project_ids: Vec<String>,
    pub project_ids_with_github_app: Vec<String>,
    pub gitlab_instance_url: Option<String>,
    pub delete_workspace_dirs: bool,
}

#[derive(Debug)]
pub struct DeleteAccountResult {
    pub workspace_snapshot: Vec<WorkspaceEntry>,
}

// ── Main orchestration function ───────────────────────────────────────────────

pub async fn delete_account_impl(
    params: DeleteAccountParams,
    client: &reqwest::Client,
    validate_url: fn(&str) -> Result<(), String>,
) -> Result<DeleteAccountResult, String> {
    let repos_path = params.postlane_dir.join("repos.json");

    // Pre-flight: verify session is still active server-side.
    preflight_session(&params.api_base, &params.token, client).await?;

    // Step 0: snapshot workspaces before any writes.
    let workspace_snapshot = snapshot_workspaces(&repos_path)?;

    // Step 1: delete all projects from the API.
    delete_all_projects(&params.api_base, &params.token, &params.project_ids, client).await?;

    // Step 2: disconnect GitHub App installations.
    disconnect_all_github_apps(
        &params.api_base, &params.token, &params.project_ids_with_github_app, client,
    ).await?;

    // Step 3: revoke GitLab token (non-fatal if SSRF blocked).
    if let Err(e) = revoke_gitlab_token(
        params.gitlab_instance_url.as_deref(), client, validate_url,
    ).await {
        log::warn!("{}", e);
    }

    // Step 5: delete Supabase account record.
    delete_account_record(&params.api_base, &params.token, client).await?;

    // Steps 6–8: wipe all local state files.
    wipe_postlane_files(&params.postlane_dir)?;

    // Step 9: delete workspace directories (only if checkbox is checked).
    let snapshot_for_step9 = if params.delete_workspace_dirs {
        workspace_snapshot.clone()
    } else {
        vec![]
    };
    delete_workspace_dirs(&snapshot_for_step9, &repos_path);

    Ok(DeleteAccountResult { workspace_snapshot })
}

// ── Pre-flight ────────────────────────────────────────────────────────────────

pub async fn preflight_session(
    api_base: &str,
    token: &str,
    client: &reqwest::Client,
) -> Result<(), String> {
    let url = format!("{}/v1/auth/session", api_base);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("Cannot verify your session. Check your connection and try again. ({})", e))?;
    match resp.status().as_u16() {
        200 => Ok(()),
        401 => Err("Your session has expired. Sign out and sign back in to continue.".to_string()),
        s => Err(format!("Cannot verify your session. Check your connection and try again. (HTTP {})", s)),
    }
}

// ── Step 0: snapshot ──────────────────────────────────────────────────────────

fn snapshot_workspaces(repos_path: &Path) -> Result<Vec<WorkspaceEntry>, String> {
    if !repos_path.exists() {
        return Ok(vec![]);
    }
    crate::storage::read_repos_with_recovery(repos_path)
        .map(|c| c.workspaces)
        .map_err(|e| format!("PL-DEL-005: Cannot read workspace registry: {:?}", e))
}

// ── Step 1 ────────────────────────────────────────────────────────────────────

pub async fn delete_all_projects(
    api_base: &str,
    token: &str,
    project_ids: &[String],
    client: &reqwest::Client,
) -> Result<(), String> {
    for id in project_ids {
        let url = format!("{}/v1/projects/{}", api_base, id);
        let resp = client.delete(&url).bearer_auth(token).send().await
            .map_err(|e| format!("PL-DEL-001: network error deleting project {}: {}", id, e))?;
        match resp.status().as_u16() {
            200 | 204 | 404 => {}
            s => return Err(format!("PL-DEL-001: server returned {} for project {}", s, id)),
        }
    }
    Ok(())
}

// ── Step 2 ────────────────────────────────────────────────────────────────────

pub async fn disconnect_all_github_apps(
    api_base: &str,
    token: &str,
    project_ids_with_app: &[String],
    _client: &reqwest::Client,
) -> Result<(), String> {
    for id in project_ids_with_app {
        crate::github_app::disconnect_github_app_impl(api_base, id, token)
            .await
            .map_err(|e| format!("GitHub App disconnect failed for {}: {}", id, e))?;
    }
    Ok(())
}

// ── Step 3 ────────────────────────────────────────────────────────────────────

pub async fn revoke_gitlab_token(
    instance_url: Option<&str>,
    client: &reqwest::Client,
    validate_url: fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    let url = match instance_url {
        None => return Ok(()),
        Some(u) => u,
    };
    let revoke_url = format!("{}/oauth/token", url.trim_end_matches('/'));
    validate_url(&revoke_url)
        .map_err(|e| format!("PL-DEL-003: GitLab revocation skipped — {}", e))?;
    let resp = client.delete(&revoke_url).send().await
        .map_err(|e| format!("PL-DEL-003: GitLab revocation request failed: {}", e))?;
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("PL-DEL-003: GitLab revocation returned {}", status))
    }
}

// ── Step 5 ────────────────────────────────────────────────────────────────────

async fn delete_account_record(
    api_base: &str,
    token: &str,
    client: &reqwest::Client,
) -> Result<(), String> {
    let url = format!("{}/v1/account/delete", api_base);
    let resp = client.post(&url).bearer_auth(token).send().await
        .map_err(|e| format!("PL-DEL-004: network error deleting account: {}", e))?;
    match resp.status().as_u16() {
        200 | 404 => Ok(()),
        401 => Err("PL-DEL-004: session invalid during account deletion".to_string()),
        s => Err(format!("PL-DEL-004: server returned {} for account deletion", s)),
    }
}

// ── Steps 6–8 ─────────────────────────────────────────────────────────────────

pub fn wipe_postlane_files(postlane_dir: &Path) -> Result<(), String> {
    use crate::storage::ReposConfig;

    // Step 6: write empty repos.json.
    let repos_path = postlane_dir.join("repos.json");
    let empty = ReposConfig { version: 2, workspaces: vec![], repos: vec![] };
    crate::storage::write_repos(&repos_path, &empty)
        .map_err(|e| format!("PL-DEL-004: failed to wipe repos.json: {:?}", e))?;

    // Steps 7–8: delete state files (silently ignore if absent).
    for name in &[
        "session.token", "local.token", "port",
        "wizard_state.json", "app_state.json",
    ] {
        let p = postlane_dir.join(name);
        if p.exists() {
            std::fs::remove_file(&p)
                .map_err(|e| format!("PL-DEL-004: failed to delete {}: {}", name, e))?;
        }
    }
    Ok(())
}

// ── Step 9 ────────────────────────────────────────────────────────────────────

pub fn delete_workspace_dirs(
    workspace_snapshot: &[WorkspaceEntry],
    repos_path: &Path,
) -> Vec<String> {
    let mut failures = Vec::new();
    for ws in workspace_snapshot {
        let ws_path = std::path::Path::new(&ws.workspace_path);
        match crate::workspace_disconnect::safelist_validate_delete_path(ws_path, repos_path) {
            Err(e) => failures.push(format!("{}: {}", ws.workspace_path, e)),
            Ok(canonical) => {
                if let Err(e) = std::fs::remove_dir_all(&canonical) {
                    failures.push(format!("{}: {}", ws.workspace_path, e));
                }
            }
        }
    }
    failures
}

#[cfg(test)]
#[path = "account_deletion_tests.rs"]
mod tests;
