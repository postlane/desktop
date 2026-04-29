// SPDX-License-Identifier: BUSL-1.1

use crate::types::PostMeta;
use std::fs;
use std::path::Path;

pub(crate) fn read_post_meta(meta_path: &Path) -> Result<PostMeta, String> {
    let content = fs::read_to_string(meta_path)
        .map_err(|e| format!("Failed to read {}: {}", meta_path.display(), e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", meta_path.display(), e))
}

pub(crate) fn write_post_meta(meta_path: &Path, meta: &PostMeta) -> Result<(), String> {
    let temp_path = meta_path.with_extension("json.tmp");
    let json_content = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("Failed to serialize {}: {}", meta_path.display(), e))?;
    fs::write(&temp_path, &json_content)
        .map_err(|e| format!("Failed to write {}: {}", temp_path.display(), e))?;
    fs::rename(&temp_path, meta_path)
        .map_err(|e| format!("Failed to rename to {}: {}", meta_path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PostMeta;
    use std::fs;
    use std::path::Path;

    fn ready_meta_json() -> &'static str {
        r#"{"status":"ready","platforms":["x"]}"#
    }

    #[test]
    fn read_post_meta_returns_parsed_meta() {
        let dir = std::env::temp_dir().join("postlane_test_rpm_read");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.json");
        fs::write(&path, ready_meta_json()).unwrap();
        let meta = read_post_meta(&path).expect("should parse");
        assert_eq!(meta.status, "ready");
        assert_eq!(meta.platforms, vec!["x"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_post_meta_errors_on_missing_file() {
        let result = read_post_meta(Path::new("/nonexistent/postlane_meta.json"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read"));
    }

    #[test]
    fn read_post_meta_errors_on_malformed_json() {
        let dir = std::env::temp_dir().join("postlane_test_rpm_malformed");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.json");
        fs::write(&path, "{ not valid json }").unwrap();
        let result = read_post_meta(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_post_meta_creates_file_and_cleans_up_tmp() {
        let dir = std::env::temp_dir().join("postlane_test_wpm_roundtrip");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.json");
        let meta: PostMeta = serde_json::from_str(ready_meta_json()).unwrap();
        write_post_meta(&path, &meta).expect("write should succeed");
        assert!(path.exists(), "meta.json must be created");
        assert!(!dir.join("meta.json.tmp").exists(), "tmp file must not remain");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_post_meta_roundtrips_status() {
        let dir = std::env::temp_dir().join("postlane_test_wpm_status");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meta.json");
        let mut meta: PostMeta = serde_json::from_str(ready_meta_json()).unwrap();
        meta.status = "dismissed".to_string();
        write_post_meta(&path, &meta).expect("ok");
        let read_back = read_post_meta(&path).expect("read back");
        assert_eq!(read_back.status, "dismissed");
        let _ = fs::remove_dir_all(&dir);
    }
}
