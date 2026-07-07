// SPDX-License-Identifier: BUSL-1.1
//! Tests for §22.9.10 — error registry completeness and uniqueness.

use super::ERROR_REGISTRY;

// Expected codes: every PL-* string in production source must be registered here.
const EXPECTED_CODES: &[&str] = &[
    "PL-WS-001", "PL-WS-002", "PL-WS-003",
    "PL-MIG-001", "PL-MIG-002",
    "PL-DEL-000", "PL-DEL-001", "PL-DEL-002", "PL-DEL-003", "PL-DEL-004", "PL-DEL-005",
];

#[test]
fn test_error_registry_contains_all_expected_codes() {
    for expected in EXPECTED_CODES {
        assert!(
            ERROR_REGISTRY.iter().any(|e| e.code == *expected),
            "error_registry missing code: {expected}"
        );
    }
}

#[test]
fn test_error_registry_has_no_duplicate_codes() {
    let mut seen = std::collections::HashSet::new();
    for entry in ERROR_REGISTRY {
        assert!(seen.insert(entry.code), "duplicate code in ERROR_REGISTRY: {}", entry.code);
    }
}

#[test]
fn test_error_registry_all_codes_have_non_empty_summary() {
    for entry in ERROR_REGISTRY {
        assert!(!entry.summary.is_empty(), "empty summary for code: {}", entry.code);
    }
}

#[test]
fn test_error_registry_source_codes_all_registered() {
    // Scans production source files for PL-* codes and asserts each is in the registry.
    // Adding a new PL-* code in source without a registry entry fails this test.
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut unregistered: Vec<String> = Vec::new();
    let mut queue = vec![src_dir.clone()];
    while let Some(dir) = queue.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() { queue.push(path); continue; }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.ends_with(".rs") || name.ends_with("_tests.rs") { continue; }
            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            for segment in content.split('"') {
                if segment.starts_with("PL-") && segment.len() <= 12
                    && !ERROR_REGISTRY.iter().any(|e| e.code == segment) {
                    unregistered.push(format!("{segment} (in {name})"));
                }
            }
        }
    }
    let unregistered: Vec<_> = {
        let mut seen = std::collections::HashSet::new();
        unregistered.into_iter().filter(|s| seen.insert(s.clone())).collect()
    };
    assert!(unregistered.is_empty(),
        "PL-* codes found in source but not in ERROR_REGISTRY — add them and a runbook entry:\n  {}",
        unregistered.join("\n  "));
}
