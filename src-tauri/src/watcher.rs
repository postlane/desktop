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

fn watch_workspace_children<F>(
    repo_id: String,
    workspace_path: &Path,
    watchers: &WatcherMap,
    mut on_change: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(Vec<PathBuf>) + Send + 'static,
{
    let posts_dirs: Vec<PathBuf> = crate::workspace::discover_child_repos(workspace_path)
        .into_iter()
        .map(|c| c.join(".postlane/posts"))
        .filter(|p| p.exists())
        .collect();
    if posts_dirs.is_empty() {
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
    for posts_dir in &posts_dirs {
        watcher.watch(posts_dir, RecursiveMode::Recursive)?;
    }
    watchers.lock().unwrap_or_else(|e| e.into_inner()).insert(repo_id, watcher);
    Ok(())
}

/// Starts watching a repo's posts directory for meta.json changes
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

    let posts_dir = repo_path.join(".postlane").join("posts");

    // If posts/ doesn't exist yet, return Ok - no-op, not an error
    if !posts_dir.exists() {
        return Ok(());
    }

    // Create watcher with closure that filters to meta.json only
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

    // Watch recursively to detect changes inside post subdirectories
    let mut watcher = watcher;
    watcher.watch(&posts_dir, RecursiveMode::Recursive)?;

    // Store watcher in HashMap to prevent it being dropped
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
    fn test_watch_repo_nonexistent_posts_dir() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(dir.path().join(".git")).expect("Failed to create test dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, _rx) = mpsc::channel();

        // Should not error when posts/ doesn't exist
        let result = watch_repo(
            "test-repo".to_string(),
            dir.path(),
            &watchers,
            move |_paths| {
                tx.send(()).unwrap();
            },
        );

        assert!(result.is_ok(), "Should not error on missing posts/ dir");
        assert_eq!(watchers.lock().unwrap().len(), 0, "Should not create watcher");
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

    // ── Workspace watcher (20.8.3) ────────────────────────────────────────────

    #[test]
    fn test_watch_repo_workspace_creates_watcher_for_child_posts_dirs() {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(ws.path().join("repo-a/.git")).expect("git a");
        fs::create_dir_all(ws.path().join("repo-b/.git")).expect("git b");
        fs::create_dir_all(ws.path().join("repo-a/.postlane/posts")).expect("posts a");
        fs::create_dir_all(ws.path().join("repo-b/.postlane/posts")).expect("posts b");
        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let result = watch_repo("ws".to_string(), ws.path(), &watchers, |_| {});
        assert!(result.is_ok(), "workspace watch should not error");
        assert_eq!(watchers.lock().unwrap().len(), 1, "one watcher entry per workspace repo_id");
        stop_all_watchers(&watchers);
    }

    #[test]
    fn test_watch_repo_workspace_no_child_posts_dirs_is_noop() {
        let ws = tempfile::TempDir::new().expect("create temp dir");
        fs::create_dir_all(ws.path().join("repo-a/.git")).expect("git");
        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let result = watch_repo("ws".to_string(), ws.path(), &watchers, |_| {});
        assert!(result.is_ok());
        assert_eq!(watchers.lock().unwrap().len(), 0, "no watcher when no child has posts/");
    }
}
