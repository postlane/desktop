// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[cfg(test)]
static TEST_SITES_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<PathBuf>>> =
    std::sync::OnceLock::new();

/// Maps repo_id → site_token; persisted to analytics_sites.json
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnalyticsSites {
    pub version: u32,
    pub sites: HashMap<String, String>,
}

impl Default for AnalyticsSites {
    fn default() -> Self { Self { version: 1, sites: HashMap::new() } }
}

fn sites_path() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let maybe = TEST_SITES_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(path) = maybe {
            return Ok(path);
        }
    }
    Ok(postlane_dir()?.join("analytics_sites.json"))
}

pub fn read_analytics_sites() -> AnalyticsSites {
    let path = match sites_path() {
        Ok(p) => p,
        Err(e) => { log::warn!("analytics sites path error: {}", e); return AnalyticsSites::default(); }
    };
    if !path.exists() { return AnalyticsSites::default(); }
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<AnalyticsSites>(&content) {
            Ok(s) if s.version == 1 => s,
            Ok(_) | Err(_) => AnalyticsSites::default(),
        },
        Err(_) => AnalyticsSites::default(),
    }
}

fn write_analytics_sites(sites: &AnalyticsSites) -> Result<(), String> {
    let path = sites_path()?;
    let json = serde_json::to_string_pretty(sites)
        .map_err(|e| format!("Failed to serialize analytics sites: {}", e))?;
    crate::init::atomic_write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write analytics sites: {}", e))
}

/// Returns the cached site_token for this repo, if one has been fetched.
pub fn get_cached_site_token(repo_id: &str) -> Option<String> {
    read_analytics_sites().sites.get(repo_id).cloned()
}

/// Persists a site_token for a repo.
pub fn save_site_token(repo_id: &str, token: &str) -> Result<(), String> {
    let mut sites = read_analytics_sites();
    sites.sites.insert(repo_id.to_string(), token.to_string());
    write_analytics_sites(&sites)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    fn lock() -> &'static Mutex<()> { TEST_LOCK.get_or_init(|| Mutex::new(())) }

    struct SitesGuard {
        _dir: tempfile::TempDir,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl SitesGuard {
        fn acquire() -> Self {
            let _lock = lock().lock().unwrap_or_else(|p| p.into_inner());
            let _dir = tempfile::TempDir::new().expect("temp dir");
            let path = _dir.path().join("analytics_sites.json");
            *super::TEST_SITES_OVERRIDE
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = Some(path);
            SitesGuard { _dir, _lock }
        }
    }

    impl Drop for SitesGuard {
        fn drop(&mut self) {
            *super::TEST_SITES_OVERRIDE
                .get_or_init(|| std::sync::Mutex::new(None))
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = None;
        }
    }

    #[test]
    fn test_save_and_get_site_token() {
        let _guard = SitesGuard::acquire();
        save_site_token("repo-1", "tok-abc").expect("save");
        assert_eq!(get_cached_site_token("repo-1").as_deref(), Some("tok-abc"));
        assert!(get_cached_site_token("repo-2").is_none());
    }

    #[test]
    fn test_get_site_token_missing_returns_none() {
        let _guard = SitesGuard::acquire();
        assert!(get_cached_site_token("nobody").is_none());
    }

    #[test]
    fn test_read_returns_default_on_bad_json() {
        let _guard = SitesGuard::acquire();
        let path = super::sites_path().expect("path");
        std::fs::write(&path, b"not valid json").expect("write bad json");
        // read_analytics_sites must return a default (empty) when JSON is unparseable
        assert!(get_cached_site_token("any-repo").is_none());
    }

    #[test]
    fn test_read_returns_default_on_wrong_version() {
        let _guard = SitesGuard::acquire();
        let path = super::sites_path().expect("path");
        std::fs::write(&path, br#"{"version":99,"sites":{"repo-x":"tok-x"}}"#).expect("write");
        // version != 1 must be treated as a corrupt/unknown file — return empty default
        assert!(
            get_cached_site_token("repo-x").is_none(),
            "token from unsupported version must not be returned"
        );
    }
}
