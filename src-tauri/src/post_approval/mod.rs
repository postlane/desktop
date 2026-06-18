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
) -> Result<(), String> {
    if state.license_expired.load(Ordering::Relaxed) {
        return Err("Your Postlane license has expired. Please renew at postlane.dev.".to_string());
    }
    let _guard = InFlightGuard::new(&state.in_flight_sends);
    validate_platform(platform)?;
    validate_post_folder(post_folder)?;
    let location = validate_repo_path(repo_path, state)?;
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
        return Err(format!("Post folder '{}' does not exist", post_folder));
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
) -> Result<(), String> {
    let consent = crate::app_state::read_app_state().telemetry_consent;
    approve_post_impl(&repo_path, &post_folder, &platform, &state, Some(&app), consent).await
}


#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
