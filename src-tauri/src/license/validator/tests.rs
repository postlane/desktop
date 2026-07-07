// SPDX-License-Identifier: BUSL-1.1
use super::*;
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;
use std::sync::{Arc, Mutex, OnceLock};

static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
fn lock() -> &'static Mutex<()> { TEST_MUTEX.get_or_init(|| Mutex::new(())) }

/// Per-test RAII guard that redirects cache_path() to a private TempDir.
/// Each test process gets its own directory, so concurrent nextest processes
/// can never observe each other's cache files regardless of scheduling.
struct CacheGuard {
    pub path: PathBuf,
    _dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl CacheGuard {
    fn acquire() -> Self {
        let _lock = lock().lock().unwrap_or_else(|p| p.into_inner());
        let _dir = tempfile::TempDir::new().expect("temp dir");
        let path = _dir.path().join("license_cache.json");
        *super::TEST_CACHE_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(path.clone());
        CacheGuard { path, _dir, _lock }
    }
}

impl Drop for CacheGuard {
    fn drop(&mut self) {
        *super::TEST_CACHE_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = None;
        // _dir drops here, removing the entire temp directory
    }
}

fn mock_valid_response() -> serde_json::Value {
    serde_json::json!({
        "valid": true,
        "user": { "id": "u1", "display_name": "Alice", "avatar_url": null },
        "repos": [{ "uuid": "r1", "name": "my-repo", "status": "active" }]
    })
}

fn test_user() -> UserInfo {
    UserInfo { id: "u1".into(), display_name: Some("Alice".into()), avatar_url: None, email: None }
}

#[tokio::test]
async fn test_validate_token_success() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(mock_valid_response());
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Valid { .. }));
}

#[tokio::test]
async fn test_validate_token_expired() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(401);
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Expired), "401 should return Expired");
}

#[tokio::test]
async fn test_validate_token_offline() {
    let guard = CacheGuard::acquire();
    let cache = LicenseCache {
        version: 1,
        validated_at: Utc::now() - Duration::hours(1),
        user: test_user(),
        repos: vec![],
        workspaces: vec![],
    };
    std::fs::write(&guard.path, serde_json::to_string_pretty(&cache).unwrap()).unwrap();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(503);
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Offline { .. }), "503 should return Offline");
}

#[test]
fn test_license_cache_expired() {
    let cache = LicenseCache {
        version: 1,
        validated_at: Utc::now() - Duration::days(8),
        user: test_user(),
        repos: vec![],
        workspaces: vec![],
    };
    assert!(is_cache_expired(&cache), "8-day-old cache should be expired");
    let fresh = LicenseCache { validated_at: Utc::now() - Duration::days(6), ..cache };
    assert!(!is_cache_expired(&fresh), "6-day-old cache should not be expired");
}

/// validate_token_enforcing_expiry upgrades Offline to Expired when cache is >30 days old
/// (§review-security-medium).
#[tokio::test]
async fn test_enforcing_variant_returns_expired_for_stale_offline_cache() {
    let guard = CacheGuard::acquire();
    let stale = LicenseCache {
        version: 1,
        validated_at: Utc::now() - Duration::days(31),
        user: test_user(),
        repos: vec![],
        workspaces: vec![],
    };
    std::fs::write(&guard.path, serde_json::to_string_pretty(&stale).unwrap()).unwrap();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(503);
    });
    let client = build_client();
    let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
    assert!(
        matches!(state, LicenseState::Expired),
        "enforcing variant must return Expired for 31-day-old offline cache"
    );
}

/// validate_token_enforcing_expiry must treat a >30-day-old offline cache as Expired
/// and a <30-day-old cache as Offline.
#[tokio::test]
async fn test_enforcing_expiry_30_day_boundary() {
    let guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(503);
    });
    let client = build_client();

    let fresh = LicenseCache {
        version: 1,
        validated_at: Utc::now() - Duration::days(29),
        user: test_user(),
        repos: vec![],
        workspaces: vec![],
    };
    std::fs::write(&guard.path, serde_json::to_string_pretty(&fresh).unwrap()).unwrap();
    let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Offline { .. }), "29-day-old cache must not be hard-expired");

    let stale = LicenseCache {
        version: 1,
        validated_at: Utc::now() - Duration::days(31),
        user: test_user(),
        repos: vec![],
        workspaces: vec![],
    };
    std::fs::write(&guard.path, serde_json::to_string_pretty(&stale).unwrap()).unwrap();
    let state = validate_token_enforcing_expiry("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Expired), "31-day-old cache must be hard-expired");
}

#[tokio::test]
async fn test_validate_token_empty_returns_unconfigured() {
    let client = build_client();
    let state = validate_token_with_client("", &client, "https://unused.example.com").await.unwrap();
    assert!(matches!(state, LicenseState::Unconfigured));
}

/// Confirms the revalidation interval fires validate_token at the configured cadence.
/// Uses a 100ms interval and polls until 3 hits are observed (5 s deadline).
#[tokio::test]
async fn test_24hr_revalidation_interval() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    let mock_handle = server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(mock_valid_response());
    });

    let client = build_client();
    let base_url = server.base_url();

    let handle = tauri::async_runtime::spawn({
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            start_revalidation_loop(
                std::time::Duration::from_millis(100),
                "test-token",
                &client,
                &base_url,
                || {},
            )
            .await
        }
    });

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if mock_handle.hits() >= 3 { break; }
        assert!(
            std::time::Instant::now() < deadline,
            "revalidation loop did not fire 3 times within 5 s"
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    handle.abort();
}

/// Confirms on_expired callback is invoked when the backend returns 401.
#[tokio::test]
async fn test_revalidation_loop_calls_on_expired_callback() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(401);
    });

    let expired_called = Arc::new(AtomicBool::new(false));
    let expired_called_clone = expired_called.clone();

    let client = build_client();
    let base_url = server.base_url();

    let handle = tauri::async_runtime::spawn({
        let client = client.clone();
        let base_url = base_url.clone();
        async move {
            start_revalidation_loop(
                std::time::Duration::from_millis(100),
                "test-token",
                &client,
                &base_url,
                move || {
                    expired_called_clone.store(true, Ordering::SeqCst);
                },
            )
            .await
        }
    });

    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    handle.abort();

    assert!(
        expired_called.load(Ordering::SeqCst),
        "on_expired callback should have been called when 401 received"
    );
}

#[test]
fn test_parse_rejection_reason_extracts_json_reason() {
    assert_eq!(
        parse_rejection_reason(r#"{"valid":false,"reason":"not_found"}"#),
        "not_found"
    );
}

#[test]
fn test_parse_rejection_reason_extracts_revoked() {
    assert_eq!(
        parse_rejection_reason(r#"{"valid":false,"reason":"revoked"}"#),
        "revoked"
    );
}

#[test]
fn test_parse_rejection_reason_falls_back_on_non_json() {
    assert_eq!(parse_rejection_reason("Unauthorized"), "Unauthorized");
}

#[test]
fn test_parse_rejection_reason_empty_body() {
    assert_eq!(parse_rejection_reason(""), "(no body)");
}

/// 401 with a JSON reason body must still return Expired (not an Err).
#[tokio::test]
async fn test_validate_token_expired_with_reason_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(401)
            .json_body(serde_json::json!({"valid": false, "reason": "not_found"}));
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Expired), "401 with JSON body must return Expired");
}

#[test]
fn test_cache_guard_removes_file_on_drop() {
    let path = {
        let guard = CacheGuard::acquire();
        std::fs::write(&guard.path, b"test").expect("write test file");
        guard.path.clone()
    };
    assert!(!path.exists(), "CacheGuard::drop must remove the cache file");
}

#[test]
fn warn_on_cache_write_is_noop_on_ok() {
    warn_on_cache_write(Ok(()));
}

#[test]
fn warn_on_cache_write_does_not_panic_on_error() {
    warn_on_cache_write(Err("no space left on device".to_string()));
}

/// Web endpoint can return `display_name: null` — must not fail deserialization.
#[tokio::test]
async fn test_validate_token_null_display_name_succeeds() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": null, "avatar_url": null },
            "repos": []
        }));
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Valid { .. }), "null display_name must parse successfully");
}

/// Web endpoint may omit `repos` field — must not fail deserialization.
#[tokio::test]
async fn test_validate_token_missing_repos_field_succeeds() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": "Alice", "avatar_url": null }
        }));
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    assert!(matches!(state, LicenseState::Valid { .. }), "missing repos field must parse successfully");
}

/// checklist 24.4.8 — the workspaces[] array parses into LicenseState::Valid.
#[tokio::test]
async fn test_validate_token_parses_workspaces_field() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": "Alice", "avatar_url": null },
            "workspaces": [
                {
                    "project_id": "proj-1",
                    "name": "My Workspace",
                    "status": "paid_owned",
                    "is_owner": true,
                    "status_updated_at": "2026-07-01T00:00:00.000Z"
                }
            ]
        }));
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    match state {
        LicenseState::Valid { workspaces, .. } => {
            assert_eq!(workspaces.len(), 1);
            assert_eq!(workspaces[0].project_id, "proj-1");
            assert_eq!(workspaces[0].name, "My Workspace");
            assert_eq!(workspaces[0].status, "paid_owned");
            assert!(workspaces[0].is_owner);
            assert_eq!(workspaces[0].status_updated_at, "2026-07-01T00:00:00.000Z");
        }
        other => panic!("expected Valid state, got: {:?}", other),
    }
}

/// Web endpoint may omit `workspaces` field entirely (older server or no
/// workspaces yet) — must not fail deserialization.
#[tokio::test]
async fn test_validate_token_missing_workspaces_field_succeeds() {
    let _guard = CacheGuard::acquire();
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/license/validate");
        then.status(200).json_body(serde_json::json!({
            "valid": true,
            "user": { "id": "u1", "display_name": "Alice", "avatar_url": null }
        }));
    });
    let client = build_client();
    let state = validate_token_with_client("tok", &client, &server.base_url()).await.unwrap();
    match state {
        LicenseState::Valid { workspaces, .. } => assert!(workspaces.is_empty()),
        other => panic!("expected Valid state, got: {:?}", other),
    }
}
