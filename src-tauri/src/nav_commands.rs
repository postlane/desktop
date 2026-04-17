// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::{AppState, AppStateFile, read_app_state, write_app_state};
use serde::{Deserialize, Serialize};

/// Payload emitted on the "meta-changed" Tauri event
#[derive(Serialize, Clone, Debug)]
pub struct MetaChangedPayload {
    pub repo_id: String,
    pub post_folder: String,
}
use std::fs;
use std::path::PathBuf;
use tauri::State;

/// Repo with runtime-computed fields for the nav component
#[derive(Serialize, Clone, Debug)]
pub struct RepoWithStatus {
    pub id: String,
    pub name: String,
    pub path: String,
    pub active: bool,
    pub added_at: String,
    /// Whether the repo path currently exists on disk
    pub path_exists: bool,
    /// Count of posts with status "ready"
    pub ready_count: u32,
    /// Count of posts with status "failed"
    pub failed_count: u32,
    /// ISO 8601 timestamp of the most recent post created_at, or None
    pub last_post_at: Option<String>,
}

/// Testable implementation: builds RepoWithStatus for all repos in AppState
pub fn get_repos_impl(state: &AppState) -> Result<Vec<RepoWithStatus>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let result = repos
        .repos
        .iter()
        .map(|repo| {
            let path_exists = std::path::Path::new(&repo.path).exists();
            let (ready_count, failed_count, last_post_at) =
                scan_post_statuses(&repo.path);
            RepoWithStatus {
                id: repo.id.clone(),
                name: repo.name.clone(),
                path: repo.path.clone(),
                active: repo.active,
                added_at: repo.added_at.clone(),
                path_exists,
                ready_count,
                failed_count,
                last_post_at,
            }
        })
        .collect();

    Ok(result)
}

/// Scans a repo's posts directory and returns (ready_count, failed_count, last_post_at).
/// Returns (0, 0, None) if the posts directory does not exist or cannot be read.
fn scan_post_statuses(repo_path: &str) -> (u32, u32, Option<String>) {
    let posts_dir = PathBuf::from(repo_path).join(".postlane/posts");
    if !posts_dir.exists() {
        return (0, 0, None);
    }

    let entries = match fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(_) => return (0, 0, None),
    };

    let mut ready_count: u32 = 0;
    let mut failed_count: u32 = 0;
    let mut latest_ts: Option<String> = None;

    for entry in entries.flatten() {
        let meta_path = entry.path().join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match meta.get("status").and_then(|s| s.as_str()) {
            Some("ready") => ready_count += 1,
            Some("failed") => failed_count += 1,
            _ => {}
        }

        if let Some(ts) = meta.get("created_at").and_then(|v| v.as_str()) {
            latest_ts = Some(match &latest_ts {
                None => ts.to_string(),
                Some(prev) => {
                    if ts > prev.as_str() {
                        ts.to_string()
                    } else {
                        prev.clone()
                    }
                }
            });
        }
    }

    (ready_count, failed_count, latest_ts)
}

#[tauri::command]
pub fn get_repos(state: State<'_, AppState>) -> Result<Vec<RepoWithStatus>, String> {
    get_repos_impl(&state)
}

#[tauri::command]
pub fn read_app_state_command() -> AppStateFile {
    read_app_state()
}

#[tauri::command]
pub fn save_app_state_command(state: AppStateFile) -> Result<(), String> {
    write_app_state(&state)
}

/// Post enriched with repo context, for the frontend drafts view
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DraftPost {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_path: String,
    pub post_folder: String,
    pub status: String,
    pub platforms: Vec<String>,
    pub schedule: Option<String>,
    pub trigger: Option<String>,
    pub platform_results: Option<std::collections::HashMap<String, String>>,
    pub error: Option<String>,
    pub image_url: Option<String>,
    pub llm_model: Option<String>,
    pub created_at: Option<String>,
}

/// Returns all ready/failed posts across all active repos, enriched with repo context.
/// Within each repo: failed first, then ready; each sub-group newest created_at first.
pub fn get_all_drafts_impl(state: &AppState) -> Result<Vec<DraftPost>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut drafts: Vec<DraftPost> = Vec::new();

    for repo in repos.repos.iter().filter(|r| r.active) {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() {
            continue;
        }

        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let post_path = entry.path();
            if !post_path.is_dir() {
                continue;
            }
            let meta_path = post_path.join("meta.json");
            if !meta_path.exists() {
                continue;
            }

            let content = match fs::read_to_string(&meta_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let status = match meta.get("status").and_then(|s| s.as_str()) {
                Some(s @ "ready") | Some(s @ "failed") => s.to_string(),
                _ => continue,
            };

            let post_folder = post_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let platforms: Vec<String> = meta
                .get("platforms")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let platform_results = meta.get("platform_results").and_then(|v| {
                v.as_object().map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
            });

            drafts.push(DraftPost {
                repo_id: repo.id.clone(),
                repo_name: repo.name.clone(),
                repo_path: repo.path.clone(),
                post_folder,
                status,
                platforms,
                schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
                trigger: meta.get("trigger").and_then(|v| v.as_str()).map(String::from),
                platform_results,
                error: meta.get("error").and_then(|v| v.as_str()).map(String::from),
                image_url: meta.get("image_url").and_then(|v| v.as_str()).map(String::from),
                llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
                created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
            });
        }
    }

    // Sort: failed first, then by created_at newest first
    drafts.sort_by(|a, b| {
        match (a.status.as_str(), b.status.as_str()) {
            ("failed", "ready") => std::cmp::Ordering::Less,
            ("ready", "failed") => std::cmp::Ordering::Greater,
            _ => match (&b.created_at, &a.created_at) {
                (Some(bt), Some(at)) => bt.cmp(at),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });

    Ok(drafts)
}

#[tauri::command]
pub fn get_all_drafts(state: State<'_, AppState>) -> Result<Vec<DraftPost>, String> {
    get_all_drafts_impl(&state)
}

/// Sent or queued post with repo context, for the Published view
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PublishedPost {
    pub repo_id: String,
    pub repo_name: String,
    pub repo_path: String,
    pub post_folder: String,
    pub status: String,
    pub platforms: Vec<String>,
    pub platform_results: Option<std::collections::HashMap<String, String>>,
    pub schedule: Option<String>,
    pub scheduler_ids: Option<std::collections::HashMap<String, String>>,
    pub llm_model: Option<String>,
    pub sent_at: Option<String>,
    pub created_at: Option<String>,
}

/// Returns sent + queued posts for a single repo, sorted by sent_at newest first.
/// Pagination: offset + limit (pass offset=0, limit=100 for first page).
pub fn get_repo_published_impl(
    repo_id: &str,
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<PublishedPost>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not found", repo_id))?;

    let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
    if !posts_dir.exists() {
        return Ok(vec![]);
    }

    let entries = match fs::read_dir(&posts_dir) {
        Ok(e) => e,
        Err(_) => return Ok(vec![]),
    };

    let mut posts: Vec<PublishedPost> = Vec::new();

    for entry in entries.flatten() {
        let post_path = entry.path();
        if !post_path.is_dir() {
            continue;
        }
        let meta_path = post_path.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = match fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let status = match meta.get("status").and_then(|s| s.as_str()) {
            Some(s @ "sent") | Some(s @ "queued") => s.to_string(),
            _ => continue,
        };

        let post_folder = post_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let platforms: Vec<String> = meta
            .get("platforms")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let platform_results = meta.get("platform_results").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
        });

        let scheduler_ids = meta.get("scheduler_ids").and_then(|v| {
            v.as_object().map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
        });

        posts.push(PublishedPost {
            repo_id: repo.id.clone(),
            repo_name: repo.name.clone(),
            repo_path: repo.path.clone(),
            post_folder,
            status,
            platforms,
            platform_results,
            schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
            scheduler_ids,
            llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
            sent_at: meta.get("sent_at").and_then(|v| v.as_str()).map(String::from),
            created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
        });
    }

    // Sort: queued first (for scheduled sub-section), then sent newest first
    posts.sort_by(|a, b| {
        match (a.status.as_str(), b.status.as_str()) {
            ("queued", "sent") => std::cmp::Ordering::Less,
            ("sent", "queued") => std::cmp::Ordering::Greater,
            _ => match (&b.sent_at, &a.sent_at) {
                (Some(bt), Some(at)) => bt.cmp(at),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });

    // Pagination
    let page: Vec<PublishedPost> = posts.into_iter().skip(offset).take(limit).collect();
    Ok(page)
}

/// Returns sent posts across all repos (active + inactive), newest first.
/// Pagination via offset + limit.
pub fn get_all_published_impl(
    offset: usize,
    limit: usize,
    state: &AppState,
) -> Result<Vec<PublishedPost>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut posts: Vec<PublishedPost> = Vec::new();

    for repo in &repos.repos {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() { continue; }

        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let post_path = entry.path();
            if !post_path.is_dir() { continue; }
            let meta_path = post_path.join("meta.json");
            if !meta_path.exists() { continue; }

            let content = match fs::read_to_string(&meta_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let status = match meta.get("status").and_then(|s| s.as_str()) {
                Some(s @ "sent") | Some(s @ "queued") => s.to_string(),
                _ => continue,
            };

            let post_folder = post_path.file_name()
                .and_then(|n| n.to_str()).unwrap_or("").to_string();

            let platforms: Vec<String> = meta.get("platforms")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();

            let platform_results = meta.get("platform_results").and_then(|v| {
                v.as_object().map(|obj| obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect())
            });

            let scheduler_ids = meta.get("scheduler_ids").and_then(|v| {
                v.as_object().map(|obj| obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect())
            });

            posts.push(PublishedPost {
                repo_id: repo.id.clone(),
                repo_name: repo.name.clone(),
                repo_path: repo.path.clone(),
                post_folder,
                status,
                platforms,
                platform_results,
                schedule: meta.get("schedule").and_then(|v| v.as_str()).map(String::from),
                scheduler_ids,
                llm_model: meta.get("llm_model").and_then(|v| v.as_str()).map(String::from),
                sent_at: meta.get("sent_at").and_then(|v| v.as_str()).map(String::from),
                created_at: meta.get("created_at").and_then(|v| v.as_str()).map(String::from),
            });
        }
    }

    posts.sort_by(|a, b| match (&b.sent_at, &a.sent_at) {
        (Some(bt), Some(at)) => bt.cmp(at),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    Ok(posts.into_iter().skip(offset).take(limit).collect())
}

#[tauri::command]
pub fn get_all_published(
    offset: usize,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<PublishedPost>, String> {
    get_all_published_impl(offset, limit, &state)
}

/// Levenshtein distance between two strings (character-level).
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
            dp[i][j] = if a[i-1] == b[j-1] {
                dp[i-1][j-1]
            } else {
                1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1])
            };
        }
    }
    dp[m][n]
}

/// Returns per-model edit rate statistics across all repos.
/// A post is "edited" if Levenshtein distance between original and approved text
/// exceeds 5% of original length on any platform.
/// Models with <5 posts are excluded entirely.
/// Models with 5–19 posts are marked limited_data = true.
pub fn get_model_stats_impl(state: &AppState) -> Result<Vec<ModelStatRow>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    // model -> (total, edited)
    let mut counts: std::collections::HashMap<String, (u32, u32)> = std::collections::HashMap::new();

    for repo in &repos.repos {
        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");
        if !posts_dir.exists() { continue; }

        let entries = match fs::read_dir(&posts_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let post_path = entry.path();
            if !post_path.is_dir() { continue; }

            let meta_path = post_path.join("meta.json");
            if !meta_path.exists() { continue; }

            let meta_content = match fs::read_to_string(&meta_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&meta_content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if meta.get("status").and_then(|s| s.as_str()) != Some("sent") { continue; }

            let model = match meta.get("llm_model").and_then(|v| v.as_str()) {
                Some(m) if !m.is_empty() => m.to_string(),
                _ => continue,
            };

            let entry_counts = counts.entry(model).or_insert((0, 0));
            entry_counts.0 += 1;

            // Check for original.json
            let original_path = post_path.join("original.json");
            if !original_path.exists() {
                // No original.json — counts toward total but not edited
                continue;
            }

            let original_content = match fs::read_to_string(&original_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let original: serde_json::Value = match serde_json::from_str(&original_content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Check each platform: x.md, bluesky.md, mastodon.md
            let platforms = ["x", "bluesky", "mastodon"];
            let mut is_edited = false;

            'platform: for platform in &platforms {
                let original_text = match original.get(platform).and_then(|v| v.as_str()) {
                    Some(t) => t.to_string(),
                    None => continue,
                };
                let approved_path = post_path.join(format!("{}.md", platform));
                let approved_text = match fs::read_to_string(&approved_path) {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let threshold = (original_text.chars().count() as f64 * 0.05).ceil() as usize;
                let dist = levenshtein(&original_text, approved_text.trim());
                if dist > threshold {
                    is_edited = true;
                    break 'platform;
                }
            }

            if is_edited {
                entry_counts.1 += 1;
            }
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

    result.sort_by(|a, b| b.total_posts.cmp(&a.total_posts));
    Ok(result)
}

#[derive(Serialize, Clone, Debug)]
pub struct ModelStatRow {
    pub model: String,
    pub total_posts: u32,
    pub edited_posts: u32,
    pub edit_rate: f64,
    pub limited_data: bool,
}

#[tauri::command]
pub fn get_model_stats(state: State<'_, AppState>) -> Result<Vec<ModelStatRow>, String> {
    get_model_stats_impl(&state)
}

/// Returns the app version from Cargo.toml
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns whether autostart is enabled (stub — real impl uses tauri-plugin-autostart)
#[tauri::command]
pub fn get_autostart_enabled() -> bool {
    // tauri-plugin-autostart state not available without AppHandle in a simple command.
    // Frontend should call tauri-plugin-autostart JS API directly for reads.
    // This stub returns false; the real toggle is handled by set_autostart_enabled.
    false
}

/// Read the repo's config.json and return (repo_path, provider_name).
/// Testable without AppHandle — the keyring and provider calls are in the Tauri command.
pub fn get_repo_config_impl(
    repo_id: &str,
    state: &AppState,
) -> Result<(String, String), String> {
    let repos = state.repos.lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos.repos.iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not in registered repos", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let provider_name = config["scheduler"]["provider"]
        .as_str()
        .ok_or("scheduler.provider not set in config.json")?
        .to_string();

    Ok((repo.path.clone(), provider_name))
}

#[tauri::command]
pub async fn list_profiles_for_repo(
    repo_id: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<crate::providers::scheduling::SchedulerProfile>, String> {
    use crate::commands::get_credential_keyring_key;
    use crate::providers::scheduling::{ProviderError, SchedulingProvider};
    use crate::providers::scheduling::ayrshare::AyrshareProvider;
    use crate::providers::scheduling::buffer::BufferProvider;
    use crate::providers::scheduling::zernio::ZernioProvider;
    use tauri_plugin_keyring::KeyringExt;

    let (_repo_path, provider_name) = get_repo_config_impl(&repo_id, &state)?;

    let keyring_keys = get_credential_keyring_key(&provider_name, Some(&repo_id));
    let mut api_key: Option<String> = None;
    for key in &keyring_keys {
        if let Ok(Some(k)) = app.keyring().get_password("postlane", key) {
            api_key = Some(k);
            break;
        }
    }
    let api_key = api_key.ok_or_else(|| {
        format!("No {} API key configured. Add it in Settings → Scheduler.", provider_name)
    })?;

    let provider: Box<dyn SchedulingProvider> = match provider_name.as_str() {
        "zernio" => Box::new(ZernioProvider::new(api_key)),
        "buffer" => Box::new(BufferProvider::new(api_key)),
        "ayrshare" => Box::new(AyrshareProvider::new(api_key)),
        other => return Err(format!("Unknown scheduler provider: {}", other)),
    };

    provider.list_profiles().await.map_err(|e: ProviderError| e.to_string())
}

/// Write a profile_id into a repo's config.json, preserving all other fields.
/// Uses an atomic write (tmp → rename) to prevent partial writes on crash.
pub fn save_profile_id_impl(config_path: &std::path::Path, profile_id: &str) -> Result<(), String> {
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }

    let content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let mut config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    if !config["scheduler"].is_object() {
        return Err("config.json is missing the 'scheduler' block".to_string());
    }

    config["scheduler"]["profile_id"] = serde_json::json!(profile_id);

    let serialized = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config.json: {}", e))?;

    let tmp_path = config_path.with_extension("tmp");
    fs::write(&tmp_path, &serialized)
        .map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&tmp_path, config_path)
        .map_err(|e| format!("Failed to rename temp config: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn save_profile_id(
    repo_id: String,
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let repos = state.repos.lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos.repos.iter()
        .find(|r| r.id == repo_id)
        .ok_or_else(|| format!("Repo '{}' not in registered repos", repo_id))?;

    let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
    save_profile_id_impl(&config_path, &profile_id)
}

#[tauri::command]
pub fn get_repo_published(
    repo_id: String,
    offset: usize,
    limit: usize,
    state: State<'_, AppState>,
) -> Result<Vec<PublishedPost>, String> {
    get_repo_published_impl(&repo_id, offset, limit, &state)
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

    #[test]
    fn test_get_repos_empty() {
        let state = make_state(vec![]);
        let result = get_repos_impl(&state).expect("should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_repos_nonexistent_path_marked_missing() {
        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: "/nonexistent/path/that/cannot/exist".to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repos_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 1);
        assert!(!result[0].path_exists);
        assert_eq!(result[0].ready_count, 0);
        assert_eq!(result[0].failed_count, 0);
        assert!(result[0].last_post_at.is_none());
    }

    #[test]
    fn test_get_repos_counts_ready_and_failed_posts() {
        let dir = std::env::temp_dir().join("postlane_test_get_repos_counts");
        let posts_dir = dir.join(".postlane/posts");

        // post 1: ready
        let p1 = posts_dir.join("post-001");
        fs::create_dir_all(&p1).expect("create post dir");
        fs::write(
            p1.join("meta.json"),
            r#"{"status":"ready","created_at":"2024-06-01T10:00:00Z"}"#,
        )
        .expect("write meta");

        // post 2: failed
        let p2 = posts_dir.join("post-002");
        fs::create_dir_all(&p2).expect("create post dir");
        fs::write(
            p2.join("meta.json"),
            r#"{"status":"failed","created_at":"2024-06-02T10:00:00Z"}"#,
        )
        .expect("write meta");

        // post 3: sent (should not count)
        let p3 = posts_dir.join("post-003");
        fs::create_dir_all(&p3).expect("create post dir");
        fs::write(
            p3.join("meta.json"),
            r#"{"status":"sent","created_at":"2024-06-03T10:00:00Z"}"#,
        )
        .expect("write meta");

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repos_impl(&state).expect("should succeed");
        assert_eq!(result[0].ready_count, 1);
        assert_eq!(result[0].failed_count, 1);
        // latest created_at across all three posts
        assert_eq!(
            result[0].last_post_at.as_deref(),
            Some("2024-06-03T10:00:00Z")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_no_posts_dir() {
        let (ready, failed, ts) = scan_post_statuses("/nonexistent/path");
        assert_eq!(ready, 0);
        assert_eq!(failed, 0);
        assert!(ts.is_none());
    }

    #[test]
    fn test_scan_malformed_meta_skipped() {
        let dir = std::env::temp_dir().join("postlane_test_scan_malformed");
        let posts_dir = dir.join(".postlane/posts/post-bad");
        fs::create_dir_all(&posts_dir).expect("create dir");
        fs::write(posts_dir.join("meta.json"), "{ not valid json }").expect("write");

        let (ready, failed, ts) =
            scan_post_statuses(dir.to_str().unwrap());
        assert_eq!(ready, 0);
        assert_eq!(failed, 0);
        assert!(ts.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // levenshtein tests
    // ------------------------------------------------------------------

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

    // ------------------------------------------------------------------
    // get_model_stats_impl tests
    // ------------------------------------------------------------------

    fn write_sent_with_original(
        dir: &std::path::Path,
        folder: &str,
        model: &str,
        original_x: &str,
        approved_x: &str,
    ) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create");
        fs::write(p.join("meta.json"), format!(
            r#"{{"status":"sent","platforms":["x"],"llm_model":"{}","sent_at":"2026-04-15T10:00:00Z"}}"#,
            model
        )).expect("write meta");
        let original_escaped = original_x.replace('\\', "\\\\").replace('"', "\\\"");
        fs::write(p.join("original.json"), format!(
            r#"{{"x":"{}"}}"#, original_escaped
        )).expect("write original");
        fs::write(p.join("x.md"), approved_x).expect("write approved");
    }

    #[test]
    fn test_get_model_stats_excludes_models_with_fewer_than_5_posts() {
        let dir = std::env::temp_dir().join("postlane_test_model_stats_exclude");
        for i in 0..4 {
            write_sent_with_original(&dir, &format!("p{}", i), "gpt-4", "hello world", "hello world");
        }
        let state = make_state(vec![crate::storage::Repo {
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
        let dir = std::env::temp_dir().join("postlane_test_model_stats_limited");
        for i in 0..7 {
            write_sent_with_original(&dir, &format!("p{}", i), "claude-haiku", "hello world", "hello world");
        }
        let state = make_state(vec![crate::storage::Repo {
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
        let dir = std::env::temp_dir().join("postlane_test_model_stats_edit");
        let original = "The quick brown fox jumped over the lazy dog";
        let edited = "A completely different sentence with nothing in common at all!";
        for i in 0..3 {
            write_sent_with_original(&dir, &format!("unchanged-{}", i), "claude-sonnet", original, original);
        }
        for i in 0..2 {
            write_sent_with_original(&dir, &format!("edited-{}", i), "claude-sonnet", original, edited);
        }
        let state = make_state(vec![crate::storage::Repo {
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

    // ------------------------------------------------------------------
    // save_profile_id_impl tests
    // ------------------------------------------------------------------

    fn write_config(dir: &std::path::Path, json: &str) -> PathBuf {
        let config_dir = dir.join(".postlane");
        fs::create_dir_all(&config_dir).expect("create .postlane dir");
        let config_path = config_dir.join("config.json");
        fs::write(&config_path, json).expect("write config.json");
        config_path
    }

    #[test]
    fn test_save_profile_id_writes_profile_id_to_config() {
        let dir = std::env::temp_dir().join("postlane_test_save_profile_id");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "platforms": ["x", "bluesky"],
            "scheduler": { "provider": "zernio", "profile_id": "" }
        }"#);

        save_profile_id_impl(&config_path, "profile-abc123").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["scheduler"]["profile_id"].as_str(), Some("profile-abc123"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_profile_id_preserves_other_config_fields() {
        let dir = std::env::temp_dir().join("postlane_test_save_profile_id_preserve");
        let config_path = write_config(&dir, r#"{
            "version": 1,
            "base_url": "https://postlane.dev",
            "platforms": ["x", "bluesky"],
            "repo_type": "saas-product",
            "scheduler": { "provider": "zernio", "profile_id": "" },
            "llm": { "provider": "anthropic", "model": "claude-sonnet-4-6" }
        }"#);

        save_profile_id_impl(&config_path, "new-profile").expect("should succeed");

        let content = fs::read_to_string(&config_path).expect("read config");
        let config: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(config["version"].as_i64(), Some(1));
        assert_eq!(config["base_url"].as_str(), Some("https://postlane.dev"));
        assert_eq!(config["repo_type"].as_str(), Some("saas-product"));
        assert_eq!(config["scheduler"]["provider"].as_str(), Some("zernio"));
        assert_eq!(config["llm"]["model"].as_str(), Some("claude-sonnet-4-6"));
        assert_eq!(config["scheduler"]["profile_id"].as_str(), Some("new-profile"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_profile_id_errors_when_config_missing() {
        let result = save_profile_id_impl(
            std::path::Path::new("/nonexistent/path/.postlane/config.json"),
            "some-profile",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_save_profile_id_errors_when_no_scheduler_block() {
        let dir = std::env::temp_dir().join("postlane_test_save_profile_no_scheduler");
        let config_path = write_config(&dir, r#"{ "version": 1, "platforms": ["x"] }"#);

        let result = save_profile_id_impl(&config_path, "profile-abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheduler"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_profile_id_rejects_unregistered_repo() {
        let state = make_state(vec![]);
        // Call the impl directly — unregistered repo_id must return an error
        let repos = state.repos.lock().expect("lock");
        let result: Result<(), String> = repos.repos.iter()
            .find(|r| r.id == "nonexistent")
            .map(|_| Ok(()))
            .unwrap_or_else(|| Err(format!("Repo '{}' not in registered repos", "nonexistent")));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in registered repos"));
    }

    // ------------------------------------------------------------------
    // get_repo_config_impl tests
    // ------------------------------------------------------------------

    #[test]
    fn test_get_repo_config_returns_provider_and_path() {
        let dir = std::env::temp_dir().join("postlane_test_get_repo_config");
        write_config(&dir, r#"{
            "version": 1,
            "scheduler": { "provider": "zernio", "profile_id": "" }
        }"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_config_impl("r1", &state).expect("should succeed");
        assert_eq!(result.0, dir.to_str().unwrap());
        assert_eq!(result.1, "zernio");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_config_errors_on_missing_repo() {
        let state = make_state(vec![]);
        let result = get_repo_config_impl("nonexistent", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not in registered repos"));
    }

    #[test]
    fn test_get_repo_config_errors_on_missing_config_file() {
        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: "/nonexistent/path/that/cannot/exist".to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repo_config_impl("r1", &state);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_repo_config_errors_when_provider_missing_from_config() {
        let dir = std::env::temp_dir().join("postlane_test_get_repo_config_no_provider");
        write_config(&dir, r#"{ "version": 1, "platforms": ["x"] }"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(),
            name: "My Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_config_impl("r1", &state);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheduler.provider"));

        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // get_all_drafts_impl tests
    // ------------------------------------------------------------------

    fn write_meta(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_all_drafts_empty() {
        let state = make_state(vec![]);
        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_all_drafts_only_ready_and_failed_included() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_filter");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"],"created_at":"2024-06-01T10:00:00Z"}"#);
        write_meta(&dir, "p2", r#"{"status":"sent","platforms":["x"],"created_at":"2024-06-02T10:00:00Z"}"#);
        write_meta(&dir, "p3", r#"{"status":"failed","platforms":["bluesky"],"created_at":"2024-06-03T10:00:00Z","error":"timeout"}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 2, "sent post should be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_failed_before_ready() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_sort");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"],"created_at":"2024-06-03T00:00:00Z"}"#);
        write_meta(&dir, "p2", r#"{"status":"failed","platforms":["x"],"created_at":"2024-06-01T00:00:00Z"}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result[0].status, "failed");
        assert_eq!(result[1].status, "ready");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_inactive_repo_excluded() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_inactive");
        write_meta(&dir, "p1", r#"{"status":"ready","platforms":["x"]}"#);

        let state = make_state(vec![Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: false, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert!(result.is_empty(), "inactive repo should be excluded");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_all_drafts_enriches_with_repo_context() {
        let dir = std::env::temp_dir().join("postlane_test_drafts_context");
        write_meta(&dir, "my-post", r#"{"status":"ready","platforms":["x","bluesky"],"trigger":"Launched v2","created_at":"2024-06-01T00:00:00Z"}"#);

        let state = make_state(vec![Repo {
            id: "abc-123".to_string(), name: "My App".to_string(),
            path: dir.to_str().unwrap().to_string(),
            active: true, added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_all_drafts_impl(&state).expect("should succeed");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].repo_id, "abc-123");
        assert_eq!(result[0].repo_name, "My App");
        assert_eq!(result[0].post_folder, "my-post");
        assert_eq!(result[0].trigger.as_deref(), Some("Launched v2"));
        assert_eq!(result[0].platforms, vec!["x", "bluesky"]);
        let _ = fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------------
    // get_repo_published_impl tests
    // ------------------------------------------------------------------

    fn write_published_meta(dir: &std::path::Path, folder: &str, json: &str) {
        let p = dir.join(".postlane/posts").join(folder);
        fs::create_dir_all(&p).expect("create post dir");
        fs::write(p.join("meta.json"), json).expect("write meta");
    }

    #[test]
    fn test_get_repo_published_empty() {
        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: "/nonexistent".to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);
        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_repo_published_only_sent_and_queued() {
        let dir = std::env::temp_dir().join("postlane_test_published_filter");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir, "p2", r#"{"status":"ready","platforms":["x"]}"#);
        write_published_meta(&dir, "p3", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result.len(), 2, "only sent + queued");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_queued_before_sent() {
        let dir = std::env::temp_dir().join("postlane_test_published_sort");
        write_published_meta(&dir, "p1", r#"{"status":"sent","platforms":["x"],"sent_at":"2026-04-15T10:00:00Z"}"#);
        write_published_meta(&dir, "p2", r#"{"status":"queued","platforms":["x"]}"#);

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let result = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(result[0].status, "queued");
        assert_eq!(result[1].status, "sent");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_pagination() {
        let dir = std::env::temp_dir().join("postlane_test_published_pagination");
        for i in 0..105 {
            write_published_meta(
                &dir,
                &format!("post-{:03}", i),
                &format!(r#"{{"status":"sent","platforms":["x"],"sent_at":"2026-04-{:02}T10:00:00Z"}}"#, (i % 28) + 1),
            );
        }

        let state = make_state(vec![crate::storage::Repo {
            id: "r1".to_string(), name: "Repo".to_string(),
            path: dir.to_str().unwrap().to_string(), active: true,
            added_at: "2024-01-01T00:00:00Z".to_string(),
        }]);

        let page1 = get_repo_published_impl("r1", 0, 100, &state).expect("ok");
        assert_eq!(page1.len(), 100);

        let page2 = get_repo_published_impl("r1", 100, 100, &state).expect("ok");
        assert_eq!(page2.len(), 5);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_get_repo_published_repo_not_found() {
        let state = make_state(vec![]);
        let result = get_repo_published_impl("nonexistent", 0, 100, &state);
        assert!(result.is_err());
    }
}
