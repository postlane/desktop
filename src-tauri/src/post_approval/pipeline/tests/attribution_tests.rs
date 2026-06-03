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
    assert!(result.contains("Photo by Jane Doe"));
    assert!(result.contains("on Unsplash"));
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

// pipeline — attribution URLs must not be wrapped in () so URL detectors on Bluesky / Upload Post
// cannot greedily consume the trailing ) as part of the URL, dropping it from the displayed text.
#[test]
fn test_attribution_urls_not_wrapped_in_parens() {
    let meta = meta_with_attribution();
    let result = append_unsplash_attribution("Hello world", &meta);
    // Neither URL should be immediately followed by )
    assert!(
        !result.contains("utm_source=postlane&utm_medium=referral)"),
        "UTM URL must not be immediately followed by ): {:?}",
        result,
    );
}

// pipeline — attribution includes photographer URL and Unsplash base URL with UTM params
#[test]
fn test_attribution_full_format_with_utm_links() {
    let meta = meta_with_attribution();
    let result = append_unsplash_attribution("Hello world", &meta);
    let expected = "Hello world\n\nPhoto by Jane Doe https://unsplash.com/@janedoe?utm_source=postlane&utm_medium=referral on Unsplash https://unsplash.com/?utm_source=postlane&utm_medium=referral";
    assert_eq!(result, expected);
}

// pipeline — empty photographer_url falls back to name-only attribution
#[test]
fn test_attribution_with_empty_photographer_url_shows_name_only() {
    use crate::post_meta::ImageAttribution;
    let meta = PostMeta {
        image_source: Some("unsplash".to_string()),
        image_attribution: Some(ImageAttribution {
            photographer_name: "Jane Doe".to_string(),
            photographer_url: String::new(),
        }),
        ..PostMeta::default()
    };
    let result = append_unsplash_attribution("Hello world", &meta);
    let expected = "Hello world\n\nPhoto by Jane Doe on Unsplash https://unsplash.com/?utm_source=postlane&utm_medium=referral";
    assert_eq!(result, expected);
}

// pipeline — unsplash source + attribution present but photographer_name empty → no attribution appended
#[test]
fn test_no_attribution_when_photographer_name_is_empty() {
    use crate::post_meta::ImageAttribution;
    let meta = PostMeta {
        image_source: Some("unsplash".to_string()),
        image_attribution: Some(ImageAttribution {
            photographer_name: "".to_string(),
            photographer_url: "https://unsplash.com/@user".to_string(),
        }),
        ..PostMeta::default()
    };
    let result = append_unsplash_attribution("Hello world", &meta);
    assert_eq!(
        result, "Hello world",
        "empty photographer_name must not append any attribution text"
    );
}
