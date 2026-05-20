// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::PostMeta;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

fn post_folder_csv_row(repo_name: &str, post_folder: &Path) -> Option<String> {
    if !post_folder.is_dir() {
        return None;
    }
    let meta_path = post_folder.join("meta.json");
    let meta_content = match fs::read_to_string(&meta_path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("cannot read {}: {e}", meta_path.display());
            return None;
        }
    };
    let meta: PostMeta = match serde_json::from_str(&meta_content) {
        Ok(m) => m,
        Err(e) => {
            log::warn!("cannot parse {}: {e}", meta_path.display());
            return None;
        }
    };
    if meta.status != "sent" {
        return None;
    }
    let slug = post_folder.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    Some(format!(
        "{},{},{},stub,{},{},0,0,0,0,\n",
        repo_name,
        slug,
        meta.platforms.join("+"),
        meta.llm_model.as_deref().unwrap_or("unknown"),
        meta.sent_at.as_deref().unwrap_or(""),
    ))
}

pub fn export_history_csv_impl(state: &AppState) -> Result<String, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut csv = String::from("repo,slug,platforms,scheduler,model,sent_at,likes,reposts,replies,impressions,view_urls\n");

    for repo in &repos.repos {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }
        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("cannot read posts dir {}: {e}", posts_dir.display());
                continue;
            }
        };
        for entry in entries.flatten() {
            if let Some(row) = post_folder_csv_row(&repo.name, &entry.path()) {
                csv.push_str(&row);
            }
        }
    }

    Ok(csv)
}

#[tauri::command]
pub fn export_history_csv(state: State<AppState>) -> Result<String, String> {
    let csv_content = export_history_csv_impl(&state)?;
    Ok(format!("{} bytes", csv_content.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::make_state;

    fn write_meta_json(dir: &std::path::Path, json: &str) {
        fs::create_dir_all(dir).expect("create post dir");
        fs::write(dir.join("meta.json"), json).expect("write meta.json");
    }

    #[test]
    fn test_post_folder_csv_row_returns_none_for_non_sent_post() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = tmp.path().join("my-post");
        write_meta_json(
            &post_dir,
            r#"{"status":"ready","platforms":["x"],"schedule":null,"trigger":null,"scheduler_ids":null,"platform_results":null,"platform_urls":null,"error":null,"image_url":null,"image_source":null,"image_attribution":null,"llm_model":null,"created_at":null,"sent_at":null}"#,
        );
        let row = post_folder_csv_row("myrepo", &post_dir);
        assert!(row.is_none(), "status=ready must yield None");
    }

    #[test]
    fn test_post_folder_csv_row_returns_none_for_directory_without_meta() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = tmp.path().join("no-meta");
        fs::create_dir_all(&post_dir).expect("create dir");
        // no meta.json written
        let row = post_folder_csv_row("myrepo", &post_dir);
        assert!(row.is_none(), "missing meta.json must yield None");
    }

    #[test]
    fn test_post_folder_csv_row_returns_csv_for_sent_post() {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = tmp.path().join("sent-post");
        write_meta_json(
            &post_dir,
            r#"{"status":"sent","platforms":["x","bluesky"],"schedule":null,"trigger":null,"scheduler_ids":null,"platform_results":null,"platform_urls":null,"error":null,"image_url":null,"image_source":null,"image_attribution":null,"llm_model":"claude-3-sonnet","created_at":null,"sent_at":"2024-06-01T10:00:00Z"}"#,
        );
        let row = post_folder_csv_row("myrepo", &post_dir);
        assert!(row.is_some(), "status=sent must yield Some");
        let line = row.unwrap();
        assert!(line.starts_with("myrepo,"), "first field must be repo name");
        assert!(line.contains("x+bluesky"), "platforms must be joined with +");
        assert!(line.contains("claude-3-sonnet"), "model must appear");
        assert!(line.contains("2024-06-01T10:00:00Z"), "sent_at must appear");
    }

    #[test]
    fn test_export_history_csv_impl_returns_header_only_when_no_repos() {
        let state = make_state(vec![]);
        let result = export_history_csv_impl(&state);
        assert!(result.is_ok(), "empty state must not error");
        let csv = result.unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 1, "must have exactly one line (the header)");
        assert!(
            lines[0].starts_with("repo,slug,platforms"),
            "header must start with repo,slug,platforms"
        );
    }
}
