// SPDX-License-Identifier: BUSL-1.1

use crate::types::PostMeta;
use regex::Regex;
use std::path::Path;

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

/// Counts characters for a platform, handling URL shortening
pub fn count_chars(content: &str, platform: &str) -> usize {
    let url_regex = Regex::new(r"https?://[^\s]+").unwrap();

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
        let count = count_chars(content, "x");
        // "Check out this cool link " = 25 chars + 23 (URL) = 48
        assert_eq!(count, 48);
    }

    #[test]
    fn test_mastodon_url_counting() {
        let content = "Check out this cool link https://example.com/very/long/url/that/is/definitely/more/than/twenty/three/characters";
        let count = count_chars(content, "mastodon");
        // Same as X - Mastodon also uses 23-char shortened URLs
        assert_eq!(count, 48);
    }

    #[test]
    fn test_bluesky_url_counting() {
        let content = "Check out https://example.com/very/long/url/here";
        let count = count_chars(content, "bluesky");
        // Bluesky counts full URL
        assert_eq!(count, content.chars().count());
    }

    #[test]
    fn test_read_meta_valid() {
        let dir = std::env::temp_dir().join("postlane_test_parser_valid");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let meta_json = r#"{
            "status": "draft",
            "platforms": ["x", "bluesky"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        fs::write(dir.join("meta.json"), meta_json).expect("Failed to write meta.json");

        let meta = read_meta(&dir).expect("Should parse valid meta.json");
        assert_eq!(meta.status, "draft");
        assert_eq!(meta.platforms.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_meta_malformed() {
        let dir = std::env::temp_dir().join("postlane_test_parser_malformed");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        fs::write(dir.join("meta.json"), "{ not valid json }").expect("Failed to write");

        let result = read_meta(&dir);
        assert!(result.is_err(), "Should fail on malformed JSON");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_post_folder_success() {
        let dir = std::env::temp_dir().join("postlane_test_validate_success");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let meta_json = r#"{
            "status": "ready",
            "platforms": ["x"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        fs::write(dir.join("meta.json"), meta_json).expect("Failed to write meta.json");
        fs::write(dir.join("x.md"), "Short post").expect("Failed to write x.md");

        let result = validate_post_folder(&dir);
        assert!(result.is_ok(), "Should validate successfully");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_post_folder_missing_file() {
        let dir = std::env::temp_dir().join("postlane_test_validate_missing");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let meta_json = r#"{
            "status": "ready",
            "platforms": ["x", "bluesky"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        fs::write(dir.join("meta.json"), meta_json).expect("Failed to write meta.json");
        fs::write(dir.join("x.md"), "Post content").expect("Failed to write x.md");
        // bluesky.md is missing

        let result = validate_post_folder(&dir);
        assert!(result.is_err(), "Should fail with missing file error");

        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            assert!(matches!(errors[0], ValidationError::MissingFile(_)));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_post_folder_over_limit() {
        let dir = std::env::temp_dir().join("postlane_test_validate_over");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        let meta_json = r#"{
            "status": "ready",
            "platforms": ["x"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": null,
            "platform_results": null,
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        fs::write(dir.join("meta.json"), meta_json).expect("Failed to write meta.json");
        // Write 300 characters (over X's 280 limit)
        fs::write(dir.join("x.md"), "a".repeat(300)).expect("Failed to write x.md");

        let result = validate_post_folder(&dir);
        assert!(result.is_err(), "Should fail with over-limit error");

        if let Err(errors) = result {
            assert_eq!(errors.len(), 1);
            if let ValidationError::OverLimit {
                platform,
                count,
                limit,
            } = &errors[0]
            {
                assert_eq!(platform, "x");
                assert_eq!(*count, 300);
                assert_eq!(*limit, 280);
            } else {
                panic!("Expected OverLimit error");
            }
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_partial_platform_results() {
        // This tests PostMeta deserialization with partial results - already in types.rs
        // Just verify the concept here
        let meta_json = r#"{
            "status": "sent",
            "platforms": ["x", "bluesky"],
            "schedule": null,
            "trigger": null,
            "scheduler_ids": {"x": "123"},
            "platform_results": {"x": "success"},
            "error": null,
            "image_url": null,
            "image_source": null,
            "image_attribution": null,
            "llm_model": null,
            "created_at": null,
            "sent_at": null
        }"#;

        let meta: PostMeta = serde_json::from_str(meta_json).expect("Should parse");
        assert_eq!(meta.platform_results.unwrap().len(), 1);
    }

    #[test]
    fn test_unknown_platform_url_counting() {
        // Unknown platforms should fall through to default case (full character count)
        let content = "Check out https://example.com/very/long/url";
        let count = count_chars(content, "unknown-platform");
        // Should count full length including full URL
        assert_eq!(count, content.chars().count());
    }

    #[test]
    fn test_count_chars_linkedin_explicit_arm() {
        // count_chars("linkedin") must have an explicit arm, not rely on the wildcard
        // fallthrough. Verify via a test that would break if the wildcard changed behaviour.
        // A 50-char URL must count as 50, not 23 (which x/mastodon would give).
        let url = format!("https://example.com/{}", "a".repeat(30)); // 50 chars
        assert_eq!(count_chars(&url, "linkedin"), 50, "linkedin URL must not be shortened");
        // And must equal count_linkedin_chars directly.
        assert_eq!(count_chars(&url, "linkedin"), count_linkedin_chars(&url));
    }

    // --- 8.2 LinkedIn character counting ---

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
        // LinkedIn counts every character including URLs — no t.co-style shortening
        let url = format!("https://example.com/{}", "a".repeat(30)); // 50-char URL
        assert_eq!(count_linkedin_chars(&url), 50);
    }

    #[test]
    fn test_linkedin_url_not_collapsed_to_23() {
        let content = format!("Check out {}", format!("https://example.com/{}", "a".repeat(30)));
        // "Check out " = 10 chars + 50-char URL = 60 total; must NOT be 10 + 23 = 33
        assert_eq!(count_linkedin_chars(&content), 60);
    }

    #[test]
    fn test_linkedin_multibyte_unicode_counts_as_one() {
        // Each Unicode scalar counts as 1 character
        assert_eq!(count_linkedin_chars("café"), 4);
    }

    #[test]
    fn test_linkedin_3000_chars_at_limit() {
        let content = "a".repeat(3000);
        assert_eq!(count_linkedin_chars(&content), 3000);
    }

    #[test]
    fn test_read_meta_missing_file() {
        let dir = std::env::temp_dir().join("postlane_test_parser_missing_meta");
        fs::create_dir_all(&dir).expect("Failed to create test dir");

        // No meta.json file exists
        let result = read_meta(&dir);

        assert!(result.is_err(), "Should fail when meta.json is missing");
        if let Err(ValidationError::ParseError(msg)) = result {
            assert!(msg.contains("Failed to read meta.json"), "Error should mention read failure");
        } else {
            panic!("Expected ParseError");
        }

        let _ = fs::remove_dir_all(&dir);
    }
}
