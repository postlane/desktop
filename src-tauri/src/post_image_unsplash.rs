// SPDX-License-Identifier: BUSL-1.1

use std::path::Path;
use crate::app_state::AppState;
use crate::post_approval::pipeline::post_location::{PostLocation, validate_repo_path};
use crate::post_meta::{ImageAttribution, PostMeta};

pub fn update_post_image_unsplash_impl(
    repo_path: &str,
    post_folder: &str,
    image_url: &str,
    download_location: &str,
    photographer_name: &str,
    photographer_url: &str,
    state: &AppState,
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
    if crate::ssrf_validation::is_private_url(image_url) {
        return Err("Invalid image URL: resolves to a private or reserved address".to_string());
    }
    if !download_location.starts_with("https://api.unsplash.com/") {
        return Err(format!(
            "Invalid download_location: must start with https://api.unsplash.com/ (got: {})",
            download_location
        ));
    }
    // Workspace children first; fall back to legacy path for repos.repos[] entries.
    let meta_path = match validate_repo_path(repo_path, state) {
        Ok(PostLocation::Workspace { workspace_path, posts_dir, .. }) => {
            Path::new(&workspace_path).join("posts").join(&posts_dir).join(post_folder).join("meta.json")
        }
        _ => PostMeta::path_for(Path::new(repo_path), post_folder),
    };
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
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    update_post_image_unsplash_impl(
        &repo_path,
        &post_folder,
        &image_url,
        &download_location,
        &photographer_name,
        &photographer_url,
        &state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{make_state, make_repo};
    use std::fs;

    fn call(
        repo: &str,
        folder: &str,
        url: &str,
        dl: &str,
        name: &str,
        purl: &str,
    ) -> Result<(), String> {
        let state = make_state(vec![make_repo("r1", repo)]);
        update_post_image_unsplash_impl(repo, folder, url, dl, name, purl, &state)
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

    /// 22.10.5/B12 — update_post_image_unsplash must write to workspace posts path.
    #[test]
    fn test_update_unsplash_accepts_workspace_child_and_writes_to_workspace_path() {
        use crate::storage::ReposConfig;
        use crate::workspace_entry::WorkspaceEntry;
        use crate::workspace_repos::{RepoEntry, WorkspaceReposConfig, write_workspace_repos};

        let ws = tempfile::TempDir::new().expect("create ws dir");
        let child = ws.path().join("my-repo");
        fs::create_dir_all(&child).expect("create child dir");
        let canonical_child = fs::canonicalize(&child).expect("canonicalize child");
        let canonical_ws = fs::canonicalize(ws.path()).expect("canonicalize ws");

        let ws_repos = WorkspaceReposConfig {
            version: 1,
            repos: vec![RepoEntry {
                id: "child-id".to_string(),
                name: "my-repo".to_string(),
                path: canonical_child.to_str().unwrap().to_string(),
                posts_dir: "my-repo".to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };
        write_workspace_repos(&canonical_ws.join("repos.json"), &ws_repos).expect("write ws repos");

        let post_dir = canonical_ws.join("posts").join("my-repo").join("unsplash-post");
        fs::create_dir_all(&post_dir).expect("create post dir");
        fs::write(post_dir.join("meta.json"), r#"{"status":"ready"}"#).expect("write meta");

        let config = ReposConfig {
            version: 2,
            workspaces: vec![WorkspaceEntry {
                id: "ws-proj".to_string(),
                name: "my-ws".to_string(),
                workspace_path: canonical_ws.to_str().unwrap().to_string(),
                active: true,
                added_at: "2026-01-01T00:00:00Z".to_string(),
            }],
            repos: vec![],
        };
        let repos_path = std::env::temp_dir().join(format!("unsplash_ws_{}.json", std::process::id()));
        let state = crate::app_state::AppState::new_with_path(config, repos_path);

        let result = update_post_image_unsplash_impl(
            canonical_child.to_str().unwrap(),
            "unsplash-post",
            "https://images.unsplash.com/photo-abc",
            "https://api.unsplash.com/photos/abc/download",
            "Jane Doe",
            "https://unsplash.com/@janedoe",
            &state,
        );
        assert!(result.is_ok(), "update_post_image_unsplash must accept workspace child: {:?}", result);
        let raw = fs::read_to_string(post_dir.join("meta.json")).expect("read meta");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(v["image_url"].as_str(), Some("https://images.unsplash.com/photo-abc"));
        assert_eq!(v["image_source"].as_str(), Some("unsplash"));

        let legacy = canonical_child.join(".postlane/posts/unsplash-post/meta.json");
        assert!(!legacy.exists(), "must NOT write to legacy child path");
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
