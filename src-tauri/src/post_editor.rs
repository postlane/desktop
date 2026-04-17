// SPDX-License-Identifier: BUSL-1.1

use std::path::PathBuf;
use std::fs;
use crate::types::PostMeta;

const VALID_PLATFORMS: &[&str] = &["x", "bluesky", "mastodon"];

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
        return Err(format!(
            "Post folder not found: {}",
            file_path.parent().unwrap_or(&file_path).display()
        ));
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

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Postlane/1.0 (og-image-fetch)")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    let html = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Extract og:image with a simple regex — avoids pulling in a full HTML parser.
    let og_regex = regex::Regex::new(
        r#"<meta[^>]+property=["']og:image["'][^>]+content=["']([^"']+)["']"#
    ).map_err(|e| format!("Regex error: {}", e))?;

    // Also handle reversed attribute order: content before property.
    let og_regex_rev = regex::Regex::new(
        r#"<meta[^>]+content=["']([^"']+)["'][^>]+property=["']og:image["']"#
    ).map_err(|e| format!("Regex error: {}", e))?;

    let image_url = og_regex
        .captures(&html)
        .or_else(|| og_regex_rev.captures(&html))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string());

    // Validate the extracted URL: must be https://, not a private IP.
    if let Some(ref img_url) = image_url {
        if !img_url.starts_with("https://") {
            return Ok(None);
        }
    }

    Ok(image_url)
}
