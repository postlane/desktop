// SPDX-License-Identifier: BUSL-1.1

use crate::types::PostMeta;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    MissingFile(String),
    OverLimit {
        platform: String,
        count: usize,
        limit: usize,
    },
    ParseError(String),
}

/// Reads meta.json from a post folder
pub fn read_meta(folder: &Path) -> Result<PostMeta, ValidationError> {
    let meta_path = folder.join("meta.json");

    let content = std::fs::read_to_string(&meta_path).map_err(|e| {
        ValidationError::ParseError(format!(
            "Failed to read meta.json at {:?}: {}",
            folder, e
        ))
    })?;

    serde_json::from_str(&content).map_err(|e| {
        ValidationError::ParseError(format!("Failed to parse meta.json at {:?}: {}", folder, e))
    })
}

/// Returns character limit for a platform
pub fn char_limit(platform: &str) -> Result<usize, ValidationError> {
    match platform {
        "x" => Ok(280),
        "bluesky" => Ok(300),
        "mastodon" => Ok(500),
        "linkedin" => Ok(3000),
        "substack_notes" => Ok(300),
        _ => Err(ValidationError::ParseError(format!(
            "Unknown platform: {}",
            platform
        ))),
    }
}

/// Counts characters for LinkedIn: every character at full length, no URL collapsing.
/// LinkedIn does not apply t.co-style URL shortening — URLs count at their true length.
pub fn count_linkedin_chars(content: &str) -> usize {
    content.chars().count()
}

static URL_REGEX: OnceLock<Regex> = OnceLock::new();

fn url_regex() -> &'static Regex {
    URL_REGEX.get_or_init(|| Regex::new(r"https?://[^\s]+").expect("valid hardcoded regex"))
}

/// Counts characters for a platform, handling URL shortening
pub fn count_chars(content: &str, platform: &str) -> usize {
    let url_regex = url_regex();

    match platform {
        "x" | "mastodon" => {
            // X and Mastodon replace URLs with 23-char t.co links
            let mut result = content.to_string();
            for url_match in url_regex.find_iter(content) {
                let url = url_match.as_str();
                result = result.replacen(url, &"x".repeat(23), 1);
            }
            result.chars().count()
        }
        "bluesky" => {
            // Bluesky counts full URL length
            content.chars().count()
        }
        "linkedin" | "substack_notes" => count_linkedin_chars(content),
        _ => content.chars().count(),
    }
}

/// Validates a post folder
pub fn validate_post_folder(folder: &Path) -> Result<PostMeta, Vec<ValidationError>> {
    let meta = read_meta(folder).map_err(|e| vec![e])?;

    let mut errors = Vec::new();

    for platform in &meta.platforms {
        // Check platform file exists
        let platform_file = folder.join(format!("{}.md", platform));
        if !platform_file.exists() {
            errors.push(ValidationError::MissingFile(format!(
                "{}.md not found in {:?}",
                platform, folder
            )));
            continue;
        }

        // Check character limit
        if let Ok(content) = std::fs::read_to_string(&platform_file) {
            let count = count_chars(&content, platform);
            if let Ok(limit) = char_limit(platform) {
                if count > limit {
                    errors.push(ValidationError::OverLimit {
                        platform: platform.clone(),
                        count,
                        limit,
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(meta)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn meta_json(status: &str, platforms: &[&str]) -> String {
        let p: Vec<String> = platforms.iter().map(|s| format!("\"{}\"", s)).collect();
        format!(
            r#"{{"status":"{}","platforms":[{}],"schedule":null,"trigger":null,"scheduler_ids":null,"platform_results":null,"error":null,"image_url":null,"image_source":null,"image_attribution":null,"llm_model":null,"created_at":null,"sent_at":null}}"#,
            status, p.join(",")
        )
    }

    #[test]
    fn test_char_limits() {
        assert_eq!(char_limit("x").unwrap(), 280);
        assert_eq!(char_limit("bluesky").unwrap(), 300);
        assert_eq!(char_limit("mastodon").unwrap(), 500);
        assert!(char_limit("unknown").is_err());
    }

    #[test]
    fn test_x_url_counting() {
        let content = "Check out this cool link https://example.com/very/long/url/that/is/definitely/more/than/twenty/three/characters";
        assert_eq!(count_chars(content, "x"), 48);
    }

    #[test]
    fn test_mastodon_url_counting() {
        let content = "Check out this cool link https://example.com/very/long/url/that/is/definitely/more/than/twenty/three/characters";
        assert_eq!(count_chars(content, "mastodon"), 48);
    }

    #[test]
    fn test_bluesky_url_counting() {
        let content = "Check out https://example.com/very/long/url/here";
        assert_eq!(count_chars(content, "bluesky"), content.chars().count());
    }

    #[test]
    fn test_read_meta_valid() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("meta.json"), meta_json("draft", &["x", "bluesky"]))
            .expect("write meta.json");
        let meta = read_meta(dir.path()).expect("parse valid meta.json");
        assert_eq!(meta.status, "draft");
        assert_eq!(meta.platforms.len(), 2);
    }

    #[test]
    fn test_read_meta_malformed() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("meta.json"), "{ not valid json }").expect("write");
        assert!(read_meta(dir.path()).is_err());
    }

    #[test]
    fn test_validate_post_folder_success() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("meta.json"), meta_json("ready", &["x"])).expect("write meta");
        fs::write(dir.path().join("x.md"), "Short post").expect("write x.md");
        assert!(validate_post_folder(dir.path()).is_ok());
    }

    #[test]
    fn test_validate_post_folder_missing_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("meta.json"), meta_json("ready", &["x", "bluesky"]))
            .expect("write meta");
        fs::write(dir.path().join("x.md"), "Post content").expect("write x.md");
        let result = validate_post_folder(dir.path());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::MissingFile(_)));
    }

    #[test]
    fn test_validate_post_folder_over_limit() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        fs::write(dir.path().join("meta.json"), meta_json("ready", &["x"])).expect("write meta");
        fs::write(dir.path().join("x.md"), "a".repeat(300)).expect("write x.md");
        let result = validate_post_folder(dir.path());
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        if let ValidationError::OverLimit { platform, count, limit } = &errors[0] {
            assert_eq!(platform, "x");
            assert_eq!(*count, 300);
            assert_eq!(*limit, 280);
        } else {
            panic!("Expected OverLimit error");
        }
    }

    #[test]
    fn test_url_regex_initialised_via_once_lock() {
        let content = "See https://example.com/path for details";
        let first = count_chars(content, "x");
        let second = count_chars(content, "x");
        assert_eq!(first, second);
        assert_eq!(first, 39);
    }

    #[test]
    fn test_unknown_platform_url_counting() {
        let content = "Check out https://example.com/very/long/url";
        assert_eq!(count_chars(content, "unknown-platform"), content.chars().count());
    }

    #[test]
    fn test_count_chars_linkedin_explicit_arm() {
        let url = format!("https://example.com/{}", "a".repeat(30));
        assert_eq!(count_chars(&url, "linkedin"), 50, "linkedin URL must not be shortened");
        assert_eq!(count_chars(&url, "linkedin"), count_linkedin_chars(&url));
    }

    #[test]
    fn test_linkedin_char_limit() {
        assert_eq!(char_limit("linkedin").unwrap(), 3000);
    }

    #[test]
    fn test_substack_notes_char_limit() {
        assert_eq!(char_limit("substack_notes").unwrap(), 300);
    }

    #[test]
    fn test_linkedin_url_counted_at_full_length() {
        let url = format!("https://example.com/{}", "a".repeat(30));
        assert_eq!(count_linkedin_chars(&url), 50);
    }

    #[test]
    fn test_linkedin_url_not_collapsed_to_23() {
        let content = format!("Check out https://example.com/{}", "a".repeat(30));
        assert_eq!(count_linkedin_chars(&content), 60);
    }

    #[test]
    fn test_linkedin_multibyte_unicode_counts_as_one() {
        assert_eq!(count_linkedin_chars("café"), 4);
    }

    #[test]
    fn test_linkedin_3000_chars_at_limit() {
        assert_eq!(count_linkedin_chars(&"a".repeat(3000)), 3000);
    }

    #[test]
    fn test_read_meta_missing_file() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let result = read_meta(dir.path());
        assert!(result.is_err());
        if let Err(ValidationError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to read meta.json"));
        } else {
            panic!("Expected ParseError");
        }
    }
}
