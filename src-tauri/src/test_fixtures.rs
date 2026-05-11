// SPDX-License-Identifier: BUSL-1.1
// Shared test helpers — only compiled in #[cfg(test)] context.

use crate::app_state::AppState;
use crate::storage::{Repo, ReposConfig};
use std::fs;
use std::path::{Path, PathBuf};

pub fn make_state(repos: Vec<Repo>) -> AppState {
    AppState::new(ReposConfig { version: 1, repos })
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
