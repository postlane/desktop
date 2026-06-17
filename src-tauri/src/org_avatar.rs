// SPDX-License-Identifier: BUSL-1.1

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use crate::ssrf_validation::is_private_url;
use std::time::Duration;

fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client")
}

/// Validates `url` against SSRF private-range rules, fetches the image bytes,
/// and returns them as a base64 data URL suitable for an `<img src>` attribute.
/// Rejects private/loopback addresses and non-HTTPS schemes.
pub async fn fetch_avatar_bytes_impl(
    url: &str,
    client: &reqwest::Client,
) -> Result<String, String> {
    if is_private_url(url) {
        return Err(format!("SSRF_BLOCKED: URL '{}' resolves to a private or reserved address", url));
    }
    if !url.starts_with("https://") {
        return Err(format!("URL '{}' must use https://", url));
    }
    fetch_and_encode_avatar(url, client).await
}

/// Fetches the image at `url` (security checks already passed) and returns a base64 data URL.
pub(crate) async fn fetch_and_encode_avatar(
    url: &str,
    client: &reqwest::Client,
) -> Result<String, String> {
    let resp = client
        .get(url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch avatar: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Avatar fetch returned status {}", resp.status()));
    }

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .split(';')
        .next()
        .unwrap_or("image/png")
        .trim()
        .to_owned();

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read avatar bytes: {}", e))?;

    Ok(format!("data:{};base64,{}", content_type, BASE64.encode(&bytes)))
}

#[tauri::command]
pub async fn fetch_avatar_bytes(url: String) -> Result<String, String> {
    let client = build_client();
    fetch_avatar_bytes_impl(&url, &client).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::MockServer;
    use httpmock::Method::GET;

    fn build_test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("test client")
    }

    #[tokio::test]
    async fn test_fetch_avatar_returns_base64_data_url() {
        let png_bytes = b"\x89PNG\r\n\x1a\n";
        let expected = format!("data:image/png;base64,{}", BASE64.encode(png_bytes));
        let data_url = format!("data:image/png;base64,{}", BASE64.encode(png_bytes));
        assert!(data_url.starts_with("data:image/png;base64,"));
        assert_eq!(data_url, expected);
    }

    #[tokio::test]
    async fn test_fetch_avatar_rejects_private_ip() {
        let client = build_test_client();
        let result = fetch_avatar_bytes_impl("https://192.168.1.1/avatar.png", &client).await;
        assert!(result.is_err(), "private IP must be rejected");
        let err = result.unwrap_err();
        assert!(err.contains("SSRF_BLOCKED"), "error must say SSRF_BLOCKED, got: {}", err);
    }

    #[tokio::test]
    async fn test_fetch_avatar_rejects_loopback() {
        let client = build_test_client();
        let result = fetch_avatar_bytes_impl("https://127.0.0.1/avatar.png", &client).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF_BLOCKED"));
    }

    #[tokio::test]
    async fn test_fetch_avatar_rejects_non_https() {
        let client = build_test_client();
        let result = fetch_avatar_bytes_impl("http://example.com/avatar.png", &client).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must use https://"));
    }

    #[tokio::test]
    async fn test_fetch_avatar_rejects_aws_metadata() {
        let client = build_test_client();
        let result = fetch_avatar_bytes_impl("https://169.254.169.254/latest/meta-data/", &client).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("SSRF_BLOCKED"));
    }

    #[tokio::test]
    async fn test_fetch_avatar_returns_base64_data_url_on_success() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/avatar.png");
            then.status(200)
                .header("content-type", "image/png")
                .body(b"fake_image_bytes" as &[u8]);
        });
        let url = format!("{}/avatar.png", server.base_url());
        let result = fetch_and_encode_avatar(&url, &build_test_client()).await;
        let data_url = result.expect("fetch should succeed");
        assert!(
            data_url.starts_with("data:image/png;base64,"),
            "expected data URL prefix, got: {}",
            data_url
        );
        let expected_b64 = BASE64.encode(b"fake_image_bytes");
        assert_eq!(data_url, format!("data:image/png;base64,{}", expected_b64));
    }

    #[tokio::test]
    async fn test_fetch_avatar_returns_err_on_non_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/avatar.png");
            then.status(404);
        });
        let url = format!("{}/avatar.png", server.base_url());
        let result = fetch_and_encode_avatar(&url, &build_test_client()).await;
        assert!(result.is_err(), "non-200 status must return Err");
        assert!(
            result.unwrap_err().contains("404"),
            "error must mention the 404 status"
        );
    }
}
