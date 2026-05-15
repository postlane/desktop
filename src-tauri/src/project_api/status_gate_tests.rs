// SPDX-License-Identifier: BUSL-1.1

use super::{check_billing_gate_with_client, check_project_status_with_client};
use crate::project_registry::{BillingGate, ProjectStatus};
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── check_project_status ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_returns_owned_for_200_owned_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-123");
        then.status(200)
            .json_body(serde_json::json!({ "status": "owned", "tier": "free" }));
    });

    let status = check_project_status_with_client(
        "proj-123",
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(status, ProjectStatus::Owned);
}

#[tokio::test]
async fn test_returns_not_found_for_404_response() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-456");
        then.status(404)
            .json_body(serde_json::json!({ "id": "proj-456", "status": "not_found" }));
    });

    let status = check_project_status_with_client(
        "proj-456",
        &build_test_client(),
        &server.base_url(),
        "tok",
    )
    .await;
    assert_eq!(status, ProjectStatus::NotFound);
}

#[tokio::test]
async fn test_returns_offline_on_network_error() {
    let status = check_project_status_with_client(
        "proj-789",
        &build_test_client(),
        "http://127.0.0.1:19998",
        "tok",
    )
    .await;
    assert_eq!(status, ProjectStatus::Offline);
}

// ── check_billing_gate ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_gate_returns_free_for_new_user() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/gate");
        then.status(200)
            .json_body(serde_json::json!({ "slot": "free" }));
    });

    let gate =
        check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(gate, BillingGate::Free);
}

#[tokio::test]
async fn test_gate_returns_none_when_no_free_slot() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/gate");
        then.status(200)
            .json_body(serde_json::json!({ "slot": "none" }));
    });

    let gate =
        check_billing_gate_with_client(&build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(gate, BillingGate::None);
}

#[tokio::test]
async fn test_gate_returns_offline_on_network_error() {
    let gate =
        check_billing_gate_with_client(&build_test_client(), "http://127.0.0.1:19997", "tok")
            .await;
    assert_eq!(gate, BillingGate::Offline);
}
