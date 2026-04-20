// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::types::PostMeta;
use std::fs;
use std::path::PathBuf;
use tauri::State;

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
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let post_folder = entry.path();
            if !post_folder.is_dir() {
                continue;
            }

            let meta_path = post_folder.join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            let meta_content = match fs::read_to_string(&meta_path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let meta: PostMeta = match serde_json::from_str(&meta_content) {
                Ok(meta) => meta,
                Err(_) => continue,
            };

            if meta.status != "sent" {
                continue;
            }

            let slug = post_folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let platforms = meta.platforms.join("+");

            let row = format!(
                "{},{},{},stub,{},{},0,0,0,0,\n",
                repo.name,
                slug,
                platforms,
                meta.llm_model.as_deref().unwrap_or("unknown"),
                meta.sent_at.as_deref().unwrap_or("")
            );

            csv.push_str(&row);
        }
    }

    Ok(csv)
}

#[tauri::command]
pub fn export_history_csv(state: State<AppState>) -> Result<String, String> {
    let csv_content = export_history_csv_impl(&state)?;
    Ok(format!("{} bytes", csv_content.len()))
}
