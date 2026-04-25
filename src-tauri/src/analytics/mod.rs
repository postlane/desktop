// SPDX-License-Identifier: BUSL-1.1

pub mod cache;
pub mod client;
pub mod engagement_sync;
pub mod sites;

use serde::{Deserialize, Serialize};

/// Analytics attribution data returned by get_post_analytics.
/// `configured` is false when no site token exists for the repo (snippet not installed).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PostAnalytics {
    pub configured: bool,
    pub sessions: u64,
    pub unique_sessions: u64,
    pub top_referrer: Option<String>,
}
