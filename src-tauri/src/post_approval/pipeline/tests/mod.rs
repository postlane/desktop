// SPDX-License-Identifier: BUSL-1.1
use super::*;
use crate::post_meta::PostMeta;
use crate::test_fixtures::{make_repo, make_state};

mod account_id_tests;
mod attribution_tests;

// --- §read_project_id_from_config ---

#[test]
fn test_read_project_id_returns_id_from_config() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let config_path = dir.path().join("config.json");
    std::fs::write(&config_path, r#"{"project_id": "proj-abc-123"}"#).expect("write");
    let result = read_project_id_from_config(&config_path);
    assert_eq!(result.unwrap(), "proj-abc-123");
}

#[test]
fn test_read_project_id_errors_when_field_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let config_path = dir.path().join("config.json");
    std::fs::write(&config_path, r#"{"scheduler": {"provider": "zernio"}}"#).expect("write");
    let result = read_project_id_from_config(&config_path);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_lowercase();
    assert!(msg.contains("postlane init"), "error must guide user: got '{}'", msg);
}

#[test]
fn test_read_project_id_errors_when_config_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let config_path = dir.path().join("nonexistent_config.json");
    let result = read_project_id_from_config(&config_path);
    assert!(result.is_err());
}

// --- §is_platform_edited ---

#[test]
fn test_approve_post_records_edited_true_when_platform_in_edited_platforms() {
    let meta = PostMeta {
        edited_platforms: Some(vec!["x".to_string()]),
        ..PostMeta::default()
    };
    assert!(is_platform_edited(&meta, "x"));
}

#[test]
fn test_approve_post_records_edited_false_when_platform_not_in_edited_platforms() {
    let meta = PostMeta {
        edited_platforms: Some(vec!["linkedin".to_string()]),
        ..PostMeta::default()
    };
    assert!(!is_platform_edited(&meta, "x"));
}

#[test]
fn test_approve_post_records_edited_false_when_edited_platforms_none() {
    let meta = PostMeta {
        edited_platforms: None,
        ..PostMeta::default()
    };
    assert!(!is_platform_edited(&meta, "x"));
}

// --- §scheduler_result ---

#[test]
fn test_approve_post_writes_sent_at_after_scheduler_success() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "sched-1", None, "2026-05-01T00:00:00Z");
    assert_eq!(
        meta.sent_platforms.get("x").map(String::as_str),
        Some("2026-05-01T00:00:00Z")
    );
}

#[test]
fn test_approve_post_writes_scheduler_id_from_response() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "abc123", None, "2026-05-01T00:00:00Z");
    assert_eq!(
        meta.scheduler_ids.get("x").map(String::as_str),
        Some("abc123")
    );
}

#[test]
fn test_approve_post_writes_platform_url_from_response() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "s1", Some("https://x.com/post/123"), "2026-05-01T00:00:00Z");
    assert_eq!(
        meta.platform_urls.get("x").map(String::as_str),
        Some("https://x.com/post/123")
    );
}

#[test]
fn test_approve_post_does_not_write_scheduler_id_when_absent() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "", None, "2026-05-01T00:00:00Z");
    assert!(meta.scheduler_ids.is_empty(), "empty scheduler_id must not be stored");
}

#[test]
fn test_approve_post_rejects_http_platform_url() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "s1", Some("http://x.com/post/123"), "2026-05-01T00:00:00Z");
    assert!(meta.platform_urls.is_empty(), "http:// url must be rejected");
}

#[test]
fn test_apply_scheduler_result_sets_sent_status() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "sched-1", None, "2026-05-01T00:00:00Z");
    assert_eq!(
        meta.status,
        Some(PostStatus::Sent),
        "apply_scheduler_result must set status=Sent so engagement_sync can find the post"
    );
}

#[test]
fn test_apply_scheduler_result_clears_error_field_on_success() {
    let mut meta = PostMeta {
        error: Some("No project linked to this repo. Run `postlane init` to connect it.".to_string()),
        ..Default::default()
    };
    apply_scheduler_result(&mut meta, "bluesky", "sched-1", None, "2026-06-02T10:00:00Z");
    assert!(meta.error.is_none(), "apply_scheduler_result must clear meta.error so stale errors don't persist after retry");
}

// --- §failed_status ---

#[test]
fn test_approve_post_writes_failed_status_on_scheduler_error() {
    let mut meta = PostMeta::default();
    record_scheduler_failure(&mut meta, "HTTP 500 from scheduler");
    assert_eq!(meta.status, Some(PostStatus::Failed));
    assert_eq!(meta.error.as_deref(), Some("HTTP 500 from scheduler"));
}

// --- §validate_char_limit ---

fn write_platform_file(dir: &std::path::Path, platform: &str, content: &str) {
    std::fs::write(dir.join(format!("{}.md", platform)), content).expect("write platform file");
}

#[test]
fn test_validate_char_limit_accepts_post_within_limit() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    write_platform_file(dir.path(), "bluesky", &"a".repeat(300));
    assert!(validate_char_limit("bluesky", dir.path()).is_ok());
}

#[test]
fn test_validate_char_limit_rejects_post_over_limit() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    write_platform_file(dir.path(), "bluesky", &"a".repeat(301));
    let err = validate_char_limit("bluesky", dir.path()).unwrap_err();
    assert!(err.contains("301"), "error must mention actual count");
    assert!(err.contains("300"), "error must mention the limit");
    assert!(err.contains("bluesky"), "error must name the platform");
}

#[test]
fn test_validate_char_limit_passes_for_platform_without_limit() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    write_platform_file(dir.path(), "webhook", &"x".repeat(9999));
    assert!(validate_char_limit("webhook", dir.path()).is_ok());
}

#[test]
fn test_validate_char_limit_rejects_x_post_over_280() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    write_platform_file(dir.path(), "x", &"a".repeat(281));
    assert!(validate_char_limit("x", dir.path()).is_err());
}

#[test]
fn test_validate_char_limit_errors_when_platform_file_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let err = validate_char_limit("bluesky", dir.path()).unwrap_err();
    assert!(err.contains("bluesky.md"), "error must name the missing file");
}

// --- §validate_platform ---

#[test]
fn test_validate_platform_accepts_x() {
    assert!(validate_platform("x").is_ok());
}

#[test]
fn test_validate_platform_accepts_bluesky() {
    assert!(validate_platform("bluesky").is_ok());
}

#[test]
fn test_validate_platform_rejects_unknown() {
    let err = validate_platform("instagram").unwrap_err();
    assert!(err.contains("Unknown platform"), "error must say 'Unknown platform', got: {}", err);
}

#[test]
fn test_validate_platform_accepts_mastodon() {
    assert!(validate_platform("mastodon").is_ok());
}

#[test]
fn test_validate_platform_accepts_substack_notes() {
    assert!(validate_platform("substack_notes").is_ok());
}

#[test]
fn test_validate_platform_accepts_linkedin() {
    assert!(validate_platform("linkedin").is_ok());
}

// --- §validate_post_folder ---

#[test]
fn test_validate_post_folder_accepts_single_component() {
    assert!(validate_post_folder("my-post-2026").is_ok());
}

#[test]
fn test_validate_post_folder_rejects_path_with_slash() {
    assert!(validate_post_folder("sub/folder").is_err());
}

#[test]
fn test_validate_post_folder_rejects_dotdot() {
    assert!(validate_post_folder("../escape").is_err());
}

// --- §validate_repo_path ---

#[test]
fn test_validate_repo_path_returns_err_for_unregistered_path() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let state = make_state(vec![]);
    let result = validate_repo_path(dir.path().to_str().expect("utf-8 path"), &state);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("not registered"), "expected 'not registered', got: {}", msg);
}

#[test]
fn test_validate_repo_path_returns_ok_for_registered_path() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let canonical = std::fs::canonicalize(dir.path()).expect("canonicalize");
    let canonical_str = canonical.to_str().expect("utf-8").to_string();
    let repo = make_repo("r1", &canonical_str);
    let state = make_state(vec![repo]);
    let result = validate_repo_path(dir.path().to_str().expect("utf-8 path"), &state);
    assert_eq!(result.unwrap().canonical(), canonical_str);
}

// --- §read_platform_content ---

#[test]
fn test_read_platform_content_returns_content_when_file_exists() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    std::fs::write(dir.path().join("x.md"), "hello").expect("write x.md");
    let result = read_platform_content(dir.path(), "x");
    assert_eq!(result.unwrap(), "hello");
}

#[test]
fn test_read_platform_content_returns_err_when_file_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let err = read_platform_content(dir.path(), "x").unwrap_err();
    assert!(err.contains("not found"), "expected 'not found', got: {}", err);
}

// --- §load_account_ids ---

#[test]
fn test_load_account_ids_returns_empty_map_when_no_account_ids_key() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    crate::test_fixtures::write_config(dir.path(), r#"{"scheduler": {}}"#);
    let config_path = dir.path().join(".postlane/config.json");
    let result = load_account_ids(&config_path);
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_load_account_ids_returns_map_when_present() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    crate::test_fixtures::write_config(
        dir.path(),
        r#"{"scheduler": {"account_ids": {"x": "acc123"}}}"#,
    );
    let config_path = dir.path().join(".postlane/config.json");
    let map = load_account_ids(&config_path).unwrap();
    assert_eq!(map.get("x").and_then(|v| v.as_str()), Some("acc123"));
}

#[test]
fn test_load_account_ids_returns_err_when_config_missing() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    assert!(load_account_ids(dir.path()).is_err());
}

// --- §apply_scheduler_result (additional branch coverage) ---

#[test]
fn test_apply_scheduler_result_drops_non_https_platform_url() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "s1", Some("http://example.com"), "2026-05-01T00:00:00Z");
    assert!(!meta.platform_urls.contains_key("x"), "non-https url must not be stored");
}

#[test]
fn test_apply_scheduler_result_drops_empty_scheduler_id() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "", None, "2026-05-01T00:00:00Z");
    assert!(!meta.scheduler_ids.contains_key("x"), "empty scheduler_id must not be stored");
}

#[test]
fn test_apply_scheduler_result_stores_https_url() {
    let mut meta = PostMeta::default();
    apply_scheduler_result(&mut meta, "x", "s1", Some("https://x.com/post/99"), "2026-05-01T00:00:00Z");
    assert_eq!(meta.platform_urls.get("x").map(String::as_str), Some("https://x.com/post/99"));
}

// --- §resolve_mastodon_credential ---

#[test]
fn test_resolve_mastodon_credential_ok_when_both_present() {
    let result = resolve_mastodon_credential(
        Some("mastodon.social".to_string()),
        Some("tok123".to_string()),
    );
    assert_eq!(result.unwrap(), ("mastodon.social".to_string(), "tok123".to_string()));
}

#[test]
fn test_resolve_mastodon_credential_err_when_instance_missing() {
    let result = resolve_mastodon_credential(None, Some("tok123".to_string()));
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("mastodon") || msg.to_lowercase().contains("connect"),
        "error must guide user to connect Mastodon, got: {}", msg
    );
}

#[test]
fn test_resolve_mastodon_credential_err_when_token_missing() {
    let result = resolve_mastodon_credential(Some("mastodon.social".to_string()), None);
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(
        msg.to_lowercase().contains("mastodon") || msg.to_lowercase().contains("reconnect"),
        "error must guide user to reconnect, got: {}", msg
    );
}

// --- §record_scheduler_failure (split assertions) ---

#[test]
fn test_record_scheduler_failure_sets_status_failed() {
    let mut meta = PostMeta::default();
    record_scheduler_failure(&mut meta, "the error");
    assert_eq!(meta.status, Some(PostStatus::Failed));
}

#[test]
fn test_record_scheduler_failure_sets_error_message() {
    let mut meta = PostMeta::default();
    record_scheduler_failure(&mut meta, "the error");
    assert_eq!(meta.error.as_deref(), Some("the error"));
}

// --- §InFlightGuard ---

#[test]
fn test_in_flight_guard_increments_counter_on_creation() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let _guard = InFlightGuard::new(&counter);
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 1);
}

#[test]
fn test_in_flight_guard_decrements_counter_on_drop() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let _guard = InFlightGuard::new(&counter);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 1);
    }
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0,
        "counter must return to 0 after guard is dropped");
}

#[test]
fn test_in_flight_guard_multiple_guards_accumulate_and_decrement_independently() {
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let g1 = InFlightGuard::new(&counter);
    let g2 = InFlightGuard::new(&counter);
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 2,
        "two guards must give count of 2");
    drop(g1);
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 1,
        "dropping one guard must leave count at 1");
    drop(g2);
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0,
        "dropping both guards must leave count at 0");
}

// --- §resolve_config_json ---

#[test]
fn test_resolve_config_json_returns_postlane_config_when_postlane_dir_exists() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    std::fs::create_dir_all(dir.path().join(".postlane")).expect("create .postlane dir");
    let result = resolve_config_json(dir.path());
    assert_eq!(result, dir.path().join(".postlane").join("config.json"),
        "must resolve to .postlane/config.json when .postlane dir is present");
}

#[test]
fn test_resolve_config_json_returns_root_config_when_postlane_dir_absent() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    // No .postlane directory — workspace layout
    let result = resolve_config_json(dir.path());
    assert_eq!(result, dir.path().join("config.json"),
        "must resolve to root config.json when .postlane dir is absent");
}
