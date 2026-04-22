// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::State;

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate().take(m + 1) {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().take(n + 1) {
        *cell = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

#[derive(Serialize, Clone, Debug)]
pub struct ModelStatRow {
    pub model: String,
    pub total_posts: u32,
    pub edited_posts: u32,
    pub edit_rate: f64,
    pub limited_data: bool,
}

/// Returns `true` if any platform's approved text differs from the original by
/// more than 5% (Levenshtein distance).
fn post_was_edited(post_path: &std::path::Path, original: &serde_json::Value) -> bool {
    const PLATFORMS: [&str; 3] = ["x", "bluesky", "mastodon"];
    for platform in &PLATFORMS {
        let Some(original_text) = original.get(platform).and_then(|v| v.as_str()) else {
            continue;
        };
        let approved_path = post_path.join(format!("{}.md", platform));
        let Ok(approved_text) = fs::read_to_string(&approved_path) else {
            continue;
        };
        let threshold = (original_text.chars().count() as f64 * 0.05).ceil() as usize;
        if levenshtein(original_text, approved_text.trim()) > threshold {
            return true;
        }
    }
    false
}

/// Tries to load `original.json` for a post and check whether it was edited.
/// Returns `Some(true/false)` if the file exists and is valid; `None` otherwise.
fn check_edit_status(post_path: &std::path::Path) -> Option<bool> {
    let original_path = post_path.join("original.json");
    if !original_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&original_path).ok()?;
    let original: serde_json::Value = serde_json::from_str(&content).ok()?;
    Some(post_was_edited(post_path, &original))
}

/// Accumulates `(total, edited)` counts for a single post directory into `counts`.
fn tally_post_stats(
    post_path: &std::path::Path,
    counts: &mut std::collections::HashMap<String, (u32, u32)>,
) {
    if !post_path.is_dir() {
        return;
    }
    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return;
    }
    let Ok(meta_content) = fs::read_to_string(&meta_path) else { return };
    let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_content) else { return };

    if meta.get("status").and_then(|s| s.as_str()) != Some("sent") {
        return;
    }
    let model = match meta.get("llm_model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return,
    };

    let entry_counts = counts.entry(model).or_insert((0, 0));
    entry_counts.0 += 1;

    if check_edit_status(post_path).unwrap_or(false) {
        entry_counts.1 += 1;
    }
}

pub fn get_model_stats_impl(state: &AppState) -> Result<Vec<ModelStatRow>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut counts: std::collections::HashMap<String, (u32, u32)> = std::collections::HashMap::new();

    for repo in &repos.repos {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            tally_post_stats(&entry.path(), &mut counts);
        }
    }

    let mut result: Vec<ModelStatRow> = counts
        .into_iter()
        .filter(|(_, (total, _))| *total >= 5)
        .map(|(model, (total, edited))| {
            let edit_rate = if total > 0 { edited as f64 / total as f64 } else { 0.0 };
            ModelStatRow {
                model,
                total_posts: total,
                edited_posts: edited,
                edit_rate,
                limited_data: total < 20,
            }
        })
        .collect();

    result.sort_by_key(|b| std::cmp::Reverse(b.total_posts));
    Ok(result)
}

#[tauri::command]
pub fn get_model_stats(state: State<'_, AppState>) -> Result<Vec<ModelStatRow>, String> {
    get_model_stats_impl(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::storage::{Repo, ReposConfig};
    use std::fs;

    fn make_state(repos: Vec<Repo>) -> AppState {
        AppState::new(ReposConfig { version: 1, repos })
    }

    fn write_sent_with_original(
        dir: &std::path::Path,
        folder: &str,
        model: &str,
        original_x: &str,
        approved_x: &str,
    ) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create");
        fs::write(
            p.join("meta.json"),
            format!(r#"{{"status":"sent","platforms":["x"],"llm_model":"{}","sent_at":"2026-04-15T10:00:00Z"}}"#, model),
        ).expect("write meta");
        let original_escaped = original_x.replace('\\', "\\\\").replace('"', "\\\"");
        fs::write(p.join("original.json"), format!(r#"{{"x":"{}"}}"#, original_escaped)).expect("write original");
        fs::write(p.join("x.md"), approved_x).expect("write approved");
    }

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_single_insertion() {
        assert_eq!(levenshtein("cat", "cats"), 1);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein("", "abc"), 3);
        assert_eq!(levenshtein("abc", ""), 3);
    }

    #[test]
    fn test_get_model_stats_excludes_models_with_fewer_than_5_posts() {
        let dir = std::env::temp_dir().join("postlane_test_model_stats_exclude_ms");
        for i in 0..4 {
            write_sent_with_original(&dir, &format!("p{}", i), "gpt-4", "hello world", "hello world");
        }
        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_model_stats_impl(&state).expect("ok");
        assert!(result.is_empty(), "model with 4 posts should be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_marks_limited_data_for_5_to_19_posts() {
        let dir = std::env::temp_dir().join("postlane_test_model_stats_limited_ms");
        for i in 0..7 {
            write_sent_with_original(&dir, &format!("p{}", i), "claude-haiku", "hello world", "hello world");
        }
        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_model_stats_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert!(result[0].limited_data, "7 posts should be limited_data");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_model_stats_counts_edited_posts() {
        let dir = std::env::temp_dir().join("postlane_test_model_stats_edit_ms");
        let original = "The quick brown fox jumped over the lazy dog";
        let edited = "A completely different sentence with nothing in common at all!";
        for i in 0..3 {
            write_sent_with_original(&dir, &format!("unchanged-{}", i), "claude-sonnet", original, original);
        }
        for i in 0..2 {
            write_sent_with_original(&dir, &format!("edited-{}", i), "claude-sonnet", original, edited);
        }
        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_model_stats_impl(&state).expect("ok");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].total_posts, 5);
        assert_eq!(result[0].edited_posts, 2);
        let _ = fs::remove_dir_all(&dir);
    }
}
