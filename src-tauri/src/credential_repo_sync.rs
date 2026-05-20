// SPDX-License-Identifier: BUSL-1.1

//! Syncs provider config to repos whose project_id matches a given credential.

use crate::app_state::AppState;
use std::path::{Path, PathBuf};

/// Returns the paths of all repos whose `.postlane/config.json` has the given `project_id`.
pub(crate) fn collect_matching_repo_paths(project_id: &str, state: &AppState) -> Vec<PathBuf> {
    let Ok(repos) = state.repos.lock() else {
        return vec![];
    };
    repos
        .repos
        .iter()
        .filter_map(|repo| {
            let config_path = Path::new(&repo.path).join(".postlane/config.json");
            let content = std::fs::read_to_string(&config_path).ok()?;
            let config: serde_json::Value = serde_json::from_str(&content).ok()?;
            if config["project_id"].as_str() == Some(project_id) {
                Some(PathBuf::from(&repo.path))
            } else {
                None
            }
        })
        .collect()
}

/// Writes `provider` into `config.local.json` for every repo matching `project_id`.
pub(crate) fn write_provider_to_matching_repos(project_id: &str, provider: &str, state: &AppState) {
    for repo_path in collect_matching_repo_paths(project_id, state) {
        if let Err(e) =
            crate::config_merge::write_scheduler_provider_to_local_config(&repo_path, provider)
        {
            log::warn!(
                "[save_scheduler_credential] write provider to {}: {}",
                repo_path.display(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_repo, make_state};

    fn make_test_state_with_repo(repo_path: &str) -> AppState {
        make_state(vec![make_repo("test-repo-id", repo_path)])
    }

    #[test]
    fn collect_matching_repo_paths_returns_empty_when_no_repos() {
        let state = make_state(vec![]);
        let paths = collect_matching_repo_paths("proj-abc", &state);
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_matching_repo_paths_returns_empty_when_config_missing() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let paths = collect_matching_repo_paths("proj-abc", &state);
        assert!(paths.is_empty(), "missing config.json must not match");
    }

    #[test]
    fn collect_matching_repo_paths_returns_path_when_project_id_matches() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"my-proj"}"#)
            .expect("write config.json");
        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        let paths = collect_matching_repo_paths("my-proj", &state);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], dir.path());
    }

    #[test]
    fn write_provider_to_matching_repos_no_op_when_no_repos() {
        let state = make_state(vec![]);
        write_provider_to_matching_repos("proj-abc", "zernio", &state);
    }

    #[test]
    fn write_provider_to_matching_repos_writes_provider_when_project_id_matches() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"proj-abc"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":""}}"#)
            .expect("write config.local.json");

        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        write_provider_to_matching_repos("proj-abc", "zernio", &state);

        let written = std::fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(v["scheduler"]["provider"].as_str(), Some("zernio"));
    }

    #[test]
    fn write_provider_to_matching_repos_skips_non_matching_project_id() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        std::fs::create_dir_all(&postlane).expect("mkdir .postlane");
        std::fs::write(postlane.join("config.json"), r#"{"project_id":"other-proj"}"#)
            .expect("write config.json");
        std::fs::write(postlane.join("config.local.json"), r#"{"scheduler":{"provider":""}}"#)
            .expect("write config.local.json");

        let state = make_test_state_with_repo(dir.path().to_str().unwrap());
        write_provider_to_matching_repos("proj-abc", "zernio", &state);

        let written = std::fs::read_to_string(postlane.join("config.local.json")).expect("read");
        let v: serde_json::Value = serde_json::from_str(&written).expect("parse");
        assert_eq!(
            v["scheduler"]["provider"].as_str(),
            Some(""),
            "non-matching repo must not be modified"
        );
    }
}
