// SPDX-License-Identifier: BUSL-1.1

pub mod cache;
pub mod client;
pub mod engagement_sync;
pub mod sites;

use serde::{Deserialize, Serialize};

/// Analytics attribution data returned by get_post_analytics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PostAnalytics {
    pub sessions: u64,
    pub unique_sessions: u64,
    pub top_referrer: Option<String>,
}
