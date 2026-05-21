// SPDX-License-Identifier: BUSL-1.1
use super::*;

#[test]
fn test_upload_post_provider_name() {
    let provider = UploadPostProvider::new("test-key".to_string());
    assert_eq!(provider.name(), "upload_post");
}

#[test]
fn test_upload_post_provider_instantiation() {
    let _provider = UploadPostProvider::new("sk_test_12345".to_string());
}

#[test]
fn test_post_url_always_returns_none() {
    let provider = UploadPostProvider::new("test-key".to_string());
    assert_eq!(provider.post_url("bluesky", "some-id"), None);
    assert_eq!(provider.post_url("x", "tweet-id"), None);
    assert_eq!(provider.post_url("linkedin", "post-id"), None);
}

#[tokio::test]
async fn test_get_queue_returns_not_supported() {
    let provider = UploadPostProvider::new("test-key".to_string());
    let result = provider.get_queue().await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ProviderError::NotSupported(_)));
}

#[tokio::test]
async fn test_test_connection_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/uploadposts/me")
            .header("Authorization", "Apikey test-key");
        then.status(200)
            .json_body(serde_json::json!({"email": "user@example.com", "plan": "Basic"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    assert!(provider.test_connection().await.is_ok());
}

#[tokio::test]
async fn test_test_connection_auth_failure() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/me");
        then.status(401);
    });

    let mut provider = UploadPostProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let err = provider.test_connection().await.unwrap_err();
    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_list_profiles_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/uploadposts/users")
            .header("Authorization", "Apikey test-key");
        then.status(200).json_body(serde_json::json!({
            "success": true,
            "plan": "Basic",
            "profiles": [
                {"username": "myhandle", "platforms": ["bluesky", "x"]},
                {"username": "workaccount", "platforms": ["linkedin"]}
            ]
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let profiles = provider.list_profiles().await.unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].id, "myhandle");
    assert_eq!(profiles[0].platforms, vec!["bluesky", "x"]);
    assert_eq!(profiles[1].id, "workaccount");
    assert_eq!(profiles[1].platforms, vec!["linkedin"]);
}

#[tokio::test]
async fn test_list_profiles_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/users");
        then.status(401);
    });

    let mut provider = UploadPostProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let err = provider.list_profiles().await.unwrap_err();
    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_cancel_post_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(DELETE)
            .path("/uploadposts/schedule/job-abc-123")
            .header("Authorization", "Apikey test-key");
        then.status(200);
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    assert!(provider.cancel_post("job-abc-123", "bluesky").await.is_ok());
    mock.assert();
}

#[tokio::test]
async fn test_cancel_post_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(DELETE).path("/uploadposts/schedule/job-xyz");
        then.status(401);
    });

    let mut provider = UploadPostProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let err = provider.cancel_post("job-xyz", "bluesky").await.unwrap_err();
    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_get_engagement_success() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/uploadposts/post-analytics/req-456")
            .header("Authorization", "Apikey test-key");
        then.status(200).json_body(serde_json::json!({
            "post_metrics": {
                "likes": 42,
                "shares": 12,
                "comments": 5,
                "views": 1500
            }
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let eng = provider.get_engagement("req-456", "bluesky").await.unwrap();
    assert_eq!(eng.likes, 42);
    assert_eq!(eng.reposts, 12);
    assert_eq!(eng.replies, 5);
    assert_eq!(eng.impressions, Some(1500));
}

#[tokio::test]
async fn test_get_engagement_without_views() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/post-analytics/req-789");
        then.status(200).json_body(serde_json::json!({
            "post_metrics": {"likes": 10, "shares": 2, "comments": 1}
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let eng = provider.get_engagement("req-789", "x").await.unwrap();
    assert_eq!(eng.impressions, None);
}

#[tokio::test]
async fn test_validate_profile_valid_username_returns_platforms() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET)
            .path("/uploadposts/users/myhandle")
            .header("Authorization", "Apikey test-key");
        then.status(200).json_body(serde_json::json!({
            "username": "myhandle",
            "social_accounts": [
                {"platform": "bluesky"},
                {"platform": "x"}
            ]
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let platforms = provider.validate_profile("myhandle").await.unwrap();
    assert_eq!(platforms, vec!["bluesky", "x"]);
}

#[tokio::test]
async fn test_validate_profile_not_found_returns_404_with_case_hint() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/users/NoSuchUser");
        then.status(404);
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let err = provider.validate_profile("NoSuchUser").await.unwrap_err();
    match err {
        ProviderError::HttpError { status, body } => {
            assert_eq!(status, 404);
            assert!(
                body.contains("case-sensitive"),
                "error must mention case-sensitivity, got: {}",
                body
            );
        }
        other => panic!("Expected HttpError(404), got {:?}", other),
    }
}

#[tokio::test]
async fn test_validate_profile_auth_error() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/users/anyuser");
        then.status(401);
    });

    let mut provider = UploadPostProvider::new("bad-key".to_string());
    provider.base_url = server.base_url();

    let err = provider.validate_profile("anyuser").await.unwrap_err();
    assert!(matches!(err, ProviderError::AuthError(_)));
}

#[tokio::test]
async fn test_validate_profile_returns_empty_when_no_social_accounts() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/users/bare");
        then.status(200).json_body(serde_json::json!({"username": "bare"}));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let platforms = provider.validate_profile("bare").await.unwrap();
    assert!(platforms.is_empty());
}

#[tokio::test]
async fn test_validate_profile_parses_flat_platforms_array() {
    use httpmock::prelude::*;

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/uploadposts/users/handle2");
        then.status(200).json_body(serde_json::json!({
            "username": "handle2",
            "platforms": ["linkedin", "bluesky"]
        }));
    });

    let mut provider = UploadPostProvider::new("test-key".to_string());
    provider.base_url = server.base_url();

    let platforms = provider.validate_profile("handle2").await.unwrap();
    assert_eq!(platforms, vec!["linkedin", "bluesky"]);
}
