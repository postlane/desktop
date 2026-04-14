// SPDX-License-Identifier: BUSL-1.1

use crate::init::postlane_dir;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EngagementEntry {
    pub expires_at: DateTime<Utc>,
    pub likes: u64,
    pub reposts: u64,
    pub replies: u64,
    pub impressions: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EngagementCache {
    pub version: u32,
    pub entries: HashMap<String, EngagementEntry>,
}

impl Default for EngagementCache {
    fn default() -> Self {
        Self {
            version: 1,
            entries: HashMap::new(),
        }
    }
}

fn cache_path() -> PathBuf {
    postlane_dir().join("engagement_cache.json")
}

/// Generates cache key: "{repo_id}:{slug}:{platform}"
pub fn cache_key(repo_id: &str, slug: &str, platform: &str) -> String {
    format!("{}:{}:{}", repo_id, slug, platform)
}

/// Reads engagement cache with silent fallback to empty cache
pub fn read_engagement_cache() -> EngagementCache {
    let path = cache_path();

    if !path.exists() {
        return EngagementCache::default();
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<EngagementCache>(&content) {
            Ok(cache) => {
                if cache.version != 1 {
                    log::warn!(
                        "engagement_cache.json version mismatch: found {}, expected 1. Returning empty cache.",
                        cache.version
                    );
                    return EngagementCache::default();
                }
                cache
            }
            Err(e) => {
                log::warn!("Failed to parse engagement_cache.json: {}. Returning empty cache.", e);
                EngagementCache::default()
            }
        },
        Err(e) => {
            log::warn!("Failed to read engagement_cache.json: {}. Returning empty cache.", e);
            EngagementCache::default()
        }
    }
}

/// Writes engagement cache atomically
pub fn write_engagement_cache(cache: &EngagementCache) -> std::io::Result<()> {
    let path = cache_path();
    let json = serde_json::to_string_pretty(&cache)?;
    crate::init::atomic_write(&path, json.as_bytes())
}

/// Checks if a cache entry is valid based on TTL and clock-manipulation guard
/// Returns true if entry is valid and should be used
pub fn is_entry_valid(entry: &EngagementEntry) -> bool {
    let now = Utc::now();

    // If expires_at is in the past, entry is stale
    if entry.expires_at < now {
        return false;
    }

    // Clock-manipulation guard: if expires_at is more than 2 hours in the future, suspicious
    let two_hours_from_now = now + Duration::hours(2);
    if entry.expires_at > two_hours_from_now {
        log::warn!(
            "Cache entry expires_at is suspiciously far in the future: {:?}. Treating as invalid.",
            entry.expires_at
        );
        return false;
    }

    true
}

/// Creates a new engagement entry with 1-hour TTL
pub fn new_entry(likes: u64, reposts: u64, replies: u64, impressions: Option<u64>) -> EngagementEntry {
    EngagementEntry {
        expires_at: Utc::now() + Duration::hours(1),
        likes,
        reposts,
        replies,
        impressions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    // Global mutex to serialize tests that use the shared cache file
    static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn get_test_mutex() -> &'static Mutex<()> {
        TEST_MUTEX.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_cache_key_format() {
        let key = cache_key("repo-123", "my-post-slug", "x");
        assert_eq!(key, "repo-123:my-post-slug:x");
    }

    #[test]
    fn test_read_engagement_cache_missing_file() {
        // Clean up if exists
        let path = cache_path();
        let _ = fs::remove_file(&path);

        let cache = read_engagement_cache();
        assert_eq!(cache.version, 1);
        assert_eq!(cache.entries.len(), 0);
    }

    #[test]
    fn test_engagement_cache_round_trip() {
        // Acquire lock to prevent race conditions with other tests
        let _lock = get_test_mutex().lock().unwrap();

        // Ensure ~/.postlane exists
        crate::init::init_postlane_dir().expect("Failed to init postlane dir");

        let mut cache = EngagementCache::default();

        let entry = new_entry(100, 50, 25, Some(1000));
        cache.entries.insert(
            cache_key("repo1", "post1", "x"),
            entry,
        );

        // Clean up any existing cache first
        let path = cache_path();
        let _ = fs::remove_file(&path);

        write_engagement_cache(&cache).expect("Failed to write cache");

        let loaded = read_engagement_cache();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.entries.len(), 1, "Should have 1 entry");

        let retrieved = loaded.entries.get("repo1:post1:x").unwrap();
        assert_eq!(retrieved.likes, 100);
        assert_eq!(retrieved.reposts, 50);
        assert_eq!(retrieved.replies, 25);
        assert_eq!(retrieved.impressions, Some(1000));

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_entry_valid_within_ttl() {
        let entry = new_entry(10, 5, 2, None);
        assert!(is_entry_valid(&entry), "Fresh entry should be valid");
    }

    #[test]
    fn test_entry_expired_in_past() {
        let entry = EngagementEntry {
            expires_at: Utc::now() - Duration::hours(1),
            likes: 10,
            reposts: 5,
            replies: 2,
            impressions: None,
        };
        assert!(!is_entry_valid(&entry), "Expired entry should be invalid");
    }

    #[test]
    fn test_entry_clock_manipulation_guard() {
        let entry = EngagementEntry {
            expires_at: Utc::now() + Duration::hours(3), // More than 2 hours in future
            likes: 10,
            reposts: 5,
            replies: 2,
            impressions: None,
        };
        assert!(!is_entry_valid(&entry), "Suspiciously far-future entry should be invalid");
    }

    #[test]
    fn test_cache_version_mismatch() {
        // Acquire lock to prevent race conditions with other tests
        let _lock = get_test_mutex().lock().unwrap();

        // Clean up before test to prevent race conditions
        let path = cache_path();
        let _ = fs::remove_file(&path);

        let mut cache = EngagementCache::default();
        cache.version = 999;

        let json = serde_json::to_string(&cache).expect("Failed to serialize");
        fs::write(&path, json).expect("Failed to write");

        let loaded = read_engagement_cache();
        assert_eq!(loaded.version, 1, "Should return default with correct version");
        assert_eq!(loaded.entries.len(), 0, "Should be empty");

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_entry_within_two_hour_window() {
        // Entry that expires in 90 minutes (within 2-hour window)
        let entry = EngagementEntry {
            expires_at: Utc::now() + Duration::minutes(90),
            likes: 10,
            reposts: 5,
            replies: 2,
            impressions: None,
        };
        assert!(is_entry_valid(&entry), "Entry within 2-hour window should be valid");
    }

    #[test]
    fn test_read_engagement_cache_malformed_returns_default() {
        let _lock = get_test_mutex().lock().unwrap();

        crate::init::init_postlane_dir().expect("Failed to init postlane dir");
        let path = cache_path();
        let _ = fs::remove_file(&path);

        // Write malformed JSON
        fs::write(&path, "{ this is not valid json }").expect("Failed to write");

        // Should return default on parse error
        let loaded = read_engagement_cache();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.entries.len(), 0);

        let _ = fs::remove_file(&path);
    }
}
