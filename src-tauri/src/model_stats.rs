// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::platform_constants::DENOMINATOR_UNIT_PLATFORM_APPROVAL;
use crate::post_meta::PostMeta;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Serialize, Clone, Debug)]
pub struct ModelStatsResponse {
    pub edit_rate: f64,
    pub edited_posts: u32,
    pub total_posts: u32,
    pub denominator_unit: String,
    pub pre_m19_post_count: u32,
}

fn tally_post_folder(post_path: &std::path::Path, resp: &mut ModelStatsResponse) {
    if !post_path.is_dir() {
        return;
    }
    let meta_path = post_path.join("meta.json");
    let Ok(meta) = PostMeta::load(&meta_path) else { return };

    for platform in meta.sent_platforms.keys() {
        resp.total_posts += 1;
        match &meta.edited_platforms {
            None => {
                resp.pre_m19_post_count += 1;
            }
            Some(edited) if edited.contains(platform) => {
                resp.edited_posts += 1;
            }
            Some(_) => {}
        }
    }
}

pub fn get_model_stats_impl(state: &AppState, home_dir: Option<&std::path::Path>) -> Result<ModelStatsResponse, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut resp = ModelStatsResponse {
        edit_rate: 0.0,
        edited_posts: 0,
        total_posts: 0,
        denominator_unit: DENOMINATOR_UNIT_PLATFORM_APPROVAL.to_string(),
        pre_m19_post_count: 0,
    };

    for repo in &repos.repos {
        let repo_path = PathBuf::from(&repo.path);
        if let Some(home) = home_dir {
            if !repo_path.starts_with(home) {
                log::warn!("[get_model_stats] skipping repo outside $HOME: {}", repo.path);
                continue;
            }
        }
        let posts_dir = repo_path.join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&posts_dir) else { continue };
        for entry in entries.flatten() {
            tally_post_folder(&entry.path(), &mut resp);
        }
    }

    resp.edit_rate = if resp.total_posts == 0 {
        0.0
    } else {
        resp.edited_posts as f64 / resp.total_posts as f64
    };

    Ok(resp)
}

#[tauri::command]
pub fn get_model_stats(state: State<'_, AppState>) -> Result<ModelStatsResponse, String> {
    get_model_stats_impl(&state, dirs::home_dir().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Repo;
    use crate::test_fixtures::{make_state, home_tmp};
    use std::fs;

    fn make_repo(dir: &std::path::Path) -> Repo {
        Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }

    fn write_meta_json(dir: &std::path::Path, folder: &str, meta_json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create dir");
        fs::write(p.join("meta.json"), meta_json).expect("write meta");
    }

    #[test]
    fn test_get_model_stats_treats_edited_platforms_none_as_not_edited() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_none_not_edited");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1", r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z"}}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 1);
        assert_eq!(resp.edited_posts, 0, "pre-M19 post must not count as edited");
        assert_eq!(resp.pre_m19_post_count, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_treats_edited_platforms_some_empty_as_not_edited() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_some_empty_not_edited");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1",
            r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z"},"edited_platforms":[]}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 1);
        assert_eq!(resp.edited_posts, 0, "Some([]) must not count as edited");
        assert_eq!(resp.pre_m19_post_count, 0, "Some([]) must not count as pre-M19");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_counts_edited_when_platform_in_edited_platforms() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_edited_when_in_list");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1",
            r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z"},"edited_platforms":["x"]}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 1);
        assert_eq!(resp.edited_posts, 1);
        assert_eq!(resp.pre_m19_post_count, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_does_not_count_edited_when_platform_not_in_edited_platforms() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_not_in_edited");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1",
            r#"{"sent_platforms":{"linkedin":"2026-01-01T00:00:00Z"},"edited_platforms":["x"]}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 1);
        assert_eq!(resp.edited_posts, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_counts_two_approvals_from_same_folder_independently() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_two_platforms");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1",
            r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z","linkedin":"2026-01-02T00:00:00Z"},"edited_platforms":["x"]}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 2, "one folder, two sent platforms → denominator = 2");
        assert_eq!(resp.edited_posts, 1, "only x was in edited_platforms");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_response_includes_denominator_unit() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_denominator_unit");
        let _ = fs::remove_dir_all(&dir);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.denominator_unit, DENOMINATOR_UNIT_PLATFORM_APPROVAL);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_response_includes_pre_m19_post_count() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_pre_m19_count");
        let _ = fs::remove_dir_all(&dir);
        write_meta_json(&dir, "p1", r#"{"sent_platforms":{"x":"2026-01-01T00:00:00Z"}}"#);
        write_meta_json(&dir, "p2",
            r#"{"sent_platforms":{"x":"2026-01-02T00:00:00Z"},"edited_platforms":[]}"#);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 2);
        assert_eq!(resp.pre_m19_post_count, 1, "only p1 has None edited_platforms");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_returns_zero_rate_when_no_sent_posts() {
        let home = dirs::home_dir().expect("home");
        let dir = home_tmp("ms_zero_rate");
        let _ = fs::remove_dir_all(&dir);
        let state = make_state(vec![make_repo(&dir)]);
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 0);
        assert_eq!(resp.edited_posts, 0);
        assert_eq!(resp.edit_rate, 0.0, "must not NaN or panic on zero denominator");
        assert!(!resp.edit_rate.is_nan());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_skips_repo_outside_home() {
        // /tmp is outside $HOME on macOS — must be skipped with no rows scanned
        let state = make_state(vec![make_repo(std::path::Path::new("/tmp/postlane_ms_outside_home"))]);
        let home = dirs::home_dir().expect("home");
        let resp = get_model_stats_impl(&state, Some(&home)).expect("ok");
        assert_eq!(resp.total_posts, 0, "repos outside $HOME must not be scanned");
    }
}
