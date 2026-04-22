// SPDX-License-Identifier: BUSL-1.1

use std::path::PathBuf;
use std::fs;
use crate::types::PostMeta;

const VALID_PLATFORMS: &[&str] = &[
    "x", "bluesky", "mastodon",
    "linkedin", "substack_notes", "substack", "product_hunt", "show_hn", "changelog",
];

// Hostnames whose URLs are always direct images even without a file extension.
const IMAGE_CDN_HOSTNAMES: &[&str] = &[
    "images.unsplash.com",
    "cdn.pixabay.com",
    "images.pexels.com",
    "lh3.googleusercontent.com",
    "pbs.twimg.com",
    "media.giphy.com",
];

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif", "svg"];

/// Returns true if the URL's host is a private/reserved address.
/// Blocks: loopback, RFC-1918 private, link-local, unique-local IPv6, localhost.
fn is_private_host(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return true,
    };
    match parsed.host() {
        None => true,
        Some(url::Host::Domain(d)) => {
            matches!(d, "localhost" | "localhost.localdomain")
        }
        Some(url::Host::Ipv4(v4)) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
                || v4.is_broadcast() || v4.is_unspecified()
        }
        Some(url::Host::Ipv6(v6)) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00 == 0xfc00) // fc00::/7 unique-local
        }
    }
}

fn og_regex() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"<meta[^>]+property=["']og:image["'][^>]+content=["']([^"']+)["']"#
        ).expect("og:image regex is valid")
    })
}

fn og_regex_rev() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:image["']"#
        ).expect("og:image rev regex is valid")
    })
}

/// Returns true if the URL points directly to an image file rather than a web page.
/// Used by the frontend to decide whether to attempt OG image extraction.
pub fn is_direct_image_url(url: &str) -> bool {
    // Check known image CDN hostnames first.
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            if IMAGE_CDN_HOSTNAMES.contains(&host) {
                return true;
            }
        }
        // Check file extension in the path, ignoring query string.
        let path = parsed.path().to_lowercase();
        return IMAGE_EXTENSIONS.iter().any(|ext| path.ends_with(&format!(".{}", ext)));
    }
    false
}

pub fn update_post_content_impl(
    repo_path: &str,
    post_folder: &str,
    platform: &str,
    new_content: &str,
) -> Result<(), String> {
    if !VALID_PLATFORMS.contains(&platform) {
        return Err(format!(
            "Invalid platform: '{}'. Must be one of: {}",
            platform,
            VALID_PLATFORMS.join(", ")
        ));
    }

    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err(
            "Invalid post folder: must not contain path separators or '..'".to_string(),
        );
    }

    let file_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder)
        .join(format!("{}.md", platform));

    if !file_path.parent().map(|p| p.exists()).unwrap_or(false) {
        return Err("Post folder not found".to_string());
    }

    let tmp_path = file_path.with_extension("md.tmp");
    fs::write(&tmp_path, new_content)
        .map_err(|e| format!("Failed to write {}: {}", tmp_path.display(), e))?;
    fs::rename(&tmp_path, &file_path)
        .map_err(|e| format!("Failed to rename to {}: {}", file_path.display(), e))?;

    Ok(())
}

#[tauri::command]
pub fn update_post_content(
    repo_path: String,
    post_folder: String,
    platform: String,
    new_content: String,
) -> Result<(), String> {
    update_post_content_impl(&repo_path, &post_folder, &platform, &new_content)
}

pub fn update_post_image_impl(
    repo_path: &str,
    post_folder: &str,
    image_url: Option<&str>,
) -> Result<(), String> {
    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err(
            "Invalid post folder: must not contain path separators or '..'".to_string(),
        );
    }

    if let Some(url) = image_url {
        if !url.starts_with("https://") {
            return Err(format!(
                "Invalid image URL: must start with https:// (got: {})",
                url
            ));
        }
        if is_private_host(url) {
            return Err("Invalid image URL: resolves to a private or reserved address".to_string());
        }
    }

    let meta_path = PathBuf::from(repo_path)
        .join(".postlane/posts")
        .join(post_folder)
        .join("meta.json");

    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let raw = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read meta.json: {}", e))?;
    let mut meta: PostMeta = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse meta.json: {}", e))?;

    meta.image_url = image_url.map(|s| s.to_string());

    let tmp_path = meta_path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&meta)
        .map_err(|e| format!("Failed to serialize meta.json: {}", e))?;
    fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write meta.json: {}", e))?;
    fs::rename(&tmp_path, &meta_path)
        .map_err(|e| format!("Failed to rename meta.json: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn update_post_image(
    repo_path: String,
    post_folder: String,
    image_url: Option<String>,
) -> Result<(), String> {
    update_post_image_impl(&repo_path, &post_folder, image_url.as_deref())
}

/// Fetches the og:image URL from a web page.
/// Returns Ok(Some(url)) if found, Ok(None) if the page has no og:image,
/// Err if the fetch fails or the URL is unsafe.
#[tauri::command]
pub async fn fetch_og_image(url: String) -> Result<Option<String>, String> {
    if !url.starts_with("https://") {
        return Err("URL must start with https://".to_string());
    }

    if is_private_host(&url) {
        return Err("URL resolves to a private or reserved address".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Postlane/1.0 (og-image-fetch)")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // lgtm[rust/non-https-url] -- unreachable for non-https: guard at top of fn returns Err first
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    if bytes.len() > 512 * 1024 {
        return Err("Response too large (max 512 KB)".to_string());
    }
    let html = String::from_utf8_lossy(&bytes).to_string();

    // Extract og:image with a simple regex — avoids pulling in a full HTML parser.
    // Also handle reversed attribute order: content before property.
    let image_url = og_regex()
        .captures(&html)
        .or_else(|| og_regex_rev().captures(&html))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string());

    // Validate the extracted URL: must be https://, not a private IP.
    if let Some(ref img_url) = image_url {
        if !img_url.starts_with("https://") || is_private_host(img_url) {
            return Ok(None);
        }
    }

    Ok(image_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_private_host ---

    #[test]
    fn test_private_host_loopback_ipv4() {
        assert!(is_private_host("https://127.0.0.1/path"));
    }

    #[test]
    fn test_private_host_loopback_ipv4_port() {
        assert!(is_private_host("https://127.0.0.1:8080/path"));
    }

    #[test]
    fn test_private_host_rfc1918_10() {
        assert!(is_private_host("https://10.0.0.1/"));
    }

    #[test]
    fn test_private_host_rfc1918_172() {
        assert!(is_private_host("https://172.20.0.1/"));
    }

    #[test]
    fn test_private_host_rfc1918_192_168() {
        assert!(is_private_host("https://192.168.1.1/"));
    }

    #[test]
    fn test_private_host_link_local() {
        assert!(is_private_host("https://169.254.169.254/latest/meta-data/"));
    }

    #[test]
    fn test_private_host_localhost_string() {
        assert!(is_private_host("https://localhost/"));
    }

    #[test]
    fn test_private_host_localhost_localdomain() {
        assert!(is_private_host("https://localhost.localdomain/"));
    }

    #[test]
    fn test_private_host_ipv6_loopback() {
        assert!(is_private_host("https://[::1]/"));
    }

    #[test]
    fn test_private_host_ipv6_unique_local() {
        assert!(is_private_host("https://[fd00::1]/"));
    }

    #[test]
    fn test_private_host_ipv6_unique_local_range_boundary() {
        assert!(is_private_host("https://[fc00::1]/"));
    }

    #[test]
    fn test_private_host_broadcast() {
        assert!(is_private_host("https://255.255.255.255/"));
    }

    #[test]
    fn test_private_host_unspecified() {
        assert!(is_private_host("https://0.0.0.0/"));
    }

    #[test]
    fn test_public_host_allowed() {
        assert!(!is_private_host("https://example.com/image.png"));
    }

    #[test]
    fn test_public_host_cdn_allowed() {
        assert!(!is_private_host("https://images.unsplash.com/photo-123"));
    }

    #[test]
    fn test_unparseable_url_rejected() {
        assert!(is_private_host("not-a-url"));
    }

    #[test]
    fn test_private_host_no_host_rejected() {
        assert!(is_private_host("file:///etc/passwd"));
    }

    // --- fetch_og_image: synchronous validation ---

    #[tokio::test]
    async fn test_fetch_og_image_rejects_http_url() {
        let result = fetch_og_image("http://example.com/page".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("https://"));
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_private_ip_direct() {
        let result = fetch_og_image("https://127.0.0.1/".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private"));
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_localhost() {
        let result = fetch_og_image("https://localhost/".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private"));
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_aws_metadata() {
        let result = fetch_og_image("https://169.254.169.254/latest/meta-data/".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private"));
    }

    // --- update_post_content_impl: path traversal ---

    #[test]
    fn test_update_content_rejects_slash_in_folder() {
        let result = update_post_content_impl("/repo", "sub/folder", "x", "content");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_content_rejects_dotdot_in_folder() {
        let result = update_post_content_impl("/repo", "../escape", "x", "content");
        assert!(result.is_err());
    }

    #[test]
    fn test_update_content_rejects_invalid_platform() {
        let result = update_post_content_impl("/repo", "valid-folder", "instagram", "content");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid platform"));
    }

    // --- update_post_image_impl: path traversal + https validation ---

    #[test]
    fn test_update_image_rejects_slash_in_folder() {
        let result = update_post_image_impl("/repo", "sub/folder", Some("https://example.com/img.png"));
        assert!(result.is_err());
    }

    #[test]
    fn test_update_image_rejects_http_url() {
        let result = update_post_image_impl("/repo", "valid", Some("http://example.com/img.png"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("https://"));
    }

    // --- update_post_image_impl: SSRF check ---

    #[test]
    fn test_update_image_rejects_private_ip() {
        let result = update_post_image_impl("/repo", "valid", Some("https://169.254.169.254/metadata"));
        assert!(result.is_err(), "link-local IP must be rejected");
        assert!(result.unwrap_err().contains("private"), "error must mention 'private'");
    }

    #[test]
    fn test_update_image_rejects_localhost() {
        let result = update_post_image_impl("/repo", "valid", Some("https://localhost/img.png"));
        assert!(result.is_err(), "localhost must be rejected");
    }

    #[test]
    fn test_update_image_rejects_rfc1918() {
        let result = update_post_image_impl("/repo", "valid", Some("https://192.168.1.1/img.png"));
        assert!(result.is_err(), "RFC-1918 IP must be rejected");
    }

    #[test]
    fn test_update_image_accepts_public_url() {
        // Will fail at meta.json lookup, but must NOT fail on SSRF check
        let result = update_post_image_impl("/nonexistent", "valid", Some("https://images.unsplash.com/photo.png"));
        // The error (if any) should NOT mention "private"
        if let Err(e) = result {
            assert!(!e.contains("private"), "public URL should not be rejected as private, got: {}", e);
        }
    }

    // --- VALID_PLATFORMS: new platform acceptance ---

    #[test]
    fn test_update_content_accepts_linkedin() {
        let result = update_post_content_impl("/repo", "valid-folder", "linkedin", "content");
        // Should not fail with "Invalid platform"
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "linkedin should be valid, got: {}", e);
        }
    }

    #[test]
    fn test_update_content_accepts_substack() {
        let result = update_post_content_impl("/repo", "valid-folder", "substack", "content");
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "substack should be valid, got: {}", e);
        }
    }

    #[test]
    fn test_update_content_accepts_product_hunt() {
        let result = update_post_content_impl("/repo", "valid-folder", "product_hunt", "content");
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "product_hunt should be valid, got: {}", e);
        }
    }

    #[test]
    fn test_update_content_accepts_show_hn() {
        let result = update_post_content_impl("/repo", "valid-folder", "show_hn", "content");
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "show_hn should be valid, got: {}", e);
        }
    }

    #[test]
    fn test_update_content_accepts_changelog() {
        let result = update_post_content_impl("/repo", "valid-folder", "changelog", "content");
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "changelog should be valid, got: {}", e);
        }
    }

    #[test]
    fn test_update_content_accepts_substack_notes() {
        let result = update_post_content_impl("/repo", "valid-folder", "substack_notes", "content");
        if let Err(e) = &result {
            assert!(!e.contains("Invalid platform"), "substack_notes should be valid, got: {}", e);
        }
    }
}
