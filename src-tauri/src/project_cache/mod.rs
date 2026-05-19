// SPDX-License-Identifier: BUSL-1.1

use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::project_validation::validate_project_id;
use serde::Deserialize;

// ── Response type ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProjectVoiceGuideResponse {
    voice_guide: Option<String>,
    #[serde(default)]
    voice_guide_fields: Option<serde_json::Value>,
}

// ── Public data type ──────────────────────────────────────────────────────────

/// Both voice guide fields returned by a single GET /v1/projects/{id} call.
#[derive(Clone, Debug)]
pub struct ProjectVoiceGuideData {
    pub voice_guide: Option<String>,
    pub voice_guide_fields: Option<serde_json::Value>,
}

// ── Cache ─────────────────────────────────────────────────────────────────────

type VoiceGuideEntry = (std::time::Instant, ProjectVoiceGuideData);
static VOICE_GUIDE_CACHE: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, VoiceGuideEntry>>,
> = std::sync::OnceLock::new();
pub(crate) const VOICE_GUIDE_CACHE_TTL_SECS: u64 = 3600;

fn voice_guide_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, VoiceGuideEntry>> {
    VOICE_GUIDE_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

fn vg_cache_get(project_id: &str, ttl_secs: u64) -> Option<ProjectVoiceGuideData> {
    let guard = voice_guide_cache().lock().ok()?;
    let (fetched_at, data) = guard.get(project_id)?;
    if fetched_at.elapsed().as_secs() < ttl_secs {
        Some(data.clone())
    } else {
        None
    }
}

fn vg_cache_set(project_id: &str, data: ProjectVoiceGuideData) {
    if let Ok(mut guard) = voice_guide_cache().lock() {
        guard.insert(project_id.to_string(), (std::time::Instant::now(), data));
    }
}

pub(crate) fn vg_cache_invalidate(project_id: &str) {
    if let Ok(mut guard) = voice_guide_cache().lock() {
        guard.remove(project_id);
    }
}

// ── Core fetch ────────────────────────────────────────────────────────────────

/// Fetches both `voice_guide` and `voice_guide_fields` in a single HTTP call.
/// All other functions in this module delegate here.
pub async fn get_project_voice_guide_full_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<ProjectVoiceGuideData, String> {
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
        .map(|r| ProjectVoiceGuideData {
            voice_guide: r.voice_guide,
            voice_guide_fields: r.voice_guide_fields,
        })
        .map_err(|e| format!("Failed to parse response: {}", e))
}

// ── Cached access ─────────────────────────────────────────────────────────────

async fn get_full_cached(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    ttl_secs: u64,
) -> Result<ProjectVoiceGuideData, String> {
    if let Some(cached) = vg_cache_get(project_id, ttl_secs) {
        return Ok(cached);
    }
    let data = get_project_voice_guide_full_with_client(project_id, client, base_url, token).await?;
    vg_cache_set(project_id, data.clone());
    Ok(data)
}

/// Returns the `voice_guide` text, using the shared cache when fresh.
pub(crate) async fn get_project_voice_guide_cached(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
    ttl_secs: u64,
) -> Result<Option<String>, String> {
    Ok(get_full_cached(project_id, client, base_url, token, ttl_secs).await?.voice_guide)
}

/// Returns the `voice_guide` text without caching. Kept for callers that need a guaranteed
/// fresh read (e.g. in tests); production callers should prefer `get_project_voice_guide_cached`.
pub async fn get_project_voice_guide_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Option<String>, String> {
    Ok(get_project_voice_guide_full_with_client(project_id, client, base_url, token).await?.voice_guide)
}

/// Returns `voice_guide_fields`, using the shared cache (standard TTL).
pub async fn get_voice_guide_fields_with_client(
    project_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<Option<serde_json::Value>, String> {
    Ok(get_full_cached(project_id, client, base_url, token, VOICE_GUIDE_CACHE_TTL_SECS)
        .await?
        .voice_guide_fields)
}

// ── Save ──────────────────────────────────────────────────────────────────────

/// Calls `PATCH {base_url}/v1/projects/{project_id}` to save both `voice_guide` and optionally
/// `voice_guide_fields`. Invalidates the shared cache on success.
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

/// Saves `voice_guide` only. Delegates to `save_project_voice_guide_and_fields_with_client`.
pub async fn save_project_voice_guide_with_client(
    project_id: &str,
    voice_guide: &str,
    client: &reqwest::Client,
    base_url: &str,
    token: &str,
) -> Result<(), String> {
    save_project_voice_guide_and_fields_with_client(
        project_id, voice_guide, None, client, base_url, token,
    )
    .await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod guide_tests;
#[cfg(test)]
mod cache_field_tests;
