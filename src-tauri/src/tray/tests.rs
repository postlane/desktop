// SPDX-License-Identifier: BUSL-1.1
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[tokio::test]
async fn test_drain_returns_immediately_when_counter_is_zero() {
    let counter = Arc::new(AtomicUsize::new(0));
    wait_for_in_flight_drain(&counter, Duration::from_millis(1000)).await;
    // Must return without hanging
}

#[tokio::test]
async fn test_drain_waits_until_counter_reaches_zero() {
    let counter = Arc::new(AtomicUsize::new(1));
    let clone = counter.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        clone.fetch_sub(1, Ordering::AcqRel);
    });
    wait_for_in_flight_drain(&counter, Duration::from_millis(2000)).await;
    assert_eq!(counter.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn test_drain_exits_after_deadline_even_if_counter_nonzero() {
    let counter = Arc::new(AtomicUsize::new(1)); // never decremented
    let start = std::time::Instant::now();
    wait_for_in_flight_drain(&counter, Duration::from_millis(120)).await;
    assert!(start.elapsed() < Duration::from_millis(600), "exceeded 600ms safety margin");
    assert_eq!(counter.load(Ordering::Acquire), 1, "counter unchanged");
}

fn status(ready: u32, failed: u32) -> TrayStatus {
    TrayStatus { ready_count: ready, failed_count: failed }
}

#[test]
fn test_badge_label_zero() {
    assert_eq!(status(0, 0).badge_label(), "");
}

#[test]
fn test_badge_label_single_ready() {
    assert_eq!(status(1, 0).badge_label(), "1");
}

#[test]
fn test_badge_label_combined() {
    assert_eq!(status(3, 2).badge_label(), "5");
}

#[test]
fn test_badge_label_cap_at_99() {
    assert_eq!(status(99, 0).badge_label(), "99");
}

#[test]
fn test_badge_label_over_99() {
    assert_eq!(status(100, 0).badge_label(), "99+");
    assert_eq!(status(50, 60).badge_label(), "99+");
}

#[test]
fn test_badge_not_red_with_only_ready() {
    assert!(!status(5, 0).badge_is_red());
}

#[test]
fn test_badge_red_with_any_failed() {
    assert!(status(0, 1).badge_is_red());
    assert!(status(5, 1).badge_is_red());
}

#[test]
fn test_badge_not_red_when_empty() {
    assert!(!status(0, 0).badge_is_red());
}

#[test]
fn test_ready_label_hidden_when_zero() {
    assert!(status(0, 0).ready_label().is_none());
}

#[test]
fn test_ready_label_singular() {
    assert_eq!(status(1, 0).ready_label().unwrap(), "1 draft ready");
}

#[test]
fn test_ready_label_plural() {
    assert_eq!(status(3, 0).ready_label().unwrap(), "3 drafts ready");
}

#[test]
fn test_approve_all_hidden_when_zero_ready() {
    assert!(status(0, 2).approve_all_label().is_none());
}

#[test]
fn test_approve_all_shown_when_ready() {
    assert!(status(2, 0).approve_all_label().is_some());
    assert_eq!(status(2, 0).approve_all_label().unwrap(), "Approve all ready (2)");
}

#[test]
fn test_failed_label_hidden_when_zero() {
    assert!(status(0, 0).failed_label().is_none());
}

#[test]
fn test_failed_label_shown() {
    assert_eq!(status(0, 3).failed_label().unwrap(), "3 failed");
}

#[test]
fn test_approve_confirm_singular() {
    assert_eq!(status(1, 0).approve_confirm_message(), "Send 1 post to scheduler?");
}

#[test]
fn test_approve_confirm_plural() {
    assert_eq!(status(5, 0).approve_confirm_message(), "Send 5 posts to scheduler?");
}

fn write_md_and_meta(repo_dir: &std::path::Path, folder: &str, platform: &str, meta_json: Option<&str>) {
    std::fs::create_dir_all(repo_dir.join(".git")).expect("create .git");
    let post_dir = repo_dir.join(".postlane/posts").join(folder);
    std::fs::create_dir_all(&post_dir).expect("create post dir");
    std::fs::write(post_dir.join(format!("{}.md", platform)), "content").expect("write md");
    if let Some(json) = meta_json {
        std::fs::write(post_dir.join("meta.json"), json).expect("write meta.json");
    }
}

#[test]
fn test_compute_tray_status_counts_ready_and_failed() {
    use crate::test_fixtures::{make_repo, make_state};
    let dir = tempfile::TempDir::new().expect("create temp dir");
    write_md_and_meta(dir.path(), "post-ready-1", "x", None);
    write_md_and_meta(dir.path(), "post-ready-2", "x", None);
    write_md_and_meta(dir.path(), "post-failed-1", "x", Some(r#"{"status":"failed"}"#));
    let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
    let result = compute_tray_status(&state);
    assert_eq!(result.ready_count, 2, "expected 2 ready");
    assert_eq!(result.failed_count, 1, "expected 1 failed");
}

#[test]
fn test_compute_tray_status_returns_zeros_when_no_posts() {
    use crate::test_fixtures::{make_repo, make_state};
    let dir = tempfile::TempDir::new().expect("create temp dir");
    std::fs::create_dir_all(dir.path().join(".git")).expect("create .git");
    let state = make_state(vec![make_repo("r1", dir.path().to_str().unwrap())]);
    let result = compute_tray_status(&state);
    assert_eq!(result, TrayStatus { ready_count: 0, failed_count: 0 });
}

#[test]
fn test_compute_tray_status_returns_zeros_when_no_repos() {
    use crate::test_fixtures::make_state;
    let state = make_state(vec![]);
    let result = compute_tray_status(&state);
    assert_eq!(result, TrayStatus { ready_count: 0, failed_count: 0 });
}
