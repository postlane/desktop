// SPDX-License-Identifier: BUSL-1.1
//! Tests for §22.7.9 — KEYRING_PATTERNS registry.

use super::*;

// ── 22.7.15: KEYRING_PATTERNS contains all known key patterns ─────────────────

#[test]
fn test_keyring_patterns_contains_license() {
    assert!(KEYRING_PATTERNS.contains(&"license"), "license key must be registered");
}

#[test]
fn test_keyring_patterns_contains_unsplash() {
    assert!(KEYRING_PATTERNS.contains(&"postlane/unsplash_access_key"),
        "unsplash key must be registered");
}

#[test]
fn test_keyring_patterns_contains_all_scheduler_providers() {
    for provider in SCHEDULER_PROVIDERS {
        let pattern = format!("{}/", provider);
        assert!(
            KEYRING_PATTERNS.iter().any(|p| *p == pattern.as_str()),
            "scheduler provider pattern {pattern} must be in KEYRING_PATTERNS"
        );
    }
}

#[test]
fn test_keyring_patterns_contains_mastodon_patterns() {
    let required = [
        "mastodon_client_id/",
        "mastodon_client_secret/",
        "mastodon/",
        "mastodon_active_instance/",
        "mastodon_active_username/",
    ];
    for pattern in required {
        assert!(KEYRING_PATTERNS.contains(&pattern),
            "Mastodon pattern '{pattern}' must be in KEYRING_PATTERNS");
    }
}

#[test]
fn test_keyring_patterns_has_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for p in KEYRING_PATTERNS {
        assert!(seen.insert(*p), "Duplicate pattern in KEYRING_PATTERNS: {p}");
    }
}

// ── project_keyring_keys generates all project-scoped keys ────────────────────

#[test]
fn test_project_keyring_keys_includes_all_schedulers() {
    let keys = project_keyring_keys("proj-abc");
    for provider in SCHEDULER_PROVIDERS {
        assert!(
            keys.iter().any(|k| k == &format!("{provider}/proj-abc")),
            "project_keyring_keys must include {provider}/proj-abc"
        );
    }
}

#[test]
fn test_project_keyring_keys_includes_mastodon_entries() {
    let keys = project_keyring_keys("proj-xyz");
    assert!(keys.iter().any(|k| k == "mastodon_active_instance/proj-xyz"));
    assert!(keys.iter().any(|k| k == "mastodon_active_username/proj-xyz"));
}

#[test]
fn test_global_keyring_keys_returns_expected_entries() {
    let globals = global_keyring_keys();
    assert!(globals.contains(&"license"));
    assert!(globals.contains(&"postlane/unsplash_access_key"));
}
