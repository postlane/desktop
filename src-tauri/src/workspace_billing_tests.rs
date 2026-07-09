// SPDX-License-Identifier: BUSL-1.1
// Tests for workspace_billing.rs — checklist 24.4.9 (subscribe/portal/deactivate).

use super::{
    billing_status_to_license_info, deactivate_workspace_with_client, get_billing_status_with_client,
    open_billing_portal_with_client, record_workspace_upgrade_prompted, subscribe_workspace_with_client,
    BillingStatusResponse,
};
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

// ── get_billing_status (checklist 24.3.6a) ────────────────────────────────────

#[tokio::test]
async fn test_billing_status_returns_owner_status() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-1/billing-status");
        then.status(200).json_body(serde_json::json!({ "status": "free_owned" }));
    });

    let result = get_billing_status_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(result, Ok(BillingStatusResponse { status: "free_owned".to_string(), owner: None }));
}

#[tokio::test]
async fn test_billing_status_returns_collaborator_status_with_owner() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-1/billing-status");
        then.status(200).json_body(serde_json::json!({ "status": "collaborator", "owner": "Dana Kim" }));
    });

    let result = get_billing_status_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert_eq!(
        result,
        Ok(BillingStatusResponse { status: "collaborator".to_string(), owner: Some("Dana Kim".to_string()) })
    );
}

#[tokio::test]
async fn test_billing_status_returns_err_on_5xx() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/v1/projects/proj-1/billing-status");
        then.status(503);
    });

    let result = get_billing_status_with_client("proj-1", &build_test_client(), &server.base_url(), "tok").await;
    assert!(result.is_err());
}

// ── billing_status_to_license_info mapping (checklist 24.3.6a) ───────────────

#[test]
fn test_billing_status_to_license_info_owner_status() {
    let response = BillingStatusResponse { status: "paid_required".to_string(), owner: None };
    let info = billing_status_to_license_info("proj-1", &response, "2026-07-09T00:00:00Z");
    assert_eq!(info.project_id, "proj-1");
    assert_eq!(info.status, "paid_required");
    assert!(info.is_owner, "any status other than collaborator implies is_owner");
    assert_eq!(info.status_updated_at, "2026-07-09T00:00:00Z");
}

#[test]
fn test_billing_status_to_license_info_collaborator_status() {
    let response = BillingStatusResponse { status: "collaborator".to_string(), owner: Some("Dana".to_string()) };
    let info = billing_status_to_license_info("proj-1", &response, "2026-07-09T00:00:00Z");
    assert!(!info.is_owner, "collaborator status implies not is_owner");
}

// ── workspace_upgrade_prompted telemetry (checklist 24.3.6a, pulled forward
// from 24.4.11c since this item's own TDD test needs it) ────────────────────

#[test]
fn test_record_workspace_upgrade_prompted_queues_when_consent_given() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_upgrade_prompted(&state, true, "proj-1");
    assert_eq!(state.telemetry.queue_len(), 1, "one event must be queued");
    let events = state.telemetry.peek_queue();
    assert_eq!(events[0].name, "workspace_upgrade_prompted");
    assert_eq!(events[0].properties["project_id"], "proj-1");
}

#[test]
fn test_record_workspace_upgrade_prompted_no_op_when_consent_not_given() {
    let state = crate::test_fixtures::make_state(vec![]);
    record_workspace_upgrade_prompted(&state, false, "proj-1");
    assert_eq!(state.telemetry.queue_len(), 0, "no event without consent");
}
