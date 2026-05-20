// SPDX-License-Identifier: BUSL-1.1

use std::fs;
use std::path::{Path, PathBuf};
use crate::post_meta::PostMeta;

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
        if crate::security::ssrf_check::is_private_url(url) {
            return Err("Invalid image URL: resolves to a private or reserved address".to_string());
        }
    }

    let meta_path = PostMeta::path_for(Path::new(repo_path), post_folder);

    if !meta_path.exists() {
        return Err("meta.json not found in post folder".to_string());
    }

    let raw = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read {}: {}", meta_path.display(), e))?;
    let mut v: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse {}: {}", meta_path.display(), e))?;
    let obj = v.as_object_mut()
        .ok_or_else(|| format!("meta.json is not a JSON object: {}", meta_path.display()))?;
    match image_url {
        Some(url) => { obj.insert("image_url".to_string(), serde_json::json!(url)); }
        None => { obj.remove("image_url"); }
    }
    let json = serde_json::to_string_pretty(&v)
        .map_err(|e| format!("Failed to serialise: {}", e))?;
    let tmp_path = meta_path.with_extension("json.tmp");
    fs::write(&tmp_path, &json)
        .map_err(|e| format!("Failed to write {}: {}", tmp_path.display(), e))?;
    fs::rename(&tmp_path, &meta_path)
        .map_err(|e| format!("Failed to rename to {}: {}", meta_path.display(), e))
}

#[tauri::command]
pub fn update_post_image(
    repo_path: String,
    post_folder: String,
    image_url: Option<String>,
) -> Result<(), String> {
    update_post_image_impl(&repo_path, &post_folder, image_url.as_deref())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- is_direct_image_url ---

    #[test]
    fn test_is_direct_image_url_cdn_hostname() {
        assert!(is_direct_image_url("https://images.unsplash.com/photo-12345"));
    }

    #[test]
    fn test_is_direct_image_url_pixabay() {
        assert!(is_direct_image_url("https://cdn.pixabay.com/photo/xyz"));
    }

    #[test]
    fn test_is_direct_image_url_jpg_extension() {
        assert!(is_direct_image_url("https://example.com/photo.jpg"));
    }

    #[test]
    fn test_is_direct_image_url_png_extension() {
        assert!(is_direct_image_url("https://example.com/image.png"));
    }

    #[test]
    fn test_is_direct_image_url_webp() {
        assert!(is_direct_image_url("https://example.com/image.webp"));
    }

    #[test]
    fn test_is_direct_image_url_html_page() {
        assert!(!is_direct_image_url("https://example.com/page"));
    }

    #[test]
    fn test_is_direct_image_url_extension_in_query_string_only() {
        // Extension only in query string — path has no image extension, must return false.
        assert!(!is_direct_image_url("https://example.com/page?file=photo.jpg"));
    }

    #[test]
    fn test_is_direct_image_url_invalid_url() {
        assert!(!is_direct_image_url("not a url"));
    }

    // --- update_post_content_impl: happy path ---

    #[test]
    fn test_update_content_writes_file_atomically() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        let result = update_post_content_impl(
            dir.path().to_str().expect("valid path"),
            "my-post",
            "x",
            "hello world",
        );
        assert!(result.is_ok(), "write should succeed: {:?}", result);
        let written = fs::read_to_string(post_dir.join("x.md")).expect("x.md must exist");
        assert_eq!(written, "hello world");
    }

    // --- update_post_image_impl: happy paths ---

    #[test]
    fn test_update_image_sets_image_url_in_meta() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(post_dir.join("meta.json"), r#"{"status":"ready"}"#).expect("write meta");
        let result = update_post_image_impl(
            dir.path().to_str().expect("valid path"),
            "my-post",
            Some("https://images.unsplash.com/photo-12345"),
        );
        assert!(result.is_ok(), "update should succeed: {:?}", result);
        let raw = fs::read_to_string(post_dir.join("meta.json")).expect("read meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        assert_eq!(
            v["image_url"].as_str(),
            Some("https://images.unsplash.com/photo-12345"),
            "image_url must be set in meta.json"
        );
    }

    #[test]
    fn test_update_image_clears_image_url_when_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(
            post_dir.join("meta.json"),
            r#"{"status":"ready","image_url":"https://images.unsplash.com/old"}"#,
        )
        .expect("write meta");
        let result = update_post_image_impl(
            dir.path().to_str().expect("valid path"),
            "my-post",
            None,
        );
        assert!(result.is_ok(), "clear should succeed: {:?}", result);
        let raw = fs::read_to_string(post_dir.join("meta.json")).expect("read meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        assert!(v.get("image_url").is_none(), "image_url must be removed from meta.json");
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
