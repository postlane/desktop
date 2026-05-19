// SPDX-License-Identifier: BUSL-1.1
use super::*;

#[tokio::test]
async fn test_schedule_post_scheduled_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/upload_text")
            .header("Authorization", "Apikey test-key");
        then.status(202)
            .json_body(serde_json::json!({"success": true, "job_id": "job-abc-123"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let scheduled_at = chrono::DateTime::parse_from_rfc3339("2025-12-31T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);

    let result = provider
        .schedule_post("Test content", "bluesky", Some(scheduled_at), None, Some("myhandle"))
        .await;

    assert!(result.is_ok(), "schedule_post failed: {:?}", result);
    assert_eq!(result.unwrap().scheduler_id, "job-abc-123");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_immediate_uses_request_id() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(200)
            .json_body(serde_json::json!({"success": true, "request_id": "req-xyz-789"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider
        .schedule_post("Post now", "x", None, None, Some("myhandle"))
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap().scheduler_id, "req-xyz-789");
    mock.assert();
}

#[tokio::test]
async fn test_schedule_post_prefers_job_id_over_request_id() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(202).json_body(serde_json::json!({
            "success": true,
            "job_id": "job-preferred",
            "request_id": "req-fallback"
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let dt = chrono::DateTime::parse_from_rfc3339("2025-12-31T10:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let result = provider
        .schedule_post("Content", "bluesky", Some(dt), None, Some("handle"))
        .await;

    assert_eq!(result.unwrap().scheduler_id, "job-preferred");
}

#[tokio::test]
async fn test_schedule_post_requires_profile_id() {
    let provider = UploadPostProvider::new("test-key".to_string());

    let err = provider
        .schedule_post("Content", "bluesky", None, None, None)
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_schedule_post_rejects_empty_profile_id() {
    let provider = UploadPostProvider::new("test-key".to_string());

    let err = provider
        .schedule_post("Content", "bluesky", None, None, Some(""))
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_schedule_post_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(401);
    });

    let mut provider = UploadPostProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let err = provider
        .schedule_post("Content", "bluesky", None, None, Some("handle"))
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_schedule_post_rate_limit() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        // Use 1s so retries complete in ~3s rather than 180s
        then.status(429).header("Retry-After", "1");
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let err = provider
        .schedule_post("Content", "bluesky", None, None, Some("handle"))
        .await
        .unwrap_err();

    match err {
        ProviderError::RateLimit(d) => assert_eq!(d.as_secs(), 1),
        other => panic!("Expected RateLimit, got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_http_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(400).json_body(serde_json::json!({"error": "Invalid platform"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let err = provider
        .schedule_post("Content", "invalid-platform", None, None, Some("handle"))
        .await
        .unwrap_err();

    match err {
        ProviderError::HttpError { status, .. } => assert_eq!(status, 400),
        other => panic!("Expected HttpError, got {:?}", other),
    }
}

#[tokio::test]
async fn test_schedule_post_missing_id_in_response() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(200).json_body(serde_json::json!({"success": true}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let err = provider
        .schedule_post("Content", "bluesky", None, None, Some("handle"))
        .await
        .unwrap_err();

    assert!(matches!(err, ProviderError::Unknown(_)));
}

#[tokio::test]
async fn test_schedule_post_platform_url_is_none() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/upload_text");
        then.status(200).json_body(serde_json::json!({"request_id": "r-1"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let result = provider
        .schedule_post("Content", "bluesky", None, None, Some("handle"))
        .await
        .unwrap();

    assert_eq!(result.platform_url, None);
}
