// SPDX-License-Identifier: BUSL-1.1

//! Writes and removes scheduler provider entries in `config.local.json`.
//!
//! v1.4 adds workspace-level `config.local.json` at `{workspace}/config.local.json`.
//! Resolution order (22.1.5): workspace root first, then `{repo}/.postlane/config.local.json`.

use crate::init::{atomic_write, read_json_file};
use std::path::{Path, PathBuf};

/// Returns the effective `config.local.json` path for a given child repo (22.1.5).
///
/// Resolution order:
///   1. `{workspace_path}/config.local.json` — if it exists
///   2. `{repo_path}/.postlane/config.local.json` — legacy per-repo fallback
///
/// For legacy per-repo installs with no workspace, pass the same path for both
/// arguments so the per-repo path is always returned.
pub fn resolve_local_config_path(repo_path: &Path, workspace_path: &Path) -> PathBuf {
    let workspace_local = workspace_path.join("config.local.json");
    if workspace_local.exists() {
        workspace_local
    } else {
        repo_path.join(".postlane").join("config.local.json")
    }
}

/// Writes `content` to `{workspace_path}/config.local.json` atomically with
/// `0600` permissions on Unix (22.1.6a). Never uses `std::fs::write` directly
/// to ensure permissions are set correctly before any data is written.
pub fn write_workspace_local_config(workspace_path: &Path, content: &str) -> Result<(), String> {
    let target = workspace_path.join("config.local.json");
    write_local_config_0600(&target, content)
}

/// Appends `config.local.json` to `{workspace_path}/.gitignore` (22.1.6).
/// Creates `.gitignore` if absent. Idempotent — does not duplicate the entry.
pub fn append_config_local_to_gitignore(workspace_path: &Path) -> Result<(), String> {
    let gitignore_path = workspace_path.join(".gitignore");
    let entry = "config.local.json";

    let existing = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)
            .map_err(|e| format!("failed to read .gitignore: {}", e))?
    } else {
        String::new()
    };

    if existing.lines().any(|line| line.trim() == entry) {
        return Ok(()); // already present — idempotent
    }

    let mut new_content = existing;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(entry);
    new_content.push('\n');

    atomic_write(&gitignore_path, new_content.as_bytes())
        .map_err(|e| format!("failed to write .gitignore: {}", e))
}

/// Writes `content` to `path` atomically with 0600 permissions on Unix.
/// On non-Unix platforms, falls back to a standard atomic write.
fn write_local_config_0600(path: &Path, content: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create directory: {}", e))?;
        }
        // Write to a unique .tmp file with 0600 permissions, then rename atomically.
        let tmp = path.with_extension("local.tmp");
        {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|e| format!("failed to create temp file: {}", e))?;
            use std::io::Write;
            file.write_all(content.as_bytes())
                .map_err(|e| format!("failed to write temp file: {}", e))?;
        }
        std::fs::rename(&tmp, path)
            .map_err(|e| format!("failed to rename temp file: {}", e))
    }
    #[cfg(not(unix))]
    {
        atomic_write(path, content.as_bytes())
            .map_err(|e| format!("failed to write config.local.json: {}", e))
    }
}

/// Adds `provider` to the scheduler fallback list in `.postlane/config.local.json`.
/// Creates the file if absent. When only one provider exists, writes `scheduler.provider`;
/// when two or more are configured, upgrades to `scheduler.fallback_order` so the
/// credential router can try each in order.
pub fn write_scheduler_provider_to_local_config(repo_path: &Path, provider: &str) -> Result<(), String> {
    // Workspace configs use flat layout ({root}/config.local.json); legacy use .postlane/.
    let postlane_dir = repo_path.join(".postlane");
    let local_path = if postlane_dir.is_dir() {
        postlane_dir.join("config.local.json")
    } else {
        repo_path.join("config.local.json")
    };

    let mut local: serde_json::Value = if local_path.exists() {
        read_json_file(&local_path)?
    } else {
        serde_json::json!({})
    };

    if !local["scheduler"].is_object() {
        local["scheduler"] = serde_json::json!({});
    }

    let mut order: Vec<String> = if let Some(arr) = local["scheduler"]["fallback_order"].as_array() {
        arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
    } else if let Some(p) = local["scheduler"]["provider"].as_str() {
        if p.is_empty() { vec![] } else { vec![p.to_string()] }
    } else {
        vec![]
    };

    if !order.contains(&provider.to_string()) {
        order.push(provider.to_string());
    }

    if order.len() > 1 {
        local["scheduler"]["fallback_order"] = serde_json::json!(&order);
        local["scheduler"].as_object_mut().map(|s| s.remove("provider"));
    } else {
        local["scheduler"]["provider"] =
            serde_json::json!(order.first().map(String::as_str).unwrap_or(""));
    }

    let json = serde_json::to_string_pretty(&local)
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    write_local_config_0600(&local_path, &json)
}

/// Removes `provider` from the scheduler fallback list in `.postlane/config.local.json`.
/// When the removed provider was the only one, sets `scheduler.provider` to `""` so that
/// `read_fallback_order_from_value` treats the repo as unconfigured.
/// Returns `Ok` without error if the file does not exist or the provider is not present.
pub fn remove_scheduler_provider_from_local_config(
    repo_path: &Path,
    provider: &str,
) -> Result<(), String> {
    let local_path = repo_path.join(".postlane").join("config.local.json");
    if !local_path.exists() {
        return Ok(());
    }

    let mut local: serde_json::Value = read_json_file(&local_path)?;

    if !local["scheduler"].is_object() {
        return Ok(());
    }

    let mut order: Vec<String> =
        if let Some(arr) = local["scheduler"]["fallback_order"].as_array() {
            arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        } else if let Some(p) = local["scheduler"]["provider"].as_str() {
            if p.is_empty() { vec![] } else { vec![p.to_string()] }
        } else {
            vec![]
        };

    order.retain(|p| p != provider);

    if let Some(sched) = local["scheduler"].as_object_mut() {
        sched.remove("fallback_order");
        sched.remove("provider");
    }

    if order.len() > 1 {
        local["scheduler"]["fallback_order"] = serde_json::json!(&order);
    } else {
        local["scheduler"]["provider"] =
            serde_json::json!(order.first().map(String::as_str).unwrap_or(""));
    }

    let json = serde_json::to_string_pretty(&local)
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    write_local_config_0600(&local_path, &json)
}

#[cfg(test)]
#[path = "config_local_write_tests.rs"]
mod tests;
