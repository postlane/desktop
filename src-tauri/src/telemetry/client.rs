// SPDX-License-Identifier: BUSL-1.1

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Mutex;

const TELEMETRY_ENDPOINT: &str = "https://api.postlane.dev/v1/telemetry";
const FLUSH_BATCH_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryEvent {
    pub name: String,
    pub properties: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

/// In-memory queue for opt-in product telemetry events.
/// Events are only queued when the user has given consent.
pub struct TelemetryClient {
    queue: Mutex<Vec<TelemetryEvent>>,
    http: reqwest::Client,
    endpoint: String,
}

impl TelemetryClient {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Vec::new()),
            http: crate::providers::scheduling::build_client(),
            endpoint: TELEMETRY_ENDPOINT.to_string(),
        }
    }

    /// Creates a client pointing at a custom endpoint (for tests).
    pub fn with_endpoint(endpoint: &str) -> Self {
        Self { endpoint: endpoint.to_string(), ..Self::new() }
    }

    /// Records an event if the user has consented. No-op if consent is false.
    pub fn record(&self, consent: bool, event: &str, properties: serde_json::Value) {
        if !consent { return; }
        let ev = TelemetryEvent {
            name: event.to_string(),
            properties,
            occurred_at: Utc::now(),
        };
        if let Ok(mut q) = self.queue.lock() {
            q.push(ev);
        }
    }

    /// Returns the number of events currently queued (for tests).
    pub fn queue_len(&self) -> usize {
        self.queue.lock().map(|q| q.len()).unwrap_or(0)
    }

    /// Flushes queued events to the backend. On any error, discards without retry.
    /// Never surfaces errors to the caller.
    pub async fn flush(&self, license_token: &str) {
        let events = {
            let mut q = match self.queue.lock() {
                Ok(q) => q,
                Err(_) => return,
            };
            std::mem::take(&mut *q)
        };
        if events.is_empty() { return; }
        for chunk in events.chunks(FLUSH_BATCH_LIMIT) {
            let _ = self.http
                .post(&self.endpoint)
                .bearer_auth(license_token)
                .json(&serde_json::json!({ "events": chunk }))
                .send()
                .await;
        }
    }
}

impl Default for TelemetryClient {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[test]
    fn test_telemetry_no_op_when_consent_false() {
        let client = TelemetryClient::new();
        client.record(false, "skill_invoked", serde_json::json!({ "skill": "draft-x" }));
        assert_eq!(client.queue_len(), 0, "No event should be queued without consent");
    }

    #[test]
    fn test_telemetry_queues_when_consent_true() {
        let client = TelemetryClient::new();
        client.record(true, "skill_invoked", serde_json::json!({ "skill": "draft-x" }));
        assert_eq!(client.queue_len(), 1, "Event should be queued with consent");
    }

    #[tokio::test]
    async fn test_telemetry_flush_discards_on_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/telemetry");
            then.status(500);
        });
        let client = TelemetryClient::with_endpoint(&format!("{}/v1/telemetry", server.base_url()));
        client.record(true, "post_approved", serde_json::json!({}));
        assert_eq!(client.queue_len(), 1);
        client.flush("tok").await;
        assert_eq!(client.queue_len(), 0, "Queue must be cleared even after flush error");
    }

    #[tokio::test]
    async fn test_telemetry_flush_sends_events() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/telemetry");
            then.status(200);
        });
        let client = TelemetryClient::with_endpoint(&format!("{}/v1/telemetry", server.base_url()));
        client.record(true, "repo_connected", serde_json::json!({}));
        client.flush("tok").await;
        mock.assert();
        assert_eq!(client.queue_len(), 0);
    }
}
