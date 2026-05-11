// SPDX-License-Identifier: BUSL-1.1

use crate::project_registry::SESSION_EXPIRED_ERROR;
use crate::project_validation::validate_project_id;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProjectVoiceGuideResponse {
    voice_guide: Option<String>,
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

pub async fn save_project_voice_guide_with_client(
    project_id: &str,
    voice_guide: &str,
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
    let resp = client
        .patch(&url)
        .bearer_auth(token)
        .json(&serde_json::json!({ "voice_guide": voice_guide }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::scheduling::build_client;
    use httpmock::prelude::*;

    // ── SessionExpired on 401 ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_project_voice_guide_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
            then.status(401);
        });
        let result = get_project_voice_guide_with_client(
            "proj-abc",
            &build_client(),
            &server.base_url(),
            "expired-tok",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            SESSION_EXPIRED_ERROR,
            "HTTP 401 must return session_expired error"
        );
    }

    #[tokio::test]
    async fn test_save_project_voice_guide_returns_session_expired_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
            then.status(401);
        });
        let result = save_project_voice_guide_with_client(
            "proj-abc",
            "Guide text.",
            &build_client(),
            &server.base_url(),
            "expired-tok",
        )
        .await;
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            SESSION_EXPIRED_ERROR,
            "HTTP 401 must return session_expired error"
        );
    }

    // ── get_project_voice_guide ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_voice_guide_returns_none_when_null() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
            then.status(200).json_body(serde_json::json!({ "voice_guide": null }));
        });
        let result = get_project_voice_guide_with_client(
            "proj-abc",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert_eq!(result, Ok(None));
    }

    #[tokio::test]
    async fn test_get_voice_guide_returns_some_when_set() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
            then.status(200)
                .json_body(serde_json::json!({ "voice_guide": "Direct and technical." }));
        });
        let result = get_project_voice_guide_with_client(
            "proj-abc",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert_eq!(result, Ok(Some("Direct and technical.".to_string())));
    }

    #[tokio::test]
    async fn test_get_voice_guide_returns_err_on_non_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-abc");
            then.status(404);
        });
        let result = get_project_voice_guide_with_client(
            "proj-abc",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_voice_guide_returns_err_on_network_failure() {
        let result = get_project_voice_guide_with_client(
            "proj-abc",
            &build_client(),
            "http://127.0.0.1:1",
            "tok",
        )
        .await;
        assert!(result.is_err());
    }

    // ── save_project_voice_guide ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_saves_voice_guide() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
            then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
        });
        save_project_voice_guide_with_client(
            "proj-abc",
            "Direct and technical.",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await
        .expect("should succeed");
        mock.assert();
    }

    #[tokio::test]
    async fn test_accepts_empty_voice_guide() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects/proj-abc");
            then.status(200).json_body(serde_json::json!({ "id": "proj-abc" }));
        });
        save_project_voice_guide_with_client(
            "proj-abc",
            "",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await
        .expect("should accept empty voice guide");
    }

    #[tokio::test]
    async fn test_save_project_voice_guide_returns_error_on_http_failure() {
        let result = save_project_voice_guide_with_client(
            "proj-abc",
            "Direct.",
            &build_client(),
            "http://127.0.0.1:1",
            "tok",
        )
        .await;
        assert!(result.is_err(), "network failure must return Err");
    }

    #[tokio::test]
    async fn test_save_project_voice_guide_rejects_voice_guide_exceeding_5000_chars() {
        let long_guide = "x".repeat(5001);
        let result = save_project_voice_guide_with_client(
            "proj-abc",
            &long_guide,
            &build_client(),
            "http://127.0.0.1:1",
            "tok",
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("5000"), "error must mention the limit");
    }

    // ── validate_project_id integration tests ────────────────────────────────

    #[tokio::test]
    async fn test_get_voice_guide_rejects_invalid_project_id() {
        let result = get_project_voice_guide_with_client(
            "proj/../../evil",
            &build_client(),
            "http://127.0.0.1:1",
            "tok",
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid"));
    }

    #[tokio::test]
    async fn test_save_voice_guide_rejects_invalid_project_id() {
        let result = save_project_voice_guide_with_client(
            "proj name",
            "guide text",
            &build_client(),
            "http://127.0.0.1:1",
            "tok",
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid"));
    }

    // ── voice guide caching ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_voice_guide_cache_hit_avoids_second_request() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache1");
            then.status(200).json_body(serde_json::json!({ "voice_guide": "Concise tone." }));
        });
        let r1 = get_project_voice_guide_cached(
            "proj-vgcache1",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await;
        assert_eq!(r1.unwrap(), Some("Concise tone.".to_string()));
        let r2 = get_project_voice_guide_cached(
            "proj-vgcache1",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await;
        assert_eq!(r2.unwrap(), Some("Concise tone.".to_string()));
        mock.assert_hits(1);
    }

    #[tokio::test]
    async fn test_voice_guide_cache_expires_with_zero_ttl() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache2");
            then.status(200).json_body(serde_json::json!({ "voice_guide": "Fresh voice." }));
        });
        get_project_voice_guide_cached(
            "proj-vgcache2",
            &build_client(),
            &server.base_url(),
            "tok",
            0,
        )
        .await
        .unwrap();
        get_project_voice_guide_cached(
            "proj-vgcache2",
            &build_client(),
            &server.base_url(),
            "tok",
            0,
        )
        .await
        .unwrap();
        mock.assert_hits(2);
    }

    #[tokio::test]
    async fn test_voice_guide_cache_invalidated_on_save() {
        let server = MockServer::start();
        let get_mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache3");
            then.status(200).json_body(serde_json::json!({ "voice_guide": "Old voice." }));
        });
        let patch_mock = server.mock(|when, then| {
            when.method(httpmock::Method::PATCH).path("/v1/projects/proj-vgcache3");
            then.status(200);
        });
        get_project_voice_guide_cached(
            "proj-vgcache3",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await
        .unwrap();
        save_project_voice_guide_with_client(
            "proj-vgcache3",
            "New voice.",
            &build_client(),
            &server.base_url(),
            "tok",
        )
        .await
        .unwrap();
        get_project_voice_guide_cached(
            "proj-vgcache3",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await
        .unwrap();
        get_mock.assert_hits(2);
        patch_mock.assert_hits(1);
    }

    #[tokio::test]
    async fn test_voice_guide_cache_miss_for_none_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(httpmock::Method::GET).path("/v1/projects/proj-vgcache4");
            then.status(200).json_body(serde_json::json!({ "voice_guide": null }));
        });
        get_project_voice_guide_cached(
            "proj-vgcache4",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await
        .unwrap();
        get_project_voice_guide_cached(
            "proj-vgcache4",
            &build_client(),
            &server.base_url(),
            "tok",
            3600,
        )
        .await
        .unwrap();
        mock.assert_hits(2);
    }
}
