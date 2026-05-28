// SPDX-License-Identifier: BUSL-1.1

//! Writes the initial `.postlane/config.json` and `.postlane/config.local.json`
//! files when a repo is first connected or discovered. Both writes are atomic.

use crate::init::atomic_write;
use crate::project_validation::reject_if_symlink;
use std::path::Path;

pub(crate) const BASE_URL: &str = "https://postlane.dev";
pub(crate) const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-6";

pub(crate) fn build_initial_config_json(project_id: &str) -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "project_id": project_id,
        "base_url": BASE_URL,
        "llm": { "provider": "anthropic", "model": DEFAULT_LLM_MODEL }
    })
}

pub(crate) fn build_initial_config_local_json() -> serde_json::Value {
    serde_json::json!({ "scheduler": { "provider": "" } })
}

/// Writes `.postlane/config.json` and `.postlane/config.local.json` into `repo_dir`.
/// Both files are written atomically (tmp -> rename). Rejects symlinks.
/// Creates `.postlane/` automatically (via atomic_write's parent-dir creation).
pub(crate) fn write_initial_config_files(repo_dir: &Path, project_id: &str) -> Result<(), String> {
    let config_path = repo_dir.join(".postlane").join("config.json");
    reject_if_symlink(&config_path)?;
    let config_bytes = serde_json::to_vec_pretty(&build_initial_config_json(project_id))
        .map_err(|e| format!("Failed to serialise config.json: {}", e))?;
    atomic_write(&config_path, &config_bytes)
        .map_err(|e| format!("Failed to write config.json: {}", e))?;

    let local_path = repo_dir.join(".postlane").join("config.local.json");
    reject_if_symlink(&local_path)?;
    let local_bytes = serde_json::to_vec_pretty(&build_initial_config_local_json())
        .map_err(|e| format!("Failed to serialise config.local.json: {}", e))?;
    atomic_write(&local_path, &local_bytes)
        .map_err(|e| format!("Failed to write config.local.json: {}", e))?;

    Ok(())
}

/// Computes the hex-encoded SHA-256 digest of `input`.
pub fn sha256_hex(input: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── sha256_hex ────────────────────────────────────────────────────────────

    #[test]
    fn test_sha256_hex_produces_64_char_hex() {
        let h = sha256_hex("test-input");
        assert_eq!(h.len(), 64, "expected 64-char SHA-256 hex, got {} chars: {}", h.len(), h);
    }

    #[test]
    fn test_sha256_hex_is_deterministic() {
        assert_eq!(sha256_hex("/users/hugo/repos/desktop"), sha256_hex("/users/hugo/repos/desktop"));
    }

    #[test]
    fn test_sha256_hex_different_inputs_differ() {
        assert_ne!(sha256_hex("/path/one"), sha256_hex("/path/two"));
    }

    // ── write_initial_config_files ────────────────────────────────────────────

    #[test]
    fn test_write_initial_config_files_creates_config_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_initial_config_files(dir.path(), "proj-abc").expect("should succeed");
        let config_path = dir.path().join(".postlane/config.json");
        assert!(config_path.exists());
        let content = std::fs::read_to_string(&config_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert_eq!(parsed["project_id"].as_str(), Some("proj-abc"));
        assert_eq!(parsed["version"].as_u64(), Some(1));
        assert_eq!(parsed["base_url"].as_str(), Some(BASE_URL));
    }

    #[test]
    fn test_write_initial_config_files_creates_config_local_json() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        write_initial_config_files(dir.path(), "proj-xyz").expect("should succeed");
        let local_path = dir.path().join(".postlane/config.local.json");
        assert!(local_path.exists());
        let content = std::fs::read_to_string(&local_path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse");
        assert!(parsed["scheduler"].is_object());
    }
}
