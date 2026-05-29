// SPDX-License-Identifier: BUSL-1.1

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub type WatcherMap = Mutex<HashMap<String, RecommendedWatcher>>;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(500);

/// Returns true if enough time has passed since `last_fired` to allow the next call.
/// `None` means the callback has never fired — always allow it.
pub fn should_fire_after_debounce(last_fired: Option<Instant>, min_interval: Duration) -> bool {
    match last_fired {
        None => true,
        Some(last) => last.elapsed() >= min_interval,
    }
}

/// Watches `{workspace}/posts/` recursively for meta.json changes (22.2.5).
///
/// All draft subdirectories under the workspace (one per child repo's `posts_dir`)
/// are covered by a single recursive watcher on `{workspace}/posts/`.
/// If the directory does not yet exist, this is a silent no-op — 22.2.4 guarantees
/// it is created eagerly when the workspace is registered.
fn watch_workspace_children<F>(
    repo_id: String,
    workspace_path: &Path,
    watchers: &WatcherMap,
    mut on_change: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<PathBuf>) + Send + 'static,
{
    let posts_dir = workspace_path.join("posts");
    if !posts_dir.exists() {
        return Ok(());
    }
    let last_fired: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let meta_changes: Vec<PathBuf> = event
                    .paths
                    .iter()
                    .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("meta.json"))
                    .cloned()
                    .collect();
                if meta_changes.is_empty() {
                    return;
                }
                let mut guard = last_fired.lock().unwrap_or_else(|e| e.into_inner());
                if should_fire_after_debounce(*guard, DEBOUNCE_INTERVAL) {
                    *guard = Some(Instant::now());
                    drop(guard);
                    on_change(meta_changes);
                }
            }
        },
        notify::Config::default(),
    )?;
    watcher.watch(&posts_dir, RecursiveMode::Recursive)?;
    watchers.lock().unwrap_or_else(|e| e.into_inner()).insert(repo_id, watcher);
    Ok(())
}

/// Starts watching a repo's `.postlane/` directory for meta.json changes.
///
/// Watches `.postlane/` rather than `.postlane/posts/` so the watcher is active
/// even when `posts/` does not yet exist at startup. The `meta.json` filename
/// filter ensures only post-state changes trigger the callback.
pub fn watch_repo<F>(
    repo_id: String,
    repo_path: &Path,
    watchers: &WatcherMap,
    mut on_change: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<PathBuf>) + Send + 'static,
{
    if crate::workspace::is_workspace_root(repo_path) {
        return watch_workspace_children(repo_id, repo_path, watchers, on_change);
    }

    let watch_dir = repo_path.join(".postlane");

    // .postlane/ is always present for registered repos (config.json requires it),
    // but guard defensively so a missing directory is a silent no-op, not a crash.
    if !watch_dir.exists() {
        log::warn!("watch_repo: .postlane/ not found at {:?}, skipping watcher", repo_path);
        return Ok(());
    }

    let last_fired: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
    let watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let meta_changes: Vec<PathBuf> = event
                    .paths
                    .iter()
                    .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("meta.json"))
                    .cloned()
                    .collect();

                if meta_changes.is_empty() {
                    return;
                }
                let mut guard = last_fired.lock().unwrap_or_else(|e| e.into_inner());
                if should_fire_after_debounce(*guard, DEBOUNCE_INTERVAL) {
                    *guard = Some(Instant::now());
                    drop(guard);
                    on_change(meta_changes);
                }
            }
        },
        notify::Config::default(),
    )?;

    let mut watcher = watcher;
    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;

    watchers.lock().unwrap_or_else(|e| e.into_inner()).insert(repo_id, watcher);

    Ok(())
}

/// Stops watching a repo
pub fn stop_watcher(repo_id: &str, watchers: &WatcherMap) {
    // Removing from HashMap drops the watcher and stops watching
    watchers.lock().unwrap_or_else(|e| e.into_inner()).remove(repo_id);
}

/// Stops all watchers (called on shutdown)
pub fn stop_all_watchers(watchers: &WatcherMap) {
    watchers.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    // ── Debounce logic ────────────────────────────────────────────────────────

    #[test]
    fn test_debounce_fires_when_never_triggered() {
        assert!(should_fire_after_debounce(None, Duration::from_millis(500)));
    }

    #[test]
    fn test_debounce_suppresses_call_within_interval() {
        let recent = Instant::now();
        assert!(!should_fire_after_debounce(Some(recent), Duration::from_millis(500)));
    }

    #[test]
    fn test_debounce_fires_after_interval_elapsed() {
        let old = Instant::now() - Duration::from_millis(600);
        assert!(should_fire_after_debounce(Some(old), Duration::from_millis(500)));
    }

    #[test]
    fn test_debounce_boundary_exactly_at_interval() {
        let exactly = Instant::now() - Duration::from_millis(500);
        assert!(should_fire_after_debounce(Some(exactly), Duration::from_millis(500)));
    }

    #[test]
    fn test_watch_repo_registers_watcher_when_posts_dir_absent() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        // .git and .postlane exist but .postlane/posts does NOT
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        fs::create_dir_all(dir.path().join(".postlane")).expect("create .postlane");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, _rx) = mpsc::channel();

        let result = watch_repo(
            "test-repo".to_string(),
            dir.path(),
            &watchers,
            move |_paths| { tx.send(()).unwrap(); },
        );

        assert!(result.is_ok(), "watch_repo must succeed when posts/ is absent");
        assert_eq!(watchers.lock().unwrap().len(), 1,
            "watcher must be registered even when posts/ does not yet exist");
    }

    #[test]
    fn test_watch_repo_detects_meta_json_changes() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join(".postlane").join("posts");
        let meta_dir = posts_dir.join("test-post");

        // Pre-create .git so is_workspace_root returns false (single-repo path).
        // Pre-create file before starting the watcher — IN_MODIFY is more reliable
        // on CI than IN_CREATE.
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        fs::create_dir_all(&meta_dir).expect("Failed to create post dir");
        let meta_path = meta_dir.join("meta.json");
        fs::write(&meta_path, r#"{"status": "initial"}"#).expect("Failed to write initial meta.json");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            dir.path(),
            &watchers,
            move |paths| {
                tx.send(paths).unwrap();
            },
        )
        .expect("Failed to start watcher");

        // Give watcher time to initialize — CI runners need more time than local dev
        thread::sleep(Duration::from_millis(500));

        // Modify the existing file — triggers IN_MODIFY which is reliable on all platforms
        fs::write(&meta_path, r#"{"status": "draft"}"#).expect("Failed to modify meta.json");

        // Wait for event — 5s ceiling is generous for even the slowest CI runners
        let result = rx.recv_timeout(Duration::from_millis(5000));
        assert!(result.is_ok(), "Should receive meta.json change event");

        if let Ok(paths) = result {
            assert!(!paths.is_empty(), "Should have at least one path");
            assert!(
                paths.iter().any(|p| p.ends_with("meta.json")),
                "Should contain meta.json path"
            );
        }

        stop_watcher("test-repo", &watchers);
    }

    #[test]
    fn test_watch_repo_ignores_md_file_changes() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join(".postlane").join("posts");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            dir.path(),
            &watchers,
            move |paths| {
                tx.send(paths).unwrap();
            },
        )
        .expect("Failed to start watcher");

        // Give watcher time to initialize
        thread::sleep(Duration::from_millis(100));

        // Write .md file (should be ignored)
        let md_path = posts_dir.join("test-post").join("x.md");
        fs::create_dir_all(md_path.parent().unwrap()).expect("Failed to create post dir");
        fs::write(&md_path, "Post content").expect("Failed to write x.md");

        // Wait and verify no event received
        let result = rx.recv_timeout(Duration::from_millis(300));
        assert!(result.is_err(), "Should NOT receive event for .md file changes");

        stop_watcher("test-repo", &watchers);
    }

    #[test]
    fn test_stop_watcher() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let posts_dir = dir.path().join(".postlane").join("posts");
        fs::create_dir_all(dir.path().join(".git")).expect("create .git");
        fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, _rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            dir.path(),
            &watchers,
            move |_paths| {
                tx.send(()).unwrap();
            },
        )
        .expect("Failed to start watcher");

        assert_eq!(watchers.lock().unwrap().len(), 1, "Should have one watcher");

        // Stop watcher
        stop_watcher("test-repo", &watchers);
        assert_eq!(watchers.lock().unwrap().len(), 0, "Should have no watchers");
    }

    #[test]
    fn test_stop_all_watchers() {
        let watchers: WatcherMap = Mutex::new(HashMap::new());

        // Add multiple watchers to different repos
        let _dirs: Vec<tempfile::TempDir> = (0..3).map(|i| {
            let dir = tempfile::TempDir::new().expect("create temp dir");
            let posts_dir = dir.path().join(".postlane").join("posts");
            fs::create_dir_all(dir.path().join(".git")).expect("create .git");
            fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

            let (tx, _rx) = mpsc::channel();
            watch_repo(
                format!("repo-{}", i),
                dir.path(),
                &watchers,
                move |_paths| {
                    tx.send(()).unwrap();
                },
            )
            .expect("Failed to start watcher");
            dir
        }).collect();

        assert_eq!(watchers.lock().unwrap().len(), 3, "Should have 3 watchers");

        // Stop all
        stop_all_watchers(&watchers);
        assert_eq!(watchers.lock().unwrap().len(), 0, "Should have no watchers");
    }

    #[test]
    fn test_stop_watcher_does_not_panic_on_poisoned_mutex() {
        let arc = std::sync::Arc::new(Mutex::new(HashMap::<String, RecommendedWatcher>::new()));
        let arc_clone = arc.clone();
        let _ = thread::spawn(move || {
            let _guard = arc_clone.lock().unwrap();
            panic!("intentional poison");
        }).join();
        // Must not panic even though the mutex is poisoned
        stop_watcher("repo-1", &arc);
    }

    #[test]
    fn test_stop_all_watchers_does_not_panic_on_poisoned_mutex() {
        let arc = std::sync::Arc::new(Mutex::new(HashMap::<String, RecommendedWatcher>::new()));
        let arc_clone = arc.clone();
        let _ = thread::spawn(move || {
            let _guard = arc_clone.lock().unwrap();
            panic!("intentional poison");
        }).join();
        stop_all_watchers(&arc);
    }

    // ── Workspace watcher (22.2.5) ────────────────────────────────────────────

    /// 22.2.5 — workspace watcher watches {workspace}/posts/ recursively, not per-child dirs.
    #[test]
    fn test_watch_repo_workspace_watches_workspace_posts_dir() {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        // Create {workspace}/posts/ — the v1.4 canonical location
        fs::create_dir_all(ws.path().join("posts")).expect("create posts dir");
        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let result = watch_repo("ws".to_string(), ws.path(), &watchers, |_| {});
        assert!(result.is_ok(), "workspace watch must not error when posts/ exists");
        assert_eq!(watchers.lock().unwrap().len(), 1, "one watcher for workspace posts/ dir");
        stop_all_watchers(&watchers);
    }

    /// 22.2.5 — noop when {workspace}/posts/ is absent.
    #[test]
    fn test_watch_repo_workspace_noop_when_posts_dir_absent() {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        // No posts/ directory — workspace not yet initialised
        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let result = watch_repo("ws".to_string(), ws.path(), &watchers, |_| {});
        assert!(result.is_ok());
        assert_eq!(watchers.lock().unwrap().len(), 0, "no watcher when posts/ absent");
    }

    /// Legacy compatibility — per-repo watcher still works for repos in `repos` array.
    #[test]
    fn test_watch_repo_workspace_creates_watcher_for_child_posts_dirs() {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        // v1.4 workspace: {workspace}/posts/ present
        fs::create_dir_all(ws.path().join("posts").join("frontend")).expect("posts/frontend");
        fs::create_dir_all(ws.path().join("posts").join("backend")).expect("posts/backend");
        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let result = watch_repo("ws".to_string(), ws.path(), &watchers, |_| {});
        assert!(result.is_ok(), "workspace watch should not error");
        assert_eq!(watchers.lock().unwrap().len(), 1, "one watcher entry per workspace repo_id");
        stop_all_watchers(&watchers);
    }
}
