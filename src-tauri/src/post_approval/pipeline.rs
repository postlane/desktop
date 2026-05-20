// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::platform_constants::KNOWN_SOCIAL_PLATFORMS;
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
    if !KNOWN_SOCIAL_PLATFORMS.contains(&platform) {
        return Err(format!(
            "Unknown platform '{}': must be one of {:?}",
            platform, KNOWN_SOCIAL_PLATFORMS
        ));
    }
    Ok(())
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

pub(super) fn validate_repo_path(repo_path: &str, state: &AppState) -> Result<String, String> {
    let canonical = std::fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize repo path '{}': {}", repo_path, e))?;
    let canonical_str = canonical
        .to_str()
        .ok_or("Repo path contains non-UTF-8 characters")?
        .to_string();
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;
    if !repos.repos.iter().any(|r| r.path == canonical_str) {
        return Err(format!("Repo '{}' is not registered", canonical_str));
    }
    Ok(canonical_str)
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

pub(super) fn load_account_ids(
    canonical_path: &Path,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let config_path = canonical_path.join(".postlane/config.json");
    let content = std::fs::read_to_string(&config_path)
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

pub(super) async fn call_scheduler(
    app: &tauri::AppHandle,
    platform: &str,
    post_path: &Path,
    meta: &PostMeta,
    canonical_path: &Path,
) -> Result<(String, Option<String>), String> {
    let config_path = canonical_path.join(".postlane/config.json");
    let project_id = read_project_id_from_config(&config_path)?;
    let cred = crate::scheduling::credential_router::get_scheduler_credential_with_fallback(
        canonical_path,
        &project_id,
        app,
    )
    .await?;
    let content = read_platform_content(post_path, platform)?;
    let account_ids = load_account_ids(canonical_path).unwrap_or_default();
    let account_id = account_ids
        .get(platform)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let scheduled_for = meta
        .scheduled_for
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let provider = crate::scheduling::credential_router::build_provider(&cred.provider, cred.api_key)?;
    let result = provider
        .schedule_post(&content, platform, scheduled_for, None, account_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok((result.scheduler_id, result.platform_url))
}

#[cfg(test)]
mod tests;
