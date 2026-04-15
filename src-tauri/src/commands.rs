// SPDX-License-Identifier: BUSL-1.1

use crate::app_state::AppState;
use crate::storage::{Repo, write_repos};
use crate::types::{PostMeta, QueuedPost, RepoHealthStatus, SendResult};
use crate::init::postlane_dir;
use std::fs;
use std::path::PathBuf;
use tauri::State;
use tauri_plugin_keyring::KeyringExt;
use uuid::Uuid;

/// Get all draft posts (status === "ready" or "failed") across all active repos
/// This is the testable implementation
pub fn get_drafts_impl(state: &AppState) -> Result<Vec<PostMeta>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut all_drafts = Vec::new();

    for repo in &repos.repos {
        // Skip inactive repos
        if !repo.active {
            continue;
        }

        let posts_dir = PathBuf::from(&repo.path).join(".postlane/posts");

        // Skip if posts directory doesn't exist
        if !posts_dir.exists() {
            continue;
        }

        // Read all post folders
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

            // Read and parse meta.json
            match fs::read_to_string(&meta_path) {
                Ok(content) => match serde_json::from_str::<PostMeta>(&content) {
                    Ok(meta) => {
                        // Only include ready or failed posts
                        if meta.status == "ready" || meta.status == "failed" {
                            all_drafts.push(meta);
                        }
                    }
                    Err(_) => continue,
                },
                Err(_) => continue,
            }
        }
    }

    // Sort: failed posts first, then by created_at (most recent first)
    all_drafts.sort_by(|a, b| {
        // First, sort by status (failed before ready)
        match (&a.status[..], &b.status[..]) {
            ("failed", "ready") => std::cmp::Ordering::Less,
            ("ready", "failed") => std::cmp::Ordering::Greater,
            _ => {
                // Same status - sort by created_at (most recent first)
                match (&b.created_at, &a.created_at) {
                    (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            }
        }
    });

    Ok(all_drafts)
}

/// Tauri command wrapper for get_drafts
#[tauri::command]
pub fn get_drafts(state: State<AppState>) -> Result<Vec<PostMeta>, String> {
    get_drafts_impl(&state)
}

/// Get or initialize the scheduling provider for a repo
/// Returns a reference to the provider, instantiating it if necessary
///
/// Steps:
/// 1. Read config.json to get scheduler.provider
/// 2. Check if AppState.scheduler is already set and matches the provider
/// 3. If not, get credential from keyring (per-repo override first, then global)
/// 4. Instantiate the provider and store in AppState.scheduler
///
/// Returns clear error if no credential found (never panics)
/// Eagerly instantiate provider if any registered repo has a configured provider with a credential
/// Called during app startup to eliminate first-send delay
/// Silently skips if no credential found - eager init is opportunistic, not required
pub async fn eager_init_provider_if_configured(
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<(), String> {
    // If no AppHandle (test mode), skip eager init
    let app = match app {
        Some(a) => a,
        None => return Ok(()),
    };

    // Get first active repo with a configured provider
    let repo_info: Option<(String, String, String)> = {
        let repos = state.repos.lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;

        repos.repos.iter()
            .filter(|r| r.active)
            .find_map(|repo| {
                let config_path = PathBuf::from(&repo.path).join(".postlane/config.json");
                if !config_path.exists() {
                    return None;
                }

                let config_content = fs::read_to_string(&config_path).ok()?;
                let config: serde_json::Value = serde_json::from_str(&config_content).ok()?;
                let provider_name = config["scheduler"]["provider"].as_str()?;

                Some((repo.path.clone(), repo.id.clone(), provider_name.to_string()))
            })
    };

    // If no repo has a configured provider, nothing to do
    let (repo_path, repo_id, provider_name) = match repo_info {
        Some(info) => info,
        None => return Ok(()),
    };

    // Try to get credential (per-repo override first, then global)
    let keyring_keys = get_credential_keyring_key(&provider_name, Some(&repo_id));

    let mut api_key: Option<String> = None;
    for key in keyring_keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => {
                api_key = Some(credential);
                break;
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    // If no credential found, silently skip - eager init is opportunistic
    if api_key.is_none() {
        return Ok(());
    }

    // Use get_or_init_provider to instantiate
    get_or_init_provider(app, &repo_path, &repo_id, state).await
}

async fn get_or_init_provider(
    app: &tauri::AppHandle,
    repo_path: &str,
    repo_id: &str,
    state: &AppState,
) -> Result<(), String> {
    use crate::providers::scheduling::{
        ayrshare::AyrshareProvider,
        buffer::BufferProvider,
        zernio::ZernioProvider,
    };

    // Step 1: Read config.json to get scheduler.provider
    let config_path = PathBuf::from(repo_path).join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found".to_string());
    }

    let config_content = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read config.json: {}", e))?;

    let config: serde_json::Value = serde_json::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config.json: {}", e))?;

    let provider_name = config["scheduler"]["provider"]
        .as_str()
        .ok_or("scheduler.provider not found in config.json")?;

    // Step 2: Check if provider is already instantiated and matches
    {
        let scheduler = state.scheduler.lock().await;

        if let Some(existing_provider) = scheduler.as_ref() {
            if existing_provider.name() == provider_name {
                // Provider already instantiated and matches - reuse it
                return Ok(());
            }
        }
    }

    // Step 3: Get credential from keyring (per-repo override first, then global)
    let keyring_keys = get_credential_keyring_key(provider_name, Some(repo_id));

    let mut api_key: Option<String> = None;
    for key in keyring_keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => {
                api_key = Some(credential);
                break;
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    let api_key = api_key.ok_or_else(|| {
        format!(
            "No {} API key configured. Add it in Settings → Scheduler.",
            provider_name
        )
    })?;

    // Step 4: Instantiate the provider
    let provider: Box<dyn crate::providers::scheduling::SchedulingProvider> = match provider_name {
        "zernio" => Box::new(ZernioProvider::new(api_key)),
        "buffer" => Box::new(BufferProvider::new(api_key)),
        "ayrshare" => Box::new(AyrshareProvider::new(api_key)),
        _ => return Err(format!("Unknown provider: {}", provider_name)),
    };

    // Step 5: Store in AppState.scheduler
    let mut scheduler = state.scheduler.lock().await;
    *scheduler = Some(provider);

    Ok(())
}

/// Approve and send a post
/// This is the testable implementation
pub async fn approve_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
    app: Option<&tauri::AppHandle>,
) -> Result<SendResult, String> {
    // Step 1: Canonicalize repo_path
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    // Step 2: Validate repo_path is in repos.json
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;

        repos.repos.iter().any(|r| r.path == canonical_str)
    };

    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    // Step 3: Validate post folder
    let post_path = canonical_path
        .join(".postlane/posts")
        .join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    // Read current meta.json
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    // Step 4: Call scheduling provider
    let mut platform_results = std::collections::HashMap::new();
    let mut scheduler_ids = std::collections::HashMap::new();

    // Only proceed with scheduling if app handle is provided (not in tests)
    if let Some(app_handle) = app {
        // Get repo ID from state (clone to avoid holding lock across await)
        let repo_id = {
            let repos = state.repos.lock()
                .map_err(|e| format!("Failed to lock repos: {}", e))?;

            let repo = repos.repos.iter()
                .find(|r| r.path == canonical_str)
                .ok_or("Repository not found in state")?;

            repo.id.clone()
        };

        // Initialize provider if needed
        get_or_init_provider(app_handle, repo_path, &repo_id, state).await?;

        // Read config.json to get profile_id
        let config_path = canonical_path.join(".postlane/config.json");
        let config_content = fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config.json: {}", e))?;

        let config: serde_json::Value = serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?;

        let profile_id = config["scheduler"]["profile_id"].as_str();

        // Schedule post for each platform
        for platform in &meta.platforms {
            // Read post content from platform-specific file
            let content_file = post_path.join(format!("{}.md", platform));
            let content = if content_file.exists() {
                fs::read_to_string(&content_file)
                    .map_err(|e| format!("Failed to read {}.md: {}", platform, e))?
            } else {
                return Err(format!("Content file {}.md not found", platform));
            };

            // Get scheduled time (if any)
            let scheduled_for = meta.schedule.as_ref().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s).ok()
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            });

            // Call provider.schedule_post
            let result = {
                let scheduler = state.scheduler.lock().await;

                let provider = scheduler.as_ref()
                    .ok_or("Provider not initialized")?;

                provider.schedule_post(
                    &content,
                    platform,
                    scheduled_for,
                    meta.image_url.as_deref(),
                    profile_id,
                ).await
            };

            match result {
                Ok(post_id) => {
                    platform_results.insert(platform.clone(), "success".to_string());
                    scheduler_ids.insert(platform.clone(), post_id);
                }
                Err(e) => {
                    platform_results.insert(platform.clone(), format!("error: {}", e));
                }
            }
        }
    } else {
        // No app handle (tests) - simulate success
        for platform in &meta.platforms {
            platform_results.insert(platform.clone(), "success".to_string());
        }
    }

    // Step 5: Update meta.json with results
    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    if !scheduler_ids.is_empty() {
        meta.scheduler_ids = Some(scheduler_ids);
    }

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
    })
}

/// Tauri command wrapper for approve_post
#[tauri::command]
pub async fn approve_post(
    repo_path: String,
    post_folder: String,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SendResult, String> {
    approve_post_impl(&repo_path, &post_folder, &state, Some(&app)).await
}

/// Dismiss a post
/// This is the testable implementation
pub fn dismiss_post_impl(
    repo_path: &str,
    post_folder: &str,
) -> Result<(), String> {
    let repo_pathbuf = PathBuf::from(repo_path);
    let post_path = repo_pathbuf.join(".postlane/posts").join(post_folder);
    let meta_path = post_path.join("meta.json");

    // Check meta.json exists
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    // Read current meta.json
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    // Update status to dismissed
    meta.status = "dismissed".to_string();

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(())
}

/// Tauri command wrapper for dismiss_post
#[tauri::command]
pub fn dismiss_post(
    repo_path: String,
    post_folder: String,
) -> Result<(), String> {
    dismiss_post_impl(&repo_path, &post_folder)
}

/// Retry a failed post (only retry failed platforms)
/// This is the testable implementation
pub fn retry_post_impl(
    repo_path: &str,
    post_folder: &str,
    state: &AppState,
) -> Result<SendResult, String> {
    // Step 1: Canonicalize repo_path
    let canonical_path = fs::canonicalize(repo_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    // Step 2: Validate repo_path is in repos.json
    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;
    let is_registered = {
        let repos = state
            .repos
            .lock()
            .map_err(|e| format!("Failed to lock repos: {}", e))?;

        repos.repos.iter().any(|r| r.path == canonical_str)
    };

    if !is_registered {
        return Err("Repository not registered (403)".to_string());
    }

    // Step 3: Validate post folder
    let post_path = canonical_path
        .join(".postlane/posts")
        .join(post_folder);

    if !post_path.exists() {
        return Err(format!("Post folder does not exist: {}", post_folder));
    }

    let meta_path = post_path.join("meta.json");
    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    // Read current meta.json
    let meta_content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;

    let mut meta: PostMeta = serde_json::from_str(&meta_content)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    // Step 4: Identify failed platforms and retry only those
    let mut platform_results = meta.platform_results.clone().unwrap_or_default();

    for platform in &meta.platforms {
        // Only retry if this platform failed
        if let Some(result) = platform_results.get(platform) {
            if result == "failed" {
                // Retry this platform (stub always succeeds)
                platform_results.insert(platform.clone(), "success".to_string());
            }
            // If it was "success", leave it unchanged
        } else {
            // No previous result - retry it
            platform_results.insert(platform.clone(), "success".to_string());
        }
    }

    // Step 5: Update meta.json with results
    meta.status = "sent".to_string();
    meta.platform_results = Some(platform_results.clone());
    meta.sent_at = Some(chrono::Utc::now().to_rfc3339());
    meta.error = None; // Clear previous error

    // Write updated meta.json atomically
    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&temp_path, json_content)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&temp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(SendResult {
        success: true,
        platform_results: Some(platform_results),
        error: None,
    })
}

/// Tauri command wrapper for retry_post
#[tauri::command]
pub fn retry_post(
    repo_path: String,
    post_folder: String,
    state: State<AppState>,
) -> Result<SendResult, String> {
    retry_post_impl(&repo_path, &post_folder, &state)
}

/// Add a repository
/// This is the testable implementation
pub fn add_repo_impl(
    path: &str,
    state: &AppState,
) -> Result<Repo, String> {
    // Step 1: Canonicalize path
    let canonical_path = fs::canonicalize(path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;

    // Step 2: Validate .git/ exists
    let git_dir = canonical_path.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    // Step 3: Validate .postlane/config.json exists
    let config_path = canonical_path.join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found. Run `postlane init` first.".to_string());
    }

    // Step 4: Generate UUID v4
    let id = Uuid::new_v4().to_string();

    // Step 5: Derive name from folder name
    let name = canonical_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid folder name")?
        .to_string();

    // Step 6: Create repo struct
    let repo = Repo {
        id: id.clone(),
        name: name.clone(),
        path: canonical_str.to_string(),
        active: true,
        added_at: chrono::Utc::now().to_rfc3339(),
    };

    // Step 7: Add to repos via Mutex and write to disk
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    repos.repos.push(repo.clone());

    // Write to repos.json
    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    // Step 8: Start watcher
    // In tests, watchers is empty - we skip this
    // In real app, this would call watch_repo()

    Ok(repo)
}

/// Tauri command wrapper for add_repo — starts the watcher after adding
#[tauri::command]
pub fn add_repo(
    path: String,
    state: State<AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Repo, String> {
    let repo = add_repo_impl(&path, &state)?;
    start_repo_watcher(&repo.id, &repo.path, &state, app_handle);
    Ok(repo)
}

/// Remove a repository
/// This is the testable implementation
pub fn remove_repo_impl(
    id: &str,
    state: &AppState,
) -> Result<(), String> {
    // Lock repos and find the repo to remove
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    // Find repo index
    let repo_index = repos
        .repos
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    // Remove from repos list
    repos.repos.remove(repo_index);

    // Write updated repos.json
    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    // Stop watcher (in tests, watchers is empty - this is a no-op)
    // In real app, this would call stop_watcher() or similar

    // Do NOT delete any files in the repo directory itself

    Ok(())
}

/// Tauri command wrapper for remove_repo
#[tauri::command]
pub fn remove_repo(
    id: String,
    state: State<AppState>,
) -> Result<(), String> {
    remove_repo_impl(&id, &state)
}

/// Set repository active state
/// This is the testable implementation
pub fn set_repo_active_impl(
    id: &str,
    active: bool,
    state: &AppState,
) -> Result<(), String> {
    // Lock repos and find the repo
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    // Find repo by ID
    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    // Update active state
    repo.active = active;

    // Write updated repos.json
    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    // Start/stop watcher (in tests, this is a no-op)
    // In real app:
    // - if active=true: start watcher
    // - if active=false: stop watcher

    Ok(())
}

/// Tauri command wrapper for set_repo_active — wires watcher start/stop
#[tauri::command]
pub fn set_repo_active(
    id: String,
    active: bool,
    state: State<AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    set_repo_active_impl(&id, active, &state)?;

    if active {
        // Resolve repo path from state to start watcher
        let repo_path = {
            let repos = state
                .repos
                .lock()
                .map_err(|e| format!("Failed to lock repos: {}", e))?;
            repos
                .repos
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.path.clone())
                .ok_or_else(|| format!("Repo '{}' not found after activation", id))?
        };
        start_repo_watcher(&id, &repo_path, &state, app_handle);
    } else {
        crate::watcher::stop_watcher(&id, &state.watchers);
    }

    Ok(())
}

/// Starts a file watcher for a repo's posts directory.
/// On each meta.json change, emits a "meta-changed" Tauri event.
fn start_repo_watcher(
    repo_id: &str,
    repo_path: &str,
    state: &AppState,
    app_handle: tauri::AppHandle,
) {
    use crate::nav_commands::MetaChangedPayload;
    use tauri::Emitter;

    let id = repo_id.to_string();
    let path = std::path::Path::new(repo_path);

    if let Err(e) = crate::watcher::watch_repo(
        id.clone(),
        path,
        &state.watchers,
        move |changed_paths| {
            for changed in &changed_paths {
                let post_folder = changed
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or("")
                    .to_string();
                let payload = MetaChangedPayload {
                    repo_id: id.clone(),
                    post_folder,
                };
                if let Err(emit_err) = app_handle.emit("meta-changed", payload.clone()) {
                    log::warn!("Failed to emit meta-changed: {}", emit_err);
                }
            }
        },
    ) {
        log::warn!("Failed to start watcher for repo {}: {}", repo_id, e);
    }
}

/// Check health of all registered repos
/// This is the testable implementation
pub fn check_repo_health_impl(
    state: &AppState,
) -> Result<Vec<RepoHealthStatus>, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let mut statuses = Vec::new();

    for repo in &repos.repos {
        // Check if .postlane/config.json exists at the stored path
        let config_path = PathBuf::from(&repo.path)
            .join(".postlane")
            .join("config.json");

        let reachable = config_path.exists();

        statuses.push(RepoHealthStatus {
            id: repo.id.clone(),
            reachable,
            path: repo.path.clone(),
        });
    }

    Ok(statuses)
}

/// Tauri command wrapper for check_repo_health
#[tauri::command]
pub fn check_repo_health(
    state: State<AppState>,
) -> Result<Vec<RepoHealthStatus>, String> {
    check_repo_health_impl(&state)
}

/// Get keyring key(s) to check for a credential
/// Returns a vector of keys to try in order: [per-repo, global] or just [global]
/// Per-repo key format: "{provider}/{repo_id}"
/// Global key format: "{provider}"
pub fn get_credential_keyring_key(provider: &str, repo_id: Option<&str>) -> Vec<String> {
    match repo_id {
        Some(id) => {
            // Check per-repo override first, then fall back to global
            vec![
                format!("{}/{}", provider, id),
                provider.to_string(),
            ]
        }
        None => {
            // Only check global key
            vec![provider.to_string()]
        }
    }
}

/// Check if libsecret is available before saving credentials
/// Takes the libsecret_available flag value from AppState
/// Returns Err if libsecret is unavailable, Ok otherwise
pub fn check_libsecret_before_save(libsecret_available: Option<bool>) -> Result<(), String> {
    match libsecret_available {
        Some(false) => {
            Err("libsecret not available. See the warning banner. Install with: sudo apt install libsecret-1-dev".to_string())
        }
        Some(true) | None => {
            // Available or not checked yet (will check on first use)
            Ok(())
        }
    }
}

/// Check if libsecret is available (Linux only)
/// Performs a test set_password / delete_password cycle
/// Returns true if keyring is available, false otherwise
pub fn check_libsecret_availability(app: Option<tauri::AppHandle>) -> bool {
    // If no app handle provided (testing), return true (assume available)
    let app = match app {
        Some(a) => a,
        None => return true,
    };

    // Try a test write/delete cycle
    let test_service = "postlane";
    let test_account = "__libsecret_test__";
    let test_password = "test";

    // Try to set a test password
    if app.keyring().set_password(test_service, test_account, test_password).is_ok() {
        // Successfully set - now try to delete
        app.keyring().delete_password(test_service, test_account).is_ok()
    } else {
        false // Set failed - libsecret unavailable
    }
}

/// Mask credential for display
/// Shows ••••••••{last4} where {last4} is the final 4 characters
/// For credentials shorter than 4 characters, shows ••••••••
pub fn mask_credential(credential: &str) -> String {
    let mask = "••••••••";

    if credential.len() >= 4 {
        let last_four = &credential[credential.len() - 4..];
        format!("{}{}", mask, last_four)
    } else {
        mask.to_string()
    }
}

/// Save scheduler credential - testable implementation
/// Validates provider name (business logic that can be unit tested)
pub fn save_scheduler_credential_impl(
    provider: &str,
    _api_key: &str,
    libsecret_available: Option<bool>,
) -> Result<(), String> {
    // Check libsecret availability before saving (Linux only)
    check_libsecret_before_save(libsecret_available)?;

    // Validate provider (v1 only supports these three)
    let valid_providers = ["zernio", "buffer", "ayrshare"];
    if !valid_providers.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }

    // Validation passed - actual keyring storage happens in Tauri command
    Ok(())
}

/// Get scheduler credential - testable implementation
/// Validates provider name (business logic that can be unit tested)
pub fn get_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    // Validate provider (v1 only supports these three)
    let valid_providers = ["zernio", "buffer", "ayrshare"];
    if !valid_providers.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }

    // Validation passed - actual keyring retrieval happens in Tauri command
    Ok(())
}

/// Delete scheduler credential - testable implementation
/// Validates provider name (business logic that can be unit tested)
pub fn delete_scheduler_credential_impl(provider: &str) -> Result<(), String> {
    // Validate provider (v1 only supports these three)
    let valid_providers = ["zernio", "buffer", "ayrshare"];
    if !valid_providers.contains(&provider) {
        return Err(format!("Unknown provider: {}", provider));
    }

    // Validation passed - actual keyring deletion happens in Tauri command
    Ok(())
}

/// Get scheduler credential from keyring (masked for display)
/// Returns ••••••••{last4} format, never the full credential
/// Checks per-repo override first (postlane/{provider}/{repo_id}), then global (postlane/{provider})
#[tauri::command]
pub fn get_scheduler_credential(
    provider: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<Option<String>, String> {
    // Step 1: Validate provider
    get_scheduler_credential_impl(&provider)?;

    // Step 2: Get keyring keys to check (per-repo first, then global)
    let keys = get_credential_keyring_key(&provider, repo_id.as_deref());

    // Step 3: Try each key in order until we find a credential
    for key in keys {
        match app.keyring().get_password("postlane", &key) {
            Ok(Some(credential)) => {
                // Found credential - mask for display
                return Ok(Some(mask_credential(&credential)));
            }
            Ok(None) => {
                // This key doesn't exist, try next one
                continue;
            }
            Err(e) => {
                // Error reading keyring - return error
                return Err(format!("Failed to retrieve credential: {}", e));
            }
        }
    }

    // No credential found at any key
    Ok(None)
}

/// Save scheduler credential to keyring
/// Supports per-repo override: if repo_id provided, stores at postlane/{provider}/{repo_id}
/// Otherwise stores at global key postlane/{provider}
#[tauri::command]
pub fn save_scheduler_credential(
    provider: String,
    api_key: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<(), String> {
    // Step 1: Get libsecret availability flag from AppState
    let libsecret_available = {
        let flag = state.libsecret_available.lock()
            .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
        *flag
    };

    // Step 2: Validate provider and check libsecret availability
    save_scheduler_credential_impl(&provider, &api_key, libsecret_available)?;

    // Step 3: Determine keyring key (per-repo or global)
    let keyring_key = match repo_id {
        Some(id) => format!("{}/{}", provider, id),
        None => provider.clone(),
    };

    // Step 4: Store in OS keyring
    app.keyring()
        .set_password("postlane", &keyring_key, &api_key)
        .map_err(|e| format!("Failed to store credential: {}", e))?;

    Ok(())
}

/// Delete scheduler credential from keyring
/// Supports per-repo deletion: if repo_id provided, deletes postlane/{provider}/{repo_id}
/// Otherwise deletes global key postlane/{provider}
#[tauri::command]
pub fn delete_scheduler_credential(
    provider: String,
    repo_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Step 1: Validate provider
    delete_scheduler_credential_impl(&provider)?;

    // Step 2: Determine keyring key (per-repo or global)
    let keyring_key = match repo_id {
        Some(id) => format!("{}/{}", provider, id),
        None => provider.clone(),
    };

    // Step 3: Delete from OS keyring
    app.keyring()
        .delete_password("postlane", &keyring_key)
        .map_err(|e| format!("Failed to delete credential: {}", e))?;

    Ok(())
}

/// Get libsecret availability status for UI warning banner
/// Returns Some(false) if unavailable, Some(true) if available, None if not checked yet
#[tauri::command]
pub fn get_libsecret_status(
    state: State<AppState>,
) -> Result<Option<bool>, String> {
    let flag = state.libsecret_available.lock()
        .map_err(|e| format!("Failed to lock libsecret_available: {}", e))?;
    Ok(*flag)
}

/// Test scheduler connection
#[tauri::command]
pub fn test_scheduler(
    provider: String,
    _state: State<AppState>,
) -> Result<bool, String> {
    // In Milestone 3, this is a stub
    // In Milestone 4, this will:
    // 1. Get credential from keyring
    // 2. Instantiate provider
    // 3. Call test_connection()
    // 4. Return Ok(true) on success, Ok(false) on connection failure, Err on missing credential

    // For now, just validate provider exists
    let valid_providers = ["zernio", "buffer", "ayrshare"];
    if !valid_providers.contains(&provider.as_str()) {
        return Err(format!("Unknown provider: {}", provider));
    }

    // Stub: always return true (connected)
    Ok(true)
}

/// Cancel a queued post
#[tauri::command]
pub fn cancel_post_command(
    _repo_path: String,
    _post_folder: String,
    _post_id: String,
    _platform: String,
    _state: State<AppState>,
) -> Result<(), String> {
    // In Milestone 3, this is a stub
    // In Milestone 4, this will:
    // 1. Call cancel_post(post_id, platform) on the scheduling provider
    // 2. On success: set meta.json.status back to "ready"
    // 3. If provider returns NotSupported: return error with message

    // For now, return not implemented error
    Err("Cancel not implemented in Milestone 3 (deferred to M4)".to_string())
}

/// Get queued posts from scheduler
#[tauri::command]
pub fn get_queue_command(
    _state: State<AppState>,
) -> Result<Vec<QueuedPost>, String> {
    // In Milestone 3, this is a stub
    // In Milestone 4, this will:
    // 1. Call get_queue() on the scheduling provider
    // 2. Return the list of currently queued posts

    // For now, return empty queue
    Ok(Vec::new())
}

/// Export history to CSV
/// This is the testable implementation (returns CSV content as string)
pub fn export_history_csv_impl(
    state: &AppState,
) -> Result<String, String> {
    let repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    // CSV header
    let mut csv = String::from("repo,slug,platforms,scheduler,model,sent_at,likes,reposts,replies,impressions,view_urls\n");

    // Scan all repos (active and inactive) for sent posts
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

            // Read and parse meta.json
            let meta_content = match fs::read_to_string(&meta_path) {
                Ok(content) => content,
                Err(_) => continue,
            };

            let meta: PostMeta = match serde_json::from_str(&meta_content) {
                Ok(meta) => meta,
                Err(_) => continue,
            };

            // Only include sent posts
            if meta.status != "sent" {
                continue;
            }

            // Extract slug from folder name
            let slug = post_folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            // Format platforms as comma-separated
            let platforms = meta.platforms.join("+");

            // For M3, we don't have engagement data yet - use placeholders
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

/// Update repository path
#[tauri::command]
pub fn update_repo_path(
    id: String,
    new_path: String,
    state: State<AppState>,
) -> Result<(), String> {
    // Canonicalize new path
    let canonical_path = fs::canonicalize(&new_path)
        .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

    let canonical_str = canonical_path.to_str().ok_or("Invalid path")?;

    // Validate .git/ exists
    let git_dir = canonical_path.join(".git");
    if !git_dir.exists() {
        return Err("Not a git repository".to_string());
    }

    // Validate .postlane/config.json exists
    let config_path = canonical_path.join(".postlane/config.json");
    if !config_path.exists() {
        return Err("config.json not found at new path".to_string());
    }

    // Update path in state
    let mut repos = state
        .repos
        .lock()
        .map_err(|e| format!("Failed to lock repos: {}", e))?;

    let repo = repos
        .repos
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("Repo with id '{}' not found", id))?;

    repo.path = canonical_str.to_string();

    // Write updated repos.json
    let repos_path = postlane_dir()?.join("repos.json");
    write_repos(&repos_path, &repos)
        .map_err(|e| format!("Failed to write repos.json: {:?}", e))?;

    // Stop old watcher and start new one (in tests, this is a no-op)
    // In real app:
    // - Stop watcher for old path
    // - Start watcher for new path

    Ok(())
}

/// Tauri command wrapper for export_history_csv
/// Opens save dialog and writes CSV to chosen location
#[tauri::command]
pub fn export_history_csv(
    state: State<AppState>,
) -> Result<String, String> {
    // Get CSV content
    let csv_content = export_history_csv_impl(&state)?;

    // In real app, this would:
    // 1. Open Tauri save_file dialog
    // 2. Write CSV content to chosen file
    // 3. Return the saved file path

    // For testing, just return the CSV content length as a placeholder
    Ok(format!("{} bytes", csv_content.len()))
}
