// SPDX-License-Identifier: BUSL-1.1

use crate::init::{atomic_write, postlane_dir};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Usage record for a single scheduler provider in the current billing month.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UsageRecord {
    pub provider: String,
    pub count: u32,
    pub month: u32,
    pub year: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct UsageStore {
    records: HashMap<String, UsageRecord>,
}

/// Returns the known monthly free-tier post limit for providers tracked by count.
/// Zernio and Buffer have no published limit and are tracked by 429 responses only.
/// Ayrshare has no free tier; Substack Notes has no scheduler queue.
pub fn get_known_limit(provider: &str) -> Option<u32> {
    match provider {
        "publer" => Some(10),
        "outstand" => Some(1_000),
        "webhook" => Some(100), // Zapier 100 tasks/month (conservative lower bound)
        _ => None,
    }
}

/// Path to ~/.postlane/scheduler_usage.json.
pub(crate) fn default_store_path() -> Result<PathBuf, String> {
    Ok(postlane_dir()?.join("scheduler_usage.json"))
}

fn read_store(path: &Path) -> UsageStore {
    if !path.exists() {
        return UsageStore::default();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => UsageStore::default(),
    }
}

fn write_store(path: &Path, store: &UsageStore) -> Result<(), String> {
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("Failed to serialize usage store: {}", e))?;
    atomic_write(path, json.as_bytes())
        .map_err(|e| format!("Failed to write usage store: {}", e))
}

/// Increments the post count for `provider`, resetting to 0 if the month has rolled over.
/// Uses `month`/`year` instead of `Utc::now()` so tests can inject a fixed date.
pub(crate) fn record_post_at(
    provider: &str,
    path: &Path,
    month: u32,
    year: u32,
) -> Result<(), String> {
    let mut store = read_store(path);
    let record = store
        .records
        .entry(provider.to_string())
        .or_insert_with(|| UsageRecord {
            provider: provider.to_string(),
            count: 0,
            month,
            year,
        });

    if record.month != month || record.year != year {
        record.count = 0;
        record.month = month;
        record.year = year;
    }

    record.count = record.count.saturating_add(1);
    write_store(path, &store)
}

/// Returns the usage record for `provider` for the given month/year.
/// Returns a zeroed record if stored data is from a different month, or if no data exists.
pub(crate) fn get_usage_at(
    provider: &str,
    path: &Path,
    month: u32,
    year: u32,
) -> Result<UsageRecord, String> {
    let store = read_store(path);
    match store.records.get(provider) {
        Some(r) if r.month == month && r.year == year => Ok(r.clone()),
        _ => Ok(UsageRecord {
            provider: provider.to_string(),
            count: 0,
            month,
            year,
        }),
    }
}

/// Returns true if `provider` has consumed >= 80% of its known free-tier limit.
pub(crate) fn is_near_limit_at(provider: &str, path: &Path, month: u32, year: u32) -> bool {
    let limit = match get_known_limit(provider) {
        Some(l) => l,
        None => return false,
    };
    match get_usage_at(provider, path, month, year) {
        Ok(r) => r.count >= limit * 4 / 5,
        Err(_) => false,
    }
}

/// Returns true if `provider` has consumed 100% of its known free-tier limit.
pub(crate) fn is_at_limit_at(provider: &str, path: &Path, month: u32, year: u32) -> bool {
    let limit = match get_known_limit(provider) {
        Some(l) => l,
        None => return false,
    };
    match get_usage_at(provider, path, month, year) {
        Ok(r) => r.count >= limit,
        Err(_) => false,
    }
}

/// Increments the post count for `provider` for the current calendar month.
pub fn record_post(provider: &str) -> Result<(), String> {
    let path = default_store_path()?;
    let now = Utc::now();
    record_post_at(provider, &path, now.month(), now.year() as u32)
}

/// Returns the current usage record for `provider`.
pub fn get_usage(provider: &str) -> Result<UsageRecord, String> {
    let path = default_store_path()?;
    let now = Utc::now();
    get_usage_at(provider, &path, now.month(), now.year() as u32)
}

/// Returns true if `provider` is >= 80% of its known free-tier limit this month.
pub fn is_near_limit(provider: &str) -> bool {
    let path = match default_store_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let now = Utc::now();
    is_near_limit_at(provider, &path, now.month(), now.year() as u32)
}

/// Returns true if `provider` has hit 100% of its known free-tier limit this month.
pub fn is_at_limit(provider: &str) -> bool {
    let path = match default_store_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let now = Utc::now();
    is_at_limit_at(provider, &path, now.month(), now.year() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("postlane_usage_{}", name));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("scheduler_usage.json")
    }

    fn cleanup(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    /// §13.4.1 — count resets when calendar month advances
    #[test]
    fn test_usage_tracker_resets_on_month_boundary() {
        let path = temp_path("reset_month");

        for _ in 0..10 {
            record_post_at("publer", &path, 1, 2026).expect("record jan");
        }
        let jan = get_usage_at("publer", &path, 1, 2026).expect("jan");
        assert_eq!(jan.count, 10, "January must have 10 posts");

        // Reading February returns 0 (stale month)
        let feb_read = get_usage_at("publer", &path, 2, 2026).expect("feb read");
        assert_eq!(feb_read.count, 0, "February read must be 0 before any posts");

        // Recording in February resets and increments to 1 (not 11)
        record_post_at("publer", &path, 2, 2026).expect("feb post");
        let feb = get_usage_at("publer", &path, 2, 2026).expect("feb after");
        assert_eq!(feb.count, 1, "First February post must be 1, not 11");

        cleanup(&path);
    }

    #[test]
    fn test_record_post_increments_count() {
        let path = temp_path("increment");
        record_post_at("publer", &path, 4, 2026).expect("first");
        record_post_at("publer", &path, 4, 2026).expect("second");
        let u = get_usage_at("publer", &path, 4, 2026).expect("get");
        assert_eq!(u.count, 2);
        cleanup(&path);
    }

    #[test]
    fn test_get_usage_missing_provider_returns_zero() {
        let path = temp_path("missing");
        let u = get_usage_at("publer", &path, 4, 2026).expect("get");
        assert_eq!(u.count, 0);
        assert_eq!(u.provider, "publer");
        cleanup(&path);
    }

    #[test]
    fn test_known_limits() {
        assert_eq!(get_known_limit("publer"), Some(10));
        assert_eq!(get_known_limit("outstand"), Some(1_000));
        assert_eq!(get_known_limit("webhook"), Some(100));
        assert_eq!(get_known_limit("zernio"), None);
        assert_eq!(get_known_limit("buffer"), None);
        assert_eq!(get_known_limit("ayrshare"), None);
        assert_eq!(get_known_limit("substack_notes"), None);
    }

    #[test]
    fn test_near_limit_threshold_is_80_percent() {
        let path = temp_path("near_limit");
        // Publer limit = 10; 80% = 8
        for _ in 0..7 {
            record_post_at("publer", &path, 4, 2026).expect("record");
        }
        assert!(!is_near_limit_at("publer", &path, 4, 2026), "7/10 must not be near limit");

        record_post_at("publer", &path, 4, 2026).expect("8th");
        assert!(is_near_limit_at("publer", &path, 4, 2026), "8/10 must be near limit");

        cleanup(&path);
    }

    #[test]
    fn test_at_limit_threshold_is_100_percent() {
        let path = temp_path("at_limit");
        for _ in 0..9 {
            record_post_at("publer", &path, 4, 2026).expect("record");
        }
        assert!(!is_at_limit_at("publer", &path, 4, 2026), "9/10 must not be at limit");

        record_post_at("publer", &path, 4, 2026).expect("10th");
        assert!(is_at_limit_at("publer", &path, 4, 2026), "10/10 must be at limit");

        cleanup(&path);
    }

    #[test]
    fn test_unlimited_providers_never_at_limit() {
        let path = temp_path("unlimited");
        for _ in 0..100 {
            record_post_at("zernio", &path, 4, 2026).expect("record");
        }
        assert!(!is_near_limit_at("zernio", &path, 4, 2026));
        assert!(!is_at_limit_at("zernio", &path, 4, 2026));
        assert!(!is_near_limit_at("buffer", &path, 4, 2026));
        cleanup(&path);
    }

    #[test]
    fn test_multiple_providers_tracked_independently() {
        let path = temp_path("multi");
        for _ in 0..5 {
            record_post_at("publer", &path, 4, 2026).expect("publer");
        }
        for _ in 0..3 {
            record_post_at("outstand", &path, 4, 2026).expect("outstand");
        }
        assert_eq!(get_usage_at("publer", &path, 4, 2026).unwrap().count, 5);
        assert_eq!(get_usage_at("outstand", &path, 4, 2026).unwrap().count, 3);
        cleanup(&path);
    }
}
