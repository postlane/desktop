// SPDX-License-Identifier: BUSL-1.1
// Shared test helpers — only compiled in #[cfg(test)] context.

use crate::app_state::AppState;
use crate::storage::{Repo, ReposConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Single mutex for all tests that read/write the real app_state.json path.
/// Both app_state and app_state_ops tests must use this to prevent races.
static APP_STATE_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

pub fn app_state_mutex() -> &'static Mutex<()> {
    APP_STATE_MUTEX.get_or_init(|| Mutex::new(()))
}

/// RAII guard for tests that touch the real app_state.json path.
/// Acquires the mutex (poison-safe), inits the dir, removes any stale file or
/// directory at the path on acquire, then removes it again on drop — so cleanup
/// is guaranteed even when an assertion panics.
pub struct AppStateGuard {
    pub path: PathBuf,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl AppStateGuard {
    pub fn acquire() -> Self {
        let _lock = app_state_mutex().lock().unwrap_or_else(|p| p.into_inner());
        crate::init::init_postlane_dir().expect("init postlane dir");
        let path = crate::app_state::app_state_path().expect("app state path");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(&path);
        AppStateGuard { path, _lock }
    }
}

impl Drop for AppStateGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_dir_all(&self.path);
    }
}

static TEST_REPOS_SEQ: AtomicU64 = AtomicU64::new(0);

pub fn make_state(repos: Vec<Repo>) -> AppState {
    let n = TEST_REPOS_SEQ.fetch_add(1, Ordering::Relaxed);
    let repos_path = std::env::temp_dir()
        .join(format!("postlane_test_repos_{}_{}.json", std::process::id(), n));
    AppState::new_with_path(ReposConfig { version: 1, repos }, repos_path)
}

pub fn make_repo(id: &str, path: &str) -> Repo {
    Repo {
        id: id.to_string(),
        name: id.to_string(),
        path: path.to_string(),
        active: true,
        added_at: "2024-01-01T00:00:00Z".to_string(),
    }
}

pub fn home_tmp(name: &str) -> PathBuf {
    let home = dirs::home_dir().expect("home dir must exist in tests");
    home.join(".postlane_test_tmp").join(name)
}

pub fn write_config(dir: &Path, json: &str) {
    let d = dir.join(".postlane");
    fs::create_dir_all(&d).expect("create .postlane");
    fs::write(d.join("config.json"), json).expect("write config.json");
}

pub fn write_meta(dir: &Path, folder: &str, json: &str) {
    let p = dir.join(".postlane/posts").join(folder);
    fs::create_dir_all(&p).expect("create post dir");
    fs::write(p.join("meta.json"), json).expect("write meta.json");
}

pub fn setup_post_dir(repo_dir: &Path, post_folder: &str) {
    fs::create_dir_all(repo_dir.join(".postlane/posts").join(post_folder))
        .expect("create post dir");
}
