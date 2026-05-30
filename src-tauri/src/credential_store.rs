// SPDX-License-Identifier: BUSL-1.1

//! Central registry of all keyring key patterns written by any module.
//!
//! Every `set_password("postlane", key, ...)` call in the codebase must have a
//! corresponding entry here. `delete_account_impl` Step 4 and workspace
//! soft-remove both iterate this list — no ad-hoc keyring deletes elsewhere.
//!
//! Pattern format: literal key or `prefix/` for prefix-matched keys.

/// All keyring key patterns (service = `"postlane"` for all entries).
///
/// Updating: if you add a `set_password` call anywhere, add the key pattern
/// here AND update the test in `credential_store_tests.rs`.
pub const KEYRING_PATTERNS: &[&str] = &[
    // Global keys (not project-scoped)
    "license",
    "postlane/unsplash_access_key",
    // Mastodon — instance-scoped (key = "mastodon_client_id/{instance}")
    "mastodon_client_id/",
    // Mastodon — instance-scoped (key = "mastodon_client_secret/{instance}")
    "mastodon_client_secret/",
    // Mastodon — project+instance-scoped (key = "mastodon/{project_id}/{instance}")
    "mastodon/",
    // Mastodon — project-scoped active state
    "mastodon_active_instance/",
    "mastodon_active_username/",
    // Scheduler credentials — project-scoped (key = "{provider}/{project_id}")
    "zernio/",
    "upload_post/",
    "ayrshare/",
    "publer/",
    "outstand/",
    "buffer/",
    "substack_notes/",
    "webhook/",
];

/// Scheduler provider prefixes used for project-scoped credential keys.
pub const SCHEDULER_PROVIDERS: &[&str] = &[
    "zernio", "upload_post", "ayrshare", "publer",
    "outstand", "buffer", "substack_notes", "webhook",
];

/// Returns all concrete keyring keys for a given `project_id`.
/// Used by workspace soft-remove and `delete_account_impl` Step 4.
pub fn project_keyring_keys(project_id: &str) -> Vec<String> {
    let mut keys = Vec::new();
    for provider in SCHEDULER_PROVIDERS {
        keys.push(format!("{}/{}", provider, project_id));
    }
    keys.push(crate::mastodon_connection::active_instance_key(project_id));
    keys.push(crate::mastodon_connection::active_username_key(project_id));
    keys
}

/// Returns all global (non-project-scoped) keyring keys.
pub fn global_keyring_keys() -> &'static [&'static str] {
    &["license", "postlane/unsplash_access_key"]
}

#[cfg(test)]
#[path = "credential_store_tests.rs"]
mod tests;
