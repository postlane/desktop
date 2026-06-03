// SPDX-License-Identifier: BUSL-1.1

mod post_location;
pub(super) use post_location::{PostLocation, validate_repo_path};

use crate::post_meta::{PostMeta, PostStatus};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// RAII guard — increments in_flight_sends on construction, decrements on drop.
/// Prevents the tray from quitting while a send is in progress.
pub(super) struct InFlightGuard(Arc<AtomicUsize>);

impl InFlightGuard {
    pub(super) fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(counter.clone())
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) fn validate_platform(platform: &str) -> Result<(), String> {
    crate::parser::char_limit(platform)
        .map(|_| ())
        .map_err(|e| match e {
            crate::parser::ValidationError::ParseError(msg) => msg,
            _ => format!("Unknown platform '{}'", platform),
        })
}

pub(super) fn validate_post_folder(post_folder: &str) -> Result<(), String> {
    if Path::new(post_folder).components().count() != 1 {
        return Err(format!(
            "Invalid post folder '{}': must be a single path component.",
            post_folder
        ));
    }
    Ok(())
}

pub(super) fn acquire_meta_lock(canonical_str: &str, post_folder: &str) -> Arc<tokio::sync::Mutex<()>> {
    let key = format!("{}\x00{}", canonical_str, post_folder);
    crate::platform_constants::POST_META_LOCKS
        .entry(key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// Returns true if `platform` appears in meta.edited_platforms.
/// None (pre-M19 post) and Some([]) (post-M19, unedited) both return false.
pub(super) fn is_platform_edited(meta: &PostMeta, platform: &str) -> bool {
    meta.edited_platforms
        .as_deref()
        .unwrap_or(&[])
        .contains(&platform.to_string())
}

pub(super) fn read_project_id_from_config(config_path: &Path) -> Result<String, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config.json at {}: {}", config_path.display(), e))?;
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;
    config["project_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "No project linked to this repo. Run `postlane init` to connect it.".to_string())
}

pub(super) fn append_unsplash_attribution(content: &str, meta: &PostMeta) -> String {
    const UNSPLASH_UTM: &str = "utm_source=postlane&utm_medium=referral";
    const UNSPLASH_BASE: &str = "https://unsplash.com/";

    if meta.image_source.as_deref() != Some("unsplash") {
        return content.to_string();
    }
    let Some(attr) = &meta.image_attribution else {
        return content.to_string();
    };
    if attr.photographer_name.is_empty() {
        return content.to_string();
    }
    let unsplash_url = format!("{}?{}", UNSPLASH_BASE, UNSPLASH_UTM);
    if attr.photographer_url.is_empty() {
        return format!("{}\n\nPhoto by {} on Unsplash {}", content, attr.photographer_name, unsplash_url);
    }
    let photographer_url = format!("{}?{}", attr.photographer_url, UNSPLASH_UTM);
    format!(
        "{}\n\nPhoto by {} {} on Unsplash {}",
        content, attr.photographer_name, photographer_url, unsplash_url
    )
}

pub(super) fn read_platform_content(post_path: &Path, platform: &str) -> Result<String, String> {
    let content_file = post_path.join(format!("{}.md", platform));
    if !content_file.exists() {
        return Err(format!(
            "Content file {}.md not found in {:?}",
            platform, post_path
        ));
    }
    std::fs::read_to_string(&content_file)
        .map_err(|e| format!("Failed to read {}.md: {}", platform, e))
}

/// Returns the path to config.json for `config_root`, handling both layout types.
/// Legacy repos: `{root}/.postlane/config.json`. Workspace: `{root}/config.json`.
pub(super) fn resolve_config_json(config_root: &Path) -> std::path::PathBuf {
    let postlane = config_root.join(".postlane");
    if postlane.is_dir() {
        postlane.join("config.json")
    } else {
        config_root.join("config.json")
    }
}

pub(super) fn load_account_ids(
    config_path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;
    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config.json: {}", e))?;
    Ok(config["scheduler"]["account_ids"]
        .as_object()
        .cloned()
        .unwrap_or_default())
}

/// Stamps a successful scheduler response onto `meta` in-place.
/// Empty scheduler_id and non-https platform_url are silently dropped.
pub(super) fn apply_scheduler_result(
    meta: &mut PostMeta,
    platform: &str,
    scheduler_id: &str,
    platform_url: Option<&str>,
    sent_at: &str,
) {
    meta.sent_platforms.insert(platform.to_string(), sent_at.to_string());
    meta.error = None;
    // Intentional: status=Sent is written when the FIRST platform is approved, not all.
    // This signals engagement_sync that the post is in-flight. Per-platform completion
    // is tracked accurately in sent_platforms — use that for "all platforms sent" queries.
    meta.status = Some(PostStatus::Sent);
    if !scheduler_id.is_empty() {
        meta.scheduler_ids.insert(platform.to_string(), scheduler_id.to_string());
    }
    if let Some(url) = platform_url {
        if url.starts_with("https://") {
            meta.platform_urls.insert(platform.to_string(), url.to_string());
        } else {
            log::warn!("[approve_post] rejected non-https platform_url: {}", url);
        }
    }
}

pub(super) fn record_scheduler_failure(meta: &mut PostMeta, error: &str) {
    meta.status = Some(PostStatus::Failed);
    meta.error = Some(error.to_string());
}

/// Paths needed by the send pipeline, grouped to stay under the 7-arg clippy limit.
pub(super) struct SendPaths<'a> {
    pub post_folder: &'a str,
    pub post_path: &'a std::path::Path,
    /// Root used for config lookups — workspace root for workspace posts, canonical path for legacy.
    pub config_root: std::path::PathBuf,
    pub meta_path: &'a std::path::Path,
}

/// Executes the send pipeline for a single platform: calls the scheduler (or
/// simulates success in test mode), then writes the updated meta to disk.
///
/// Returns `Err` in all failure cases so the frontend always surfaces the error.
/// Returning `Ok` after a scheduler success with a failed meta write would leave
/// the queue showing the post unsent, causing a duplicate send on retry.
pub(super) async fn run_send_pipeline(
    app: Option<&tauri::AppHandle>,
    platform: &str,
    paths: &SendPaths<'_>,
    meta: &mut PostMeta,
    sent_at: &str,
) -> Result<(), String> {
    if let Some(app_handle) = app {
        match call_scheduler(app_handle, platform, paths.post_path, meta, &paths.config_root).await {
            Ok((scheduler_id, platform_url)) => {
                apply_scheduler_result(meta, platform, &scheduler_id, platform_url.as_deref(), sent_at);
                log::info!("[approve_post] sent '{}' to '{}' via scheduler", paths.post_folder, platform);
                if let Err(e) = meta.save(paths.meta_path) {
                    log::error!("[approve_post] meta write failed after successful send: {}", e);
                    // Return Err so the frontend surfaces this to the user.
                    // Returning Ok() would leave the queue showing the post as unsent,
                    // causing a duplicate send when the user clicks Approve again.
                    return Err(format!(
                        "Post was sent but the local record could not be saved. \
                         Do not approve again — your post has already been submitted. \
                         Error: {}",
                        e
                    ));
                }
            }
            Err(e) => {
                // No retry: a single attempt failure writes status=Failed and surfaces
                // the error to the user. Retrying automatically risks duplicate sends
                // if the scheduler accepted the request but returned a transient error.
                // The user can re-approve from the queue once the issue is resolved.
                record_scheduler_failure(meta, &e);
                let _ = meta.save(paths.meta_path);
                return Err(e);
            }
        }
    } else {
        // Test mode: simulate scheduler success without an HTTP call.
        meta.sent_platforms.insert(platform.to_string(), sent_at.to_string());
        meta.status = Some(PostStatus::Sent);
        meta.save(paths.meta_path)?;
    }
    Ok(())
}

/// Returns Err if the platform content exceeds its character limit.
/// Silently passes for platforms that have no defined limit (e.g. webhook).
pub(super) fn validate_char_limit(platform: &str, post_path: &Path) -> Result<(), String> {
    let content = read_platform_content(post_path, platform)?;
    let count = crate::parser::count_chars(&content, platform);
    if let Ok(limit) = crate::parser::char_limit(platform) {
        if count > limit {
            return Err(format!(
                "Post exceeds the {} character limit for {} ({}/{}). Edit the post to shorten it.",
                limit, platform, count, limit
            ));
        }
    }
    Ok(())
}

/// Validates that both the active Mastodon instance and access token are present.
/// Takes the already-fetched Option values so this function is pure and testable.
pub(super) fn resolve_mastodon_credential(
    instance: Option<String>,
    access_token: Option<String>,
) -> Result<(String, String), String> {
    let instance = instance.ok_or_else(|| {
        "No Mastodon account connected. Connect one in Settings \u{2192} Channels.".to_string()
    })?;
    let access_token = access_token.ok_or_else(|| {
        format!(
            "Mastodon access token missing for '{}'. Reconnect in Settings \u{2192} Channels.",
            instance
        )
    })?;
    Ok((instance, access_token))
}

async fn call_mastodon_direct(
    app: &tauri::AppHandle,
    post_path: &Path,
    meta: &PostMeta,
    project_id: &str,
) -> Result<(String, Option<String>), String> {
    use crate::mastodon_connection::{access_token_key, active_instance_key, KEYRING_SERVICE};
    use crate::providers::scheduling::SchedulingProvider;
    use tauri_plugin_keyring::KeyringExt;

    let instance = app
        .keyring()
        .get_password(KEYRING_SERVICE, &active_instance_key(project_id))
        .map_err(|e| format!("Keyring error reading Mastodon instance: {}", e))?;
    let access_token = instance.as_deref().and_then(|inst| {
        app.keyring().get_password(KEYRING_SERVICE, &access_token_key(project_id, inst)).ok().flatten()
    });

    let (instance, access_token) = resolve_mastodon_credential(instance, access_token)?;

    let provider =
        crate::providers::scheduling::mastodon::MastodonProvider::create(&instance, access_token)
            .await
            .map_err(|e| e.to_string())?;

    let raw_content = read_platform_content(post_path, "mastodon")?;
    let content = append_unsplash_attribution(&raw_content, meta);
    let scheduled_for = meta
        .scheduled_for
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let result = provider
        .schedule_post(&content, "mastodon", scheduled_for, meta.image_url.as_deref(), None)
        .await
        .map_err(|e| e.to_string())?;
    Ok((result.scheduler_id, result.platform_url))
}

pub(super) async fn fetch_and_cache_account_id(
    platform: &str,
    provider: &dyn crate::providers::scheduling::SchedulingProvider,
    config_path: &Path,
) -> Option<String> {
    let profiles = match provider.list_profiles().await {
        Ok(p) => p,
        Err(e) => {
            log::warn!("[call_scheduler] could not fetch account_id for {}: {}", platform, e);
            return None;
        }
    };
    let profile = profiles.iter().find(|p| p.platforms.contains(&platform.to_string()))?;
    let _ = crate::account_id_store::save_account_id_impl(config_path, platform, &profile.id);
    Some(profile.id.clone())
}

pub(super) async fn call_scheduler(
    app: &tauri::AppHandle,
    platform: &str,
    post_path: &Path,
    meta: &PostMeta,
    config_root: &Path,
) -> Result<(String, Option<String>), String> {
    let config_path = resolve_config_json(config_root);
    let project_id = read_project_id_from_config(&config_path)?;
    if platform == "mastodon" {
        return call_mastodon_direct(app, post_path, meta, &project_id).await;
    }
    let cred = crate::scheduling::credential_router::get_scheduler_credential_with_fallback(
        config_root,
        &project_id,
        app,
    )
    .await?;
    let raw_content = read_platform_content(post_path, platform)?;
    let content = append_unsplash_attribution(&raw_content, meta);
    let account_ids = load_account_ids(&config_path).unwrap_or_default();
    let scheduled_for = meta
        .scheduled_for
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let provider = crate::scheduling::credential_router::build_provider(&cred.provider, cred.api_key)?;
    let resolved_id = match account_ids.get(platform).and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
        Some(id) => Some(id.to_string()),
        None => fetch_and_cache_account_id(platform, &*provider, &config_path).await,
    };
    let result = provider
        .schedule_post(&content, platform, scheduled_for, meta.image_url.as_deref(), resolved_id.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok((result.scheduler_id, result.platform_url))
}

#[cfg(test)]
mod tests;
