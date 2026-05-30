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

// ── 22.7.15: KEYRING_PATTERNS completeness meta-test ─────────────────────────
// Scans all non-test production source files for `.set_password(` calls.
// If a new call site appears in a file not listed below, this test fails —
// forcing the author to register the key pattern in KEYRING_PATTERNS.

fn files_with_set_password(src_dir: &std::path::Path) -> Vec<String> {
    let mut result = Vec::new();
    let mut queue = vec![src_dir.to_path_buf()];
    while let Some(dir) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() { queue.push(path); continue; }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") { continue; }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            if content.contains(".set_password(") {
                if let Ok(rel) = path.strip_prefix(src_dir) {
                    result.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
    result
}

#[test]
fn test_all_set_password_call_sites_have_keyring_pattern() {
    // Every production file that calls .set_password() must be registered here.
    // Add a new entry here ONLY AFTER also registering the key pattern in KEYRING_PATTERNS.
    let expected: std::collections::HashSet<&str> = [
        "app_lifecycle.rs",             // "license"
        "credential_migration.rs",      // "{provider}/{project_id}" — scheduler migration path
        "mastodon_app_registration.rs", // "mastodon_client_id/{instance}", "mastodon_client_secret/{instance}"
        "mastodon_token_exchange.rs",   // "mastodon/{project_id}/{instance}", active_instance, active_username
        "scheduler_credentials.rs",     // "{provider}/{project_id}" + transient "__libsecret_test__" (libsecret probe, immediately deleted)
        "unsplash_search.rs",           // "postlane/unsplash_access_key"
    ].iter().copied().collect();

    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = files_with_set_password(&src_dir);

    let unexpected: Vec<_> = found.iter()
        .filter(|f| !expected.contains(f.as_str()))
        .collect();

    assert!(unexpected.is_empty(),
        "New .set_password() call sites found — register the key pattern in KEYRING_PATTERNS \
         then add the file to this list in credential_store_tests.rs:\n  {}",
        unexpected.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  "));
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
