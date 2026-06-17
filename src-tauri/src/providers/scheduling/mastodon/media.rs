// SPDX-License-Identifier: BUSL-1.1

use super::ProviderError;

/// SSRF validation for image URLs before downloading for Mastodon media upload.
///
/// Rejects non-HTTPS URLs and URLs that resolve to private IP ranges.
/// Security rule: no fetches to private ranges — same policy as instance validation.
pub(super) async fn validate_image_url(url: &str) -> Result<(), ProviderError> {
    if !url.starts_with("https://") {
        return Err(ProviderError::Unknown(format!(
            "Image URL must use HTTPS: {}", url
        )));
    }
    let hostname = url
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    if hostname.is_empty() {
        return Err(ProviderError::Unknown("Image URL has no hostname".to_string()));
    }
    let addr = format!("{}:443", hostname);
    let addrs: Vec<_> = tokio::net::lookup_host(&addr)
        .await
        .map_err(|e| ProviderError::Unknown(format!("Cannot resolve image URL host {}: {}", hostname, e)))?
        .collect();
    for socket_addr in &addrs {
        if crate::ssrf_validation::is_private_ip(&socket_addr.ip()) {
            return Err(ProviderError::Unknown(format!(
                "Image URL resolves to a private IP address ({})", socket_addr.ip()
            )));
        }
    }
    Ok(())
}

/// Download the image at `image_url` and upload it to the Mastodon `/api/v1/media` endpoint.
///
/// SSRF validation is skipped in test builds so mock HTTP servers on loopback can be used.
/// Returns the Mastodon `media_id` string for inclusion in the status `media_ids` array.
pub(super) async fn upload_media_from_url(
    client: &reqwest::Client,
    api_base: &str,
    access_token: &str,
    image_url: &str,
) -> Result<String, ProviderError> {
    #[cfg(not(test))]
    validate_image_url(image_url).await?;

    let image_response = client
        .get(image_url)
        .send()
        .await
        .map_err(|e| ProviderError::NetworkError(format!("Failed to download image: {}", e)))?;

    if !image_response.status().is_success() {
        return Err(ProviderError::Unknown(format!(
            "Image download failed with status {}",
            image_response.status()
        )));
    }

    let content_type = image_response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = image_response
        .bytes()
        .await
        .map_err(|e| ProviderError::Unknown(format!("Failed to read image bytes: {}", e)))?;

    upload_media_bytes(client, api_base, access_token, &bytes, &content_type).await
}

/// Upload raw image bytes to Mastodon's `/api/v1/media` endpoint.
///
/// Exposed as `pub(super)` so tests can call it directly without the download step.
/// Returns the Mastodon `media_id`.
pub(super) async fn upload_media_bytes(
    client: &reqwest::Client,
    api_base: &str,
    access_token: &str,
    bytes: &[u8],
    content_type: &str,
) -> Result<String, ProviderError> {
    let url = format!("{}/media", api_base);
    let part = reqwest::multipart::Part::bytes(bytes.to_vec())
        .file_name("image.jpg")
        .mime_str(content_type)
        .map_err(|e| ProviderError::Unknown(format!("Invalid image content-type: {}", e)))?;
    let form = reqwest::multipart::Form::new().part("file", part);

    let response = client
        .post(&url)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::HttpError { status, body });
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| ProviderError::Unknown(format!("Failed to parse media upload response: {}", e)))?;

    json["id"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| ProviderError::Unknown(format!("Missing id in media upload response: {}", json)))
}
