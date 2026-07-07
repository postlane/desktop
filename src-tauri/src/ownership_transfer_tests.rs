// SPDX-License-Identifier: BUSL-1.1
// Tests for ownership_transfer.rs — checklist 24.4.15/24.4.15a.

use super::{initiate_departure_with_client, transfer_to_admin_with_client};
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── transfer_to_admin ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_transfer_succeeds_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/billing/transfer/proj-1")
            .json_body(serde_json::json!({ "idempotency_key": "key-1", "target_user_id": "admin-1" }));
        then.status(200).json_body(serde_json::json!({ "checkout_url": null }));
    });

    let result =
        transfer_to_admin_with_client("proj-1", "admin-1", "key-1", &build_test_client(), &server.base_url(), "tok")
            .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_transfer_returns_err_on_400_member_role_target() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/transfer/proj-1");
        then.status(400).json_body(serde_json::json!({ "error": "target_member_role_cannot_receive_transfer" }));
    });

    let result =
        transfer_to_admin_with_client("proj-1", "member-1", "key-1", &build_test_client(), &server.base_url(), "tok")
            .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_transfer_returns_err_on_network_failure() {
    let result = transfer_to_admin_with_client(
        "proj-1",
        "admin-1",
        "key-1",
        &build_test_client(),
        "http://127.0.0.1:19995",
        "tok",
    )
    .await;
    assert!(result.is_err());
}

// ── initiate_departure ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_initiate_departure_succeeds_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/initiate-departure/proj-1");
        then.status(200).json_body(serde_json::json!({ "status": "owner_departing" }));
    });

    let result = initiate_departure_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_initiate_departure_returns_err_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/initiate-departure/proj-1");
        then.status(403).json_body(serde_json::json!({ "error": "forbidden" }));
    });

    let result = initiate_departure_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}
