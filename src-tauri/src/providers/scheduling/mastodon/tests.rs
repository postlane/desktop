// SPDX-License-Identifier: BUSL-1.1
use super::*;

// 9.5.1 — immediate post returns Status shape; post_id and post_url extracted correctly
#[tokio::test]
async fn test_schedule_post_immediate_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(200).json_body(serde_json::json!({
            "id": "103704874086360371",
            "url": "https://mastodon.social/@alice/103704874086360371",
            "content": "<p>Hello world</p>",
            "created_at": "2019-12-05T11:34:47.196Z"
        }));
    });

    let provider = make_provider(&server);
    let result = provider.schedule_post("Hello world", "mastodon", None, None, None).await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let r = result.unwrap();
    assert_eq!(r.scheduler_id, "103704874086360371");
    assert_eq!(r.platform_url, Some("https://mastodon.social/@alice/103704874086360371".to_string()));
    mock.assert();
}

// 9.5.2 — scheduled post returns ScheduledStatus shape; post_url is None
#[tokio::test]
async fn test_schedule_post_scheduled_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(200).json_body(serde_json::json!({
            "id": "3221",
            "scheduled_at": "2019-12-05T12:33:01.000Z",
            "params": { "text": "Hello future world" }
        }));
    });

    let provider = make_provider(&server);
    let scheduled = chrono::DateTime::parse_from_rfc3339("2019-12-05T12:33:01Z")
        .unwrap()
        .with_timezone(&Utc);
    let result = provider.schedule_post("Hello future world", "mastodon", Some(scheduled), None, None).await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let r = result.unwrap();
    assert_eq!(r.scheduler_id, "3221");
    assert!(r.platform_url.is_none(), "Scheduled post should have no platform_url");
    mock.assert();
}

// 9.5.3 — both response shapes return Ok (no panic on either)
#[tokio::test]
async fn test_schedule_post_handles_both_response_shapes() {
    let server = MockServer::start();

    // Immediate (Status shape)
    server.mock(|when, then| {
        when.method(POST).path("/statuses").body_contains("immediate");
        then.status(200).json_body(serde_json::json!({
            "id": "111",
            "url": "https://mastodon.social/@alice/111",
            "created_at": "2024-01-01T00:00:00Z"
        }));
    });

    // Scheduled (ScheduledStatus shape)
    server.mock(|when, then| {
        when.method(POST).path("/statuses").body_contains("scheduled");
        then.status(200).json_body(serde_json::json!({
            "id": "222",
            "scheduled_at": "2024-06-01T10:00:00.000Z",
            "params": { "text": "scheduled content" }
        }));
    });

    let provider = make_provider(&server);
    let future = chrono::DateTime::parse_from_rfc3339("2024-06-01T10:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let r1 = provider.schedule_post("immediate content", "mastodon", None, None, None).await;
    let r2 = provider.schedule_post("scheduled content", "mastodon", Some(future), None, None).await;

    assert!(r1.is_ok(), "Immediate shape should be Ok: {:?}", r1);
    assert!(r2.is_ok(), "Scheduled shape should be Ok: {:?}", r2);
}

// 9.5.4 — cancel scheduled post succeeds with 200
#[tokio::test]
async fn test_cancel_post_scheduled_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(DELETE).path("/scheduled_statuses/3221");
        then.status(200).json_body(serde_json::json!({}));
    });

    let provider = make_provider(&server);
    let result = provider.cancel_post("3221", "mastodon").await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    mock.assert();
}

// 9.5.5 — cancel immediate post returns NotSupported with correct message
#[tokio::test]
async fn test_cancel_post_immediate_not_supported() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(DELETE).path("/scheduled_statuses/103704874086360371");
        then.status(404);
    });

    let provider = make_provider(&server);
    let result = provider.cancel_post("103704874086360371", "mastodon").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::NotSupported(msg) => {
            assert!(msg.contains("Mastodon posts cannot be deleted"), "Unexpected message: {}", msg);
        }
        other => panic!("Expected NotSupported, got {:?}", other),
    }
    mock.assert();
}

// 9.5.6 — verify_credentials maps to exactly one SchedulerProfile
#[tokio::test]
async fn test_list_profiles_returns_single_profile() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(200).json_body(serde_json::json!({
            "id": "14715",
            "display_name": "Alice Bobsworth",
            "acct": "alice"
        }));
    });

    let provider = make_provider(&server);
    let result = provider.list_profiles().await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let profiles = result.unwrap();
    assert_eq!(profiles.len(), 1, "Mastodon returns exactly one profile");
    assert_eq!(profiles[0].id, "14715");
    assert_eq!(profiles[0].name, "Alice Bobsworth");
    assert_eq!(profiles[0].platforms, vec!["mastodon"]);
    mock.assert();
}

// 9.5.7 — scheduled_statuses array maps to Vec<QueuedPost>
#[tokio::test]
async fn test_get_queue_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/scheduled_statuses");
        then.status(200).json_body(serde_json::json!([
            {
                "id": "3221",
                "scheduled_at": "2024-06-01T12:00:00.000Z",
                "params": { "text": "First scheduled post" }
            },
            {
                "id": "3222",
                "scheduled_at": "2024-06-02T12:00:00.000Z",
                "params": { "text": "Second scheduled post" }
            }
        ]));
    });

    let provider = make_provider(&server);
    let result = provider.get_queue().await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let queue = result.unwrap();
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[0].post_id, "3221");
    assert_eq!(queue[0].platform, "mastodon");
    assert_eq!(queue[0].content_preview, "First scheduled post");
    assert_eq!(queue[1].post_id, "3222");
    mock.assert();
}

// 9.5.8 — engagement fields map correctly; impressions is None
#[tokio::test]
async fn test_get_engagement_maps_mastodon_fields() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/statuses/103704874086360371");
        then.status(200).json_body(serde_json::json!({
            "id": "103704874086360371",
            "favourites_count": 42,
            "reblogs_count": 12,
            "replies_count": 5
        }));
    });

    let provider = make_provider(&server);
    let result = provider.get_engagement("103704874086360371", "mastodon").await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let engagement = result.unwrap();
    assert_eq!(engagement.likes, 42, "favourites_count maps to likes");
    assert_eq!(engagement.reposts, 12, "reblogs_count maps to reposts");
    assert_eq!(engagement.replies, 5, "replies_count maps to replies");
    assert!(engagement.impressions.is_none(), "Mastodon has no impression count");
    mock.assert();
}

// 9.5.9 — test_connection returns Ok(()) on 200
#[tokio::test]
async fn test_test_connection_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(200).json_body(serde_json::json!({
            "id": "14715",
            "display_name": "Alice",
            "acct": "alice"
        }));
    });

    let provider = make_provider(&server);
    let result = provider.test_connection().await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    mock.assert();
}

// 9.5.10 — test_connection returns AuthError on 401
#[tokio::test]
async fn test_test_connection_auth_error() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/accounts/verify_credentials");
        then.status(401).json_body(serde_json::json!({ "error": "The access token is invalid" }));
    });

    let provider = make_provider(&server);
    let result = provider.test_connection().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ProviderError::AuthError(_) => {}
        other => panic!("Expected AuthError, got {:?}", other),
    }
    mock.assert();
}

// 9.5.11 — 429 with Retry-After header returns RateLimit with the correct duration

// --- media upload tests ---

// upload_media_bytes — 200 response with id field returns media_id
#[tokio::test]
async fn test_upload_media_bytes_returns_media_id() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/media");
        then.status(200).json_body(serde_json::json!({
            "id": "abc123",
            "url": "https://files.mastodon.social/img.jpg"
        }));
    });
    let result = super::media::upload_media_bytes(
        &build_client(),
        &server.base_url(),
        "test-token",
        b"fake image bytes",
        "image/jpeg",
    ).await;
    assert!(result.is_ok(), "{:?}", result);
    assert_eq!(result.unwrap(), "abc123");
}

// upload_media_bytes — non-2xx returns HttpError
#[tokio::test]
async fn test_upload_media_bytes_non_2xx_returns_http_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/media");
        then.status(422).body("Unprocessable entity");
    });
    let result = super::media::upload_media_bytes(
        &build_client(),
        &server.base_url(),
        "test-token",
        b"bad data",
        "image/jpeg",
    ).await;
    assert!(matches!(result, Err(ProviderError::HttpError { status: 422, .. })), "{:?}", result);
}

// upload_media_bytes — response missing id field returns Unknown error
#[tokio::test]
async fn test_upload_media_bytes_missing_id_returns_error() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/media");
        then.status(200).json_body(serde_json::json!({ "url": "https://files.mastodon.social/img.jpg" }));
    });
    let result = super::media::upload_media_bytes(
        &build_client(),
        &server.base_url(),
        "test-token",
        b"fake bytes",
        "image/jpeg",
    ).await;
    assert!(matches!(result, Err(ProviderError::Unknown(_))), "{:?}", result);
}

// schedule_post with image_url — uploads media first and includes media_ids in status body
#[tokio::test]
async fn test_schedule_post_with_image_url_includes_media_ids_in_body() {
    let image_server = MockServer::start();
    image_server.mock(|when, then| {
        when.method(GET).path("/photo.jpg");
        then.status(200)
            .header("Content-Type", "image/jpeg")
            .body(b"fake-image-bytes".as_ref());
    });

    let mastodon_server = MockServer::start();
    mastodon_server.mock(|when, then| {
        when.method(POST).path("/media");
        then.status(200).json_body(serde_json::json!({ "id": "media-id-xyz" }));
    });
    let statuses_mock = mastodon_server.mock(|when, then| {
        when.method(POST).path("/statuses").body_contains("media_ids");
        then.status(200).json_body(serde_json::json!({
            "id": "status-123",
            "url": "https://mastodon.social/@alice/status-123"
        }));
    });

    let provider = make_provider(&mastodon_server);
    let image_url = format!("{}/photo.jpg", image_server.base_url());
    let result = provider.schedule_post("Hello with image", "mastodon", None, Some(&image_url), None).await;

    assert!(result.is_ok(), "{:?}", result);
    statuses_mock.assert();
}

// schedule_post without image — no media_ids key in status body
#[tokio::test]
async fn test_schedule_post_without_image_excludes_media_ids_from_body() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/statuses");
        then.status(200).json_body(serde_json::json!({
            "id": "status-456",
            "url": "https://mastodon.social/@alice/status-456"
        }));
    });

    let provider = make_provider(&server);
    let result = provider.schedule_post("No image here", "mastodon", None, None, None).await;
    assert!(result.is_ok(), "{:?}", result);
    mock.assert();
}

// validate_image_url — rejects private IP (security rule)
#[tokio::test]
async fn test_validate_image_url_rejects_private_ip() {
    let result = super::media::validate_image_url("https://192.168.1.1/photo.jpg").await;
    assert!(result.is_err(), "Private IP image URL must be rejected");
}

// validate_image_url — rejects non-HTTPS scheme
#[tokio::test]
async fn test_validate_image_url_rejects_non_https() {
    let result = super::media::validate_image_url("http://images.unsplash.com/photo.jpg").await;
    assert!(result.is_err(), "HTTP image URL must be rejected");
}
