// SPDX-License-Identifier: BUSL-1.1

use std::path::Path;
use crate::post_meta::{ImageAttribution, PostMeta};

pub fn update_post_image_unsplash_impl(
    repo_path: &str,
    post_folder: &str,
    image_url: &str,
    download_location: &str,
    photographer_name: &str,
    photographer_url: &str,
) -> Result<(), String> {
    if post_folder.contains('/') || post_folder.contains('\\') || post_folder.contains("..") {
        return Err(
            "Invalid post folder: must not contain path separators or '..'".to_string(),
        );
    }
    if !image_url.starts_with("https://") {
        return Err(format!(
            "Invalid image URL: must start with https:// (got: {})",
            image_url
        ));
    }
    if crate::security::ssrf_check::is_private_url(image_url) {
        return Err("Invalid image URL: resolves to a private or reserved address".to_string());
    }
    if !download_location.starts_with("https://api.unsplash.com/") {
        return Err(format!(
            "Invalid download_location: must start with https://api.unsplash.com/ (got: {})",
            download_location
        ));
    }
    let meta_path = PostMeta::path_for(Path::new(repo_path), post_folder);
    let mut meta = PostMeta::load(&meta_path)?;
    meta.image_url = Some(image_url.to_string());
    meta.image_download_location = Some(download_location.to_string());
    meta.image_source = Some("unsplash".to_string());
    meta.image_attribution = Some(ImageAttribution {
        photographer_name: photographer_name.to_string(),
        photographer_url: photographer_url.to_string(),
    });
    meta.save(&meta_path)
}

#[tauri::command]
pub fn update_post_image_unsplash(
    repo_path: String,
    post_folder: String,
    image_url: String,
    download_location: String,
    photographer_name: String,
    photographer_url: String,
) -> Result<(), String> {
    update_post_image_unsplash_impl(
        &repo_path,
        &post_folder,
        &image_url,
        &download_location,
        &photographer_name,
        &photographer_url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn call(
        repo: &str,
        folder: &str,
        url: &str,
        dl: &str,
        name: &str,
        purl: &str,
    ) -> Result<(), String> {
        update_post_image_unsplash_impl(repo, folder, url, dl, name, purl)
    }

    // 21.8.18 — writes image_url, image_download_location, image_source, image_attribution
    #[test]
    fn test_update_unsplash_writes_all_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(post_dir.join("meta.json"), r#"{"status":"ready"}"#).expect("write meta");
        let result = call(
            dir.path().to_str().expect("valid path"),
            "my-post",
            "https://images.unsplash.com/photo-abc",
            "https://api.unsplash.com/photos/abc/download?ixid=xyz",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_ok(), "unsplash update should succeed: {:?}", result);
        let raw = fs::read_to_string(post_dir.join("meta.json")).expect("read meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        assert_eq!(v["image_url"].as_str(), Some("https://images.unsplash.com/photo-abc"));
        assert_eq!(
            v["image_download_location"].as_str(),
            Some("https://api.unsplash.com/photos/abc/download?ixid=xyz")
        );
        assert_eq!(v["image_source"].as_str(), Some("unsplash"));
        assert_eq!(v["image_attribution"]["photographer_name"].as_str(), Some("Jane Doe"));
        assert_eq!(
            v["image_attribution"]["photographer_url"].as_str(),
            Some("https://unsplash.com/@janedoe")
        );
    }

    #[test]
    fn test_update_unsplash_preserves_existing_fields() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let post_dir = dir.path().join(".postlane/posts/my-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(
            post_dir.join("meta.json"),
            r#"{"status":"ready","trigger":"my trigger"}"#,
        )
        .expect("write meta");
        let result = call(
            dir.path().to_str().expect("valid path"),
            "my-post",
            "https://images.unsplash.com/photo-abc",
            "https://api.unsplash.com/photos/abc/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_ok());
        let raw = fs::read_to_string(post_dir.join("meta.json")).expect("read meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse meta");
        assert_eq!(v["trigger"].as_str(), Some("my trigger"), "pre-existing non-PostMeta fields must survive");
    }

    #[test]
    fn test_update_unsplash_rejects_http_image_url() {
        let result = call(
            "/repo",
            "my-post",
            "http://images.unsplash.com/photo-abc",
            "https://api.unsplash.com/photos/abc/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("https://"));
    }

    #[test]
    fn test_update_unsplash_rejects_non_unsplash_download_location() {
        let result = call(
            "/repo",
            "my-post",
            "https://images.unsplash.com/photo-abc",
            "https://evil.com/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("api.unsplash.com"));
    }

    #[test]
    fn test_update_unsplash_rejects_path_traversal_in_folder() {
        let result = call(
            "/repo",
            "../escape",
            "https://images.unsplash.com/photo-abc",
            "https://api.unsplash.com/photos/abc/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_update_unsplash_rejects_private_ip_image_url() {
        let result = call(
            "/repo",
            "my-post",
            "https://169.254.169.254/metadata",
            "https://api.unsplash.com/photos/abc/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
        );
        assert!(result.is_err(), "link-local IP must be rejected");
        assert!(result.unwrap_err().contains("private"), "error must mention 'private'");
    }
}
