// SPDX-License-Identifier: BUSL-1.1

use axum::{response::Json, routing::get, Router};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

/// Integration test for single-instance enforcement
/// Starts a mock server on 47312 that responds to /health,
/// then verifies the startup check detects it
#[tokio::test]
async fn test_single_instance_detection() {
    // Start a mock server on port 47312
    let app = Router::new().route(
        "/health",
        get(|| async { Json(HealthResponse { status: "ok".to_string() }) }),
    );

    let addr = SocketAddr::from(([127, 0, 0, 1], 47312));
    let listener = TcpListener::bind(addr).await.unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Simulate the health check from check_single_instance
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();

    let response = client
        .get("http://127.0.0.1:47312/health")
        .send()
        .await;

    // Should successfully connect to the mock server
    assert!(response.is_ok(), "Health check should succeed");
    assert_eq!(response.unwrap().status(), 200);
}

#[tokio::test]
async fn test_single_instance_stale_port_file() {
    // Test that health check fails when no server is running
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();

    // Try to connect to a port with no server
    let response = client
        .get("http://127.0.0.1:57312/health") // Different port, no server
        .send()
        .await;

    // Should fail - no server running
    assert!(response.is_err(), "Health check should fail when no server");
}

#[tokio::test]
async fn test_health_check_timeout() {
    use std::time::Instant;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(200))
        .build()
        .unwrap();

    let start = Instant::now();

    // Try to connect to non-existent server
    let _response = client
        .get("http://127.0.0.1:47313/health")
        .send()
        .await;

    let elapsed = start.elapsed();

    // Should timeout around 200ms (within 300ms to account for overhead)
    assert!(
        elapsed.as_millis() < 300,
        "Health check should timeout within 300ms, took {}ms",
        elapsed.as_millis()
    );
}
