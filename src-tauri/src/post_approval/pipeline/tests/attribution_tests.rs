// SPDX-License-Identifier: BUSL-1.1
// Tests for append_unsplash_attribution — appending photo credit to post content.
use super::super::*;
use crate::post_meta::{ImageAttribution, PostMeta};

fn meta_with_attribution() -> PostMeta {
    PostMeta {
        image_source: Some("unsplash".to_string()),
        image_attribution: Some(ImageAttribution {
            photographer_name: "Jane Doe".to_string(),
            photographer_url: "https://unsplash.com/@janedoe".to_string(),
        }),
        ..PostMeta::default()
    }
}

#[test]
fn test_unsplash_attribution_appended_to_content() {
    let meta = meta_with_attribution();
    let result = append_unsplash_attribution("Hello world", &meta);
    assert!(result.starts_with("Hello world"));
    assert!(result.contains("Photo by Jane Doe on Unsplash"));
}

#[test]
fn test_no_attribution_when_source_is_not_unsplash() {
    let meta = PostMeta {
        image_source: Some("other".to_string()),
        image_attribution: Some(ImageAttribution {
            photographer_name: "Jane Doe".to_string(),
            photographer_url: "https://unsplash.com/@janedoe".to_string(),
        }),
        ..PostMeta::default()
    };
    let result = append_unsplash_attribution("Hello world", &meta);
    assert_eq!(result, "Hello world");
}

#[test]
fn test_no_attribution_when_attribution_data_missing() {
    let meta = PostMeta {
        image_source: Some("unsplash".to_string()),
        image_attribution: None,
        ..PostMeta::default()
    };
    let result = append_unsplash_attribution("Hello world", &meta);
    assert_eq!(result, "Hello world");
}
