// SPDX-License-Identifier: BUSL-1.1

pub(crate) mod pipeline;

use crate::app_state::AppState;
use crate::post_meta::PostMeta;
use pipeline::{
    acquire_meta_lock, is_platform_edited, run_send_pipeline, validate_char_limit,
    validate_platform, validate_post_folder, validate_repo_path,
    InFlightGuard, PostLocation, SendPaths,
};
use std::sync::atomic::Ordering;
use tauri::State;

/// checklist 24.4.11 — license_status values that block new approvals.
/// paid_required/owner_departing/collaborator do not block: paid_required
/// means payment hasn't completed yet but drafts aren't retroactively
/// blocked; owner_departing is mid-transfer, not a billing failure.
const BLOCKING_STATUSES: [&str; 3] = ["inactive", "payment_failed", "unlicensed"];

const PAYMENT_FAILED_GRACE_PERIOD_DAYS: i64 = 14;

/// Error returned by `approve_post` — either a licensing block with
/// structured CTA info the frontend renders differently per status/role
/// (checklist 24.4.11), or a plain message for every other failure mode.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind")]
pub enum ApproveError {
    #[serde(rename = "blocked")]
    Blocked { status: String, is_owner: bool, days_remaining: Option<i64> },
    #[serde(rename = "message")]
    Message { message: String },
}

impl From<String> for ApproveError {
    fn from(message: String) -> Self {
        ApproveError::Message { message }
    }
}

impl std::fmt::Display for ApproveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApproveError::Blocked { status, .. } => write!(f, "Approval blocked: workspace status is {}", status),
            ApproveError::Message { message } => write!(f, "{}", message),
        }
    }
}

/// Days left in the 14-day grace period before an unpaid workspace ages into
/// `unlicensed`, computed from when `status` last changed. Returns `None`
/// if `status_updated_at` is missing or unparseable, rather than guessing.
fn compute_days_remaining(status_updated_at: &str) -> Option<i64> {
    let updated_at = chrono::DateTime::parse_from_rfc3339(status_updated_at).ok()?;
    let elapsed_days = chrono::Utc::now().signed_duration_since(updated_at).num_days();
    Some((PAYMENT_FAILED_GRACE_PERIOD_DAYS - elapsed_days).max(0))
}

/// checklist 24.4.11 — rejects with a structured `Blocked` error when the
/// workspace's license_status is inactive/payment_failed/unlicensed. Legacy
/// (pre-workspace) repos and workspaces with no license_status yet (never
/// synced, or predates checklist 24.4.8) are never blocked here.
fn check_license_gate(location: &PostLocation) -> Result<(), ApproveError> {
    let PostLocation::Workspace { license_status, is_owner, status_updated_at, .. } = location else {
        return Ok(());
    };
    let Some(status) = license_status else {
        return Ok(());
    };
    if !BLOCKING_STATUSES.contains(&status.as_str()) {
        return Ok(());
    }
    let days_remaining = if status == "payment_failed" {
        status_updated_at.as_deref().and_then(compute_days_remaining)
    } else {
        None
    };
    Err(ApproveError::Blocked {
        status: status.clone(),
        is_owner: is_owner.unwrap_or(false),
        days_remaining,
    })
}

/// Fires the Unsplash download trigger if applicable, then writes `image_download_triggered_at`.
/// Only fires when `image_download_location` is set, `image_download_triggered_at` is null,
/// and `download_location` starts with `https://api.unsplash.com/`.
/// Skips silently (with a warning log) if the URL fails validation — approval must not be blocked.
fn fire_unsplash_download_trigger(
    meta: &mut PostMeta,
    meta_path: &std::path::Path,
    app: Option<&tauri::AppHandle>,
) {
    let Some(ref location) = meta.image_download_location.clone() else {
        return;
    };
    if meta.image_download_triggered_at.is_some() {
        return;
    }
    // SSRF defence: domain whitelist is the primary guard here.
    // Unlike OG image fetches (which use is_private_url for DNS-resolution-time
    // validation), the download_location is validated against a specific Unsplash
    // hostname prefix. Any private IP literal (127.x, 192.168.x, etc.) cannot pass
    // this check. DNS rebinding against api.unsplash.com is outside the threat model.
    if !location.starts_with("https://api.unsplash.com/") {
        log::warn!(
            "[approve_post] image_download_location rejected (must start with https://api.unsplash.com/): {}",
            location
        );
        return;
    }
    meta.image_download_triggered_at = Some(chrono::Utc::now().to_rfc3339());
    if let Err(e) = meta.save(meta_path) {
        log::error!("[approve_post] failed to write image_download_triggered_at: {}", e);
    }
    if app.is_some() {
        let loc = location.clone();
        tauri::async_runtime::spawn(async move {
            let client = reqwest::Client::builder()
                .user_agent(concat!("Postlane/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default();
            if let Err(e) = client.get(&loc).send().await {
                log::warn!("[approve_post] Unsplash download trigger failed (non-fatal): {}", e);
            }
        });
    }
}

/// Core implementation, injectable for testing (no real HTTP call when `app` is None).
pub async fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
    state: &AppState,
    app: Option<&tauri::AppHandle>,
    consent: bool,
) -> Result<(), ApproveError> {
    if state.license_expired.load(Ordering::Relaxed) {
        return Err("Your Postlane license has expired. Please renew at postlane.dev.".to_string().into());
    }
    let _guard = InFlightGuard::new(&state.in_flight_sends);
    validate_platform(platform)?;
    validate_post_folder(post_folder)?;
    let location = validate_repo_path(repo_path, state)?;
    check_license_gate(&location)?;
    let canonical_str = location.canonical().to_string();
    let lock = acquire_meta_lock(&canonical_str, post_folder);
    let _lock_guard = lock.lock().await;
    let post_path = location.posts_base(post_folder);
    let meta_path = post_path.join("meta.json");
    let mut meta = PostMeta::load(&meta_path)?;
    if meta.sent_platforms.contains_key(platform) {
        return Ok(());
    }
    if !post_path.exists() {
        return Err(format!("Post folder '{}' does not exist", post_folder).into());
    }
    validate_char_limit(platform, &post_path)?;
    // 21.8.8: fire Unsplash download trigger if not yet triggered.
    // Validates download_location starts with https://api.unsplash.com/ before any network call.
    // meta.json is a local writable file — always validate the URL (SSRF prevention).
    fire_unsplash_download_trigger(&mut meta, &meta_path, app);
    let sent_at = chrono::Utc::now().to_rfc3339();
    let is_edited = is_platform_edited(&meta, platform);
    let config_root = std::path::PathBuf::from(location.config_root());
    let paths = SendPaths { post_folder, post_path: &post_path, config_root, meta_path: &meta_path };
    run_send_pipeline(app, platform, &paths, &mut meta, &sent_at).await?;

    // Write sent.jsonl history entry for workspace repos (22.2.9)
    if let PostLocation::Workspace { workspace_path, posts_dir, repo_name, .. } = &location {
        let hist_dir = std::path::Path::new(workspace_path)
            .join("history")
            .join(posts_dir);
        let entry = crate::workspace_history::SentEntry {
            sent_at: sent_at.clone(),
            repo_name: repo_name.clone(),
            post_folder: post_folder.to_string(),
            platform: platform.to_string(),
            scheduler_id: meta.scheduler_ids.get(platform).cloned().unwrap_or_default(),
        };
        if let Err(e) = crate::workspace_history::append_sent_entry(&hist_dir, &entry) {
            log::warn!("[approve_post] failed to write sent.jsonl: {}", e);
        }
    }

    state.telemetry.record(
        consent,
        "post_approved",
        serde_json::json!({"platform": platform, "is_edited": is_edited}),
    );
    Ok(())
}

/// Tauri command — approves a post for publishing on the given platform.
#[tauri::command]
pub async fn approve_post(
    repo_path: String,
    post_folder: String,
    platform: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), ApproveError> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    approve_post_impl(&repo_path, &post_folder, &platform, &state, Some(&app), consent).await
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "license_gate_tests.rs"]
mod license_gate_tests;
