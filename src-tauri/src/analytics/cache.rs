// SPDX-License-Identifier: BUSL-1.1

use super::PostAnalytics;
use crate::init::postlane_dir;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnalyticsCacheEntry {
    pub data: PostAnalytics,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnalyticsCache {
    pub version: u32,
    pub entries: HashMap<String, AnalyticsCacheEntry>,
}

impl Default for AnalyticsCache {
    fn default() -> Self {
        Self { version: 1, entries: HashMap::new() }
    }
}

fn cache_path() -> Result<PathBuf, String> {
    Ok(postlane_dir()?.join("analytics_cache.json"))
}

/// Cache key: "{repo_id}:{post_folder}"
pub fn cache_key(repo_id: &str, post_folder: &str) -> String {
    format!("{}:{}", repo_id, post_folder)
}

/// Returns true if the entry is fresh and the clock has not been manipulated.
/// Stale if past TTL; suspicious if expires_at is more than 2 hours in the future.
pub fn is_entry_valid(entry: &AnalyticsCacheEntry) -> bool {
    let now = Utc::now();
    if entry.expires_at < now {
        return false;
    }
    if entry.expires_at > now + Duration::hours(2) {
        log::warn!("analytics_cache entry expires_at suspiciously far in the future; treating as stale");
        return false;
    }
    true
}

/// Creates a new cache entry. Zero-result entries use a 5-minute TTL so that
/// users who just installed the snippet see real data within minutes rather than an hour.
pub fn new_entry(data: PostAnalytics) -> AnalyticsCacheEntry {
    let ttl = if data.unique_sessions == 0 { Duration::minutes(5) } else { Duration::hours(1) };
    AnalyticsCacheEntry { data, expires_at: Utc::now() + ttl }
}

/// Reads analytics_cache.json; silently returns empty cache on any error.
pub fn read_analytics_cache() -> AnalyticsCache {
    let path = match cache_path() {
        Ok(p) => p,
        Err(e) => { log::warn!("analytics cache path error: {}", e); return AnalyticsCache::default(); }
    };
    if !path.exists() { return AnalyticsCache::default(); }
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<AnalyticsCache>(&content) {
            Ok(c) if c.version == 1 => c,
            Ok(c) => { log::warn!("analytics_cache.json version {} != 1; discarding", c.version); AnalyticsCache::default() }
            Err(e) => { log::warn!("analytics_cache.json parse error: {}", e); AnalyticsCache::default() }
        },
        Err(e) => { log::warn!("analytics_cache.json read error: {}", e); AnalyticsCache::default() }
    }
}

/// Writes analytics_cache.json atomically.
pub fn write_analytics_cache(cache: &AnalyticsCache) -> Result<(), String> {
    let path = cache_path()?;
    let json = serde_json::to_string_pretty(cache)
        .map_err(|e| format!("Failed to serialize analytics cache: {}", e))?;
    crate::init::atomic_write(&path, json.as_bytes())
        .map_err(|e| format!("Failed to write analytics cache: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    fn lock() -> &'static Mutex<()> { TEST_MUTEX.get_or_init(|| Mutex::new(())) }

    #[test]
    fn test_analytics_cache_returns_hit_within_ttl() {
        let entry = new_entry(PostAnalytics { configured: true, sessions: 5, unique_sessions: 3, top_referrer: None });
        assert!(is_entry_valid(&entry));
        assert_eq!(entry.data.sessions, 5);
    }

    #[test]
    fn test_analytics_cache_clock_guard() {
        let entry = AnalyticsCacheEntry {
            data: PostAnalytics::default(),
            expires_at: Utc::now() + Duration::hours(3),
        };
        assert!(!is_entry_valid(&entry), "expires_at 3h in future should be stale");
    }

    #[test]
    fn test_analytics_cache_version_mismatch() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let bad = r#"{"version":2,"entries":{}}"#;
        std::fs::write(&path, bad).unwrap();
        let cache = read_analytics_cache();
        assert_eq!(cache.version, 1);
        assert!(cache.entries.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_analytics_cache_zero_result_has_short_ttl() {
        let entry = new_entry(PostAnalytics::default()); // unique_sessions == 0
        let ttl = entry.expires_at - Utc::now();
        assert!(ttl < Duration::minutes(10), "Zero-result entry TTL should be < 10 minutes");
        assert!(ttl > Duration::minutes(4), "Zero-result entry TTL should be > 4 minutes");
    }

    #[test]
    fn test_analytics_cache_nonzero_result_has_long_ttl() {
        let entry = new_entry(PostAnalytics { configured: true, sessions: 5, unique_sessions: 3, top_referrer: None });
        let ttl = entry.expires_at - Utc::now();
        assert!(ttl > Duration::minutes(50), "Non-zero entry should keep 1-hour TTL");
    }

    #[test]
    fn test_analytics_cache_round_trip() {
        let _g = lock().lock().unwrap();
        crate::init::init_postlane_dir().expect("init");
        let path = cache_path().expect("path");
        let _ = std::fs::remove_file(&path);
        let mut cache = AnalyticsCache::default();
        cache.entries.insert(
            cache_key("r1", "post-1"),
            new_entry(PostAnalytics { configured: true, sessions: 10, unique_sessions: 7, top_referrer: Some("t.co".into()) }),
        );
        write_analytics_cache(&cache).expect("write");
        let loaded = read_analytics_cache();
        let e = loaded.entries.get("r1:post-1").expect("entry");
        assert_eq!(e.data.sessions, 10);
        assert_eq!(e.data.top_referrer.as_deref(), Some("t.co"));
        let _ = std::fs::remove_file(&path);
    }
}
