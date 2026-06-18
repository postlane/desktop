// SPDX-License-Identifier: BUSL-1.1

use dashmap::DashMap;
use std::sync::{Arc, LazyLock, Mutex};

/// Denominator unit for ModelStatsResponse — each sent platform counts as one post.
/// Use this constant in all callers so a typo is a compile error, not a silent mismatch.
pub const DENOMINATOR_UNIT_PLATFORM_APPROVAL: &str = "platform_approval";

/// Tauri event emitted by delete_project and consumed by ProjectsProvider.refresh().
/// Defined here so all emit sites and all subscribe sites use the same constant
/// rather than an inline string — divergence causes silent refresh failures.
pub const PROJECTS_CHANGED_EVENT: &str = "projects-changed";

/// Re-exported for callers that want the canonical scheduler provider list without
/// importing scheduler_credentials directly. Authoritative list lives in scheduler_credentials.rs.
pub use crate::scheduler_credentials::VALID_PROVIDERS;

/// Per-post-folder lock map shared by all commands that read then write meta.json
/// (approve_post, save_post_draft). Keyed by `"{canonical_repo_path}\x00{post_folder}"`.
/// The null-byte separator is invalid in POSIX paths, making key collisions impossible.
/// Uses tokio::sync::Mutex so the guard can be held across .await without blocking threads.
pub static POST_META_LOCKS: LazyLock<DashMap<String, Arc<tokio::sync::Mutex<()>>>> =
    LazyLock::new(DashMap::new);

/// Per-post-folder lock map for synchronous (non-async) read-mutate-write operations on meta.json.
/// Used by callers running on non-tokio threads (e.g. file-watcher callbacks) that cannot
/// hold a tokio::sync::Mutex across an await point.
/// Keyed identically to POST_META_LOCKS so the two maps guard the same resources.
pub static POST_META_SYNC_LOCKS: LazyLock<DashMap<String, Arc<Mutex<()>>>> =
    LazyLock::new(DashMap::new);

/// Per-config-path lock map for synchronous read-mutate-write operations on config.json.
/// Keyed by the canonical path string. Prevents account_id and account_name writers from
/// interleaving their read-modify-write cycles and clobbering each other.
/// Uses std::sync::Mutex (not tokio) because callers are synchronous.
pub static CONFIG_JSON_LOCKS: LazyLock<DashMap<String, Arc<Mutex<()>>>> =
    LazyLock::new(DashMap::new);
