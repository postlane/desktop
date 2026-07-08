// SPDX-License-Identifier: BUSL-1.1
// Tests for workspace_billing.rs — checklist 24.4.9 (subscribe/portal/deactivate).

use super::{deactivate_workspace_with_client, open_billing_portal_with_client, subscribe_workspace_with_client};
use crate::providers::scheduling::build_client;
use httpmock::prelude::*;

fn build_test_client() -> reqwest::Client {
    build_client()
}

// ── subscribe_workspace ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_subscribe_returns_checkout_url_when_present() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/billing/subscribe")
            .json_body(serde_json::json!({ "project_id": "proj-1", "idempotency_key": "key-1" }));
        then.status(200).json_body(serde_json::json!({ "checkout_url": "https://checkout.stripe.com/abc" }));
    });

    let result =
        subscribe_workspace_with_client("proj-1", "key-1", &build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(result, Ok(Some("https://checkout.stripe.com/abc".to_string())));
}

#[tokio::test]
async fn test_subscribe_returns_none_when_checkout_url_is_null() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/subscribe");
        then.status(200).json_body(serde_json::json!({ "checkout_url": null }));
    });

    let result =
        subscribe_workspace_with_client("proj-1", "key-1", &build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(result, Ok(None));
}

#[tokio::test]
async fn test_subscribe_returns_err_on_409_insufficient_workspaces() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/subscribe");
        then.status(409).json_body(serde_json::json!({ "error": "insufficient_owned_workspaces" }));
    });

    let result =
        subscribe_workspace_with_client("proj-1", "key-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_subscribe_returns_err_on_network_failure() {
    let result = subscribe_workspace_with_client(
        "proj-1", "key-1", &build_test_client(), "http://127.0.0.1:19994", "tok",
    )
    .await;
    assert!(result.is_err());
}

// ── open_billing_portal ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_portal_returns_url_on_success() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/billing/portal")
            .json_body(serde_json::json!({ "project_id": "proj-1" }));
        then.status(200).json_body(serde_json::json!({ "url": "https://billing.stripe.com/session/xyz" }));
    });

    let result = open_billing_portal_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(result, Ok("https://billing.stripe.com/session/xyz".to_string()));
}

#[tokio::test]
async fn test_portal_returns_err_on_409_no_stripe_customer() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/portal");
        then.status(409).json_body(serde_json::json!({ "error": "no_stripe_customer" }));
    });

    let result = open_billing_portal_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}

// ── deactivate_workspace ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_deactivate_succeeds_on_200() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST)
            .path("/v1/billing/deactivate/proj-1")
            .json_body(serde_json::json!({ "idempotency_key": "key-1" }));
        then.status(200).json_body(serde_json::json!({ "status": "inactive" }));
    });

    let result =
        deactivate_workspace_with_client("proj-1", "key-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_deactivate_returns_err_on_403_not_owner() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(POST).path("/v1/billing/deactivate/proj-1");
        then.status(403).json_body(serde_json::json!({ "error": "forbidden" }));
    });

    let result =
        deactivate_workspace_with_client("proj-1", "key-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}
