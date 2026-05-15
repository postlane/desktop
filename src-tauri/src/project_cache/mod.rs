// SPDX-License-Identifier: BUSL-1.1

use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::project_validation::validate_project_id;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProjectVoiceGuideResponse {
    voice_guide: Option<String>,
    #[serde(default)]
    voice_guide_fields: Option<serde_json::Value>,
}

type VoiceGuideEntry = (std::time::Instant, String);
static VOICE_GUIDE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, VoiceGuideEntry>>,
> = std::sync::OnceLock::new();
pub(crate) const VOICE_GUIDE_CACHE_TTL_SECS: u64 = 3600;

fn voice_guide_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, VoiceGuideEntry>> {
    VOICE_GUIDE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn vg_cache_get(project_id: &str, ttl_secs: u64) -> Option<String> {
    let guard = voice_guide_cache().lock().ok()?;
    let (fetched_at, content) = guard.get(project_id)?;
    if fetched_at.elapsed().as_secs() < ttl_secs {
        Some(content.clone())
    } else {
        None
    }
}

fn vg_cache_set(project_id: &str, content: String) {
    if let Ok(mut guard) = voice_guide_cache().lock() {
        guard.insert(project_id.to_string(), (std::time::Instant::now(), content));
    }
}

pub(crate) fn vg_cache_invalidate(project_id: &str) {
    if let Ok(mut guard) = voice_guide_cache().lock() {
        guard.remove(project_id);
    }
}

pub(crate) async fn get_project_voice_guide_cached(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    ttl_secs: u64,
) -> Result<Option<String>, String> {
    if let Some(cached) = vg_cache_get(project_id, ttl_secs) {
        return Ok(Some(cached));
    }
    let result = get_project_voice_guide_with_client(project_id, client, base_url, token).await?;
    if let Some(ref content) = result {
        vg_cache_set(project_id, content.clone());
    }
    Ok(result)
}

/// Calls `GET {base_url}/v1/projects/{project_id}` and returns the `voice_guide` field.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn get_project_voice_guide_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Option<String>, String> {
    validate_project_id(project_id)?;
    let url = format!("{}/v1/projects/{}", base_url, project_id);
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
    resp.json::<ProjectVoiceGuideResponse>()
        .await
        .map(|r| r.voice_guide)
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Calls `GET {base_url}/v1/projects/{project_id}` and returns the `voice_guide_fields` field.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn get_voice_guide_fields_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Option<serde_json::Value>, String> {
    validate_project_id(project_id)?;
    let url = format!("{}/v1/projects/{}", base_url, project_id);
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
    resp.json::<ProjectVoiceGuideResponse>()
        .await
        .map(|r| r.voice_guide_fields)
        .map_err(|e| format!("Failed to parse response: {}", e))
}

/// Calls `PATCH {base_url}/v1/projects/{project_id}` to save both `voice_guide` and optionally
/// `voice_guide_fields`. Invalidates the in-memory voice guide cache on success.
/// Returns `Err(SESSION_EXPIRED_ERROR)` on 401; `Err(...)` on any other non-200.
pub async fn save_project_voice_guide_and_fields_with_client(
    project_id: &str,
    voice_guide: &str,
    voice_guide_fields: Option<&serde_json::Value>,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    if voice_guide.len() > 5000 {
        return Err(format!(
            "voice_guide must be 5000 characters or fewer (got {})",
            voice_guide.len()
        ));
    }
    validate_project_id(project_id)?;
    let url = format!("{}/v1/projects/{}", base_url, project_id);
    let mut body = serde_json::json!({ "voice_guide": voice_guide });
    if let Some(fields) = voice_guide_fields {
        body["voice_guide_fields"] = fields.clone();
    }
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
    vg_cache_invalidate(project_id);
    Ok(())
}

/// Calls `PATCH {base_url}/v1/projects/{project_id}` to save `voice_guide` only.
/// Delegates to [`save_project_voice_guide_and_fields_with_client`] with `None` for fields.
pub async fn save_project_voice_guide_with_client(
    project_id: &str,
    voice_guide: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    save_project_voice_guide_and_fields_with_client(project_id, voice_guide, None, client, base_url, token).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod guide_tests;
#[cfg(test)]
mod cache_field_tests;
