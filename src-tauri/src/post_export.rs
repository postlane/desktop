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
