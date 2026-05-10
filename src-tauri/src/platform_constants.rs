// SPDX-License-Identifier: BUSL-1.1

use dashmap::DashMap;
use std::sync::{Arc, LazyLock};

/// Scheduler provider names supported in v1.
/// All scheduler credential commands validate against this list.
/// Divergence between commands is a compile error, not a silent runtime mismatch.
pub const KNOWN_SCHEDULER_PROVIDERS: &[&str] = &["zernio"];

/// Social platform identifiers accepted by approve_post and get_org_published.
/// Distinct namespace from KNOWN_SCHEDULER_PROVIDERS — social platforms and scheduler
/// providers are not interchangeable (one Zernio account publishes to all platforms).
pub const KNOWN_SOCIAL_PLATFORMS: &[&str] = &["x", "linkedin", "bluesky"];

/// Denominator unit for ModelStatsResponse — each sent platform counts as one post.
/// Use this constant in all callers so a typo is a compile error, not a silent mismatch.
pub const DENOMINATOR_UNIT_PLATFORM_APPROVAL: &str = "platform_approval";

/// Tauri event emitted by delete_project and consumed by ProjectsProvider.refresh().
/// Defined here so all emit sites and all subscribe sites use the same constant
/// rather than an inline string — divergence causes silent refresh failures.
pub const PROJECTS_CHANGED_EVENT: &str = "projects-changed";

/// Per-post-folder lock map shared by all commands that read then write meta.json
/// (approve_post, save_post_draft). Keyed by `"{canonical_repo_path}\x00{post_folder}"`.
/// The null-byte separator is invalid in POSIX paths, making key collisions impossible.
/// Uses tokio::sync::Mutex so the guard can be held across .await without blocking threads.
pub static POST_META_LOCKS: LazyLock<DashMap<String, Arc<tokio::sync::Mutex<()>>>> =
    LazyLock::new(DashMap::new);
