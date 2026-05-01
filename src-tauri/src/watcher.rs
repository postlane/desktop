// SPDX-License-Identifier: BUSL-1.1

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub type WatcherMap = Mutex<HashMap<String, RecommendedWatcher>>;

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
    let posts_dir = repo_path.join(".postlane").join("posts");

    // If posts/ doesn't exist yet, return Ok - no-op, not an error
    if !posts_dir.exists() {
        return Ok(());
    }

    // Create watcher with closure that filters to meta.json only
    let watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Filter to meta.json changes only
                let meta_changes: Vec<PathBuf> = event
                    .paths
                    .iter()
                    .filter(|p| p.file_name().and_then(|n| n.to_str()) == Some("meta.json"))
                    .cloned()
                    .collect();

                if !meta_changes.is_empty() {
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
    use std::time::Duration;

    #[test]
    fn test_watch_repo_nonexistent_posts_dir() {
        let dir = std::env::temp_dir().join("postlane_test_watcher_nonexistent");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, _rx) = mpsc::channel();

        // Should not error when posts/ doesn't exist
        let result = watch_repo(
            "test-repo".to_string(),
            &dir,
            &watchers,
            move |_paths| {
                tx.send(()).unwrap();
            },
        );

        assert!(result.is_ok(), "Should not error on missing posts/ dir");
        assert_eq!(watchers.lock().unwrap().len(), 0, "Should not create watcher");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watch_repo_detects_meta_json_changes() {
        let dir = std::env::temp_dir().join("postlane_test_watcher_meta");
        let posts_dir = dir.join(".postlane").join("posts");
        let meta_dir = posts_dir.join("test-post");

        // Pre-create directory AND file before starting the watcher.
        // inotify IN_CREATE (new file) is less reliable on CI than IN_MODIFY
        // (existing file changed). By writing a placeholder first we ensure
        // the event we actually wait for is a modify, not a create.
        fs::create_dir_all(&meta_dir).expect("Failed to create post dir");
        let meta_path = meta_dir.join("meta.json");
        fs::write(&meta_path, r#"{"status": "initial"}"#).expect("Failed to write initial meta.json");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            &dir,
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
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_watch_repo_ignores_md_file_changes() {
        let dir = std::env::temp_dir().join("postlane_test_watcher_ignore");
        let posts_dir = dir.join(".postlane").join("posts");
        fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            &dir,
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
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stop_watcher() {
        let dir = std::env::temp_dir().join("postlane_test_watcher_stop");
        let posts_dir = dir.join(".postlane").join("posts");
        fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

        let watchers: WatcherMap = Mutex::new(HashMap::new());
        let (tx, _rx) = mpsc::channel();

        // Start watcher
        watch_repo(
            "test-repo".to_string(),
            &dir,
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

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stop_all_watchers() {
        let watchers: WatcherMap = Mutex::new(HashMap::new());

        // Add multiple watchers to different repos
        for i in 0..3 {
            let dir = std::env::temp_dir().join(format!("postlane_test_watcher_all_{}", i));
            let posts_dir = dir.join(".postlane").join("posts");
            fs::create_dir_all(&posts_dir).expect("Failed to create posts dir");

            let (tx, _rx) = mpsc::channel();
            watch_repo(
                format!("repo-{}", i),
                &dir,
                &watchers,
                move |_paths| {
                    tx.send(()).unwrap();
                },
            )
            .expect("Failed to start watcher");
        }

        assert_eq!(watchers.lock().unwrap().len(), 3, "Should have 3 watchers");

        // Stop all
        stop_all_watchers(&watchers);
        assert_eq!(watchers.lock().unwrap().len(), 0, "Should have no watchers");

        // Cleanup
        for i in 0..3 {
            let dir = std::env::temp_dir().join(format!("postlane_test_watcher_all_{}", i));
            let _ = fs::remove_dir_all(&dir);
        }
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
}
