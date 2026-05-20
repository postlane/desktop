// SPDX-License-Identifier: BUSL-1.1

/// Parses the `Link` header and returns the URL for `rel="next"`, if present.
///
/// Mastodon pagination uses RFC 5988 link headers:
/// `<https://host/path?max_id=123>; rel="next"`
pub(super) fn parse_link_next(headers: &reqwest::header::HeaderMap) -> Option<String> {
    let link = headers.get("link")?.to_str().ok()?;
    for part in link.split(',') {
        let part = part.trim();
        if part.contains(r#"rel="next""#) {
            if let Some(url) = part.split(';').next() {
                let url = url.trim().trim_start_matches('<').trim_end_matches('>');
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }
    None
}

/// Maps a Mastodon scheduled_status JSON object to a `QueuedPost`.
pub(super) fn map_scheduled_status(item: &serde_json::Value) -> Option<crate::types::QueuedPost> {
    let post_id = item["id"].as_str()?.to_string();
    let scheduled_for = item["scheduled_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))?;

    let text = item["params"]["text"].as_str().unwrap_or("");
    let content_preview = if text.chars().count() > 80 {
        let truncated: String = text.chars().take(80).collect();
        format!("{}...", truncated)
    } else {
        text.to_string()
    };

    Some(crate::types::QueuedPost {
        post_id,
        platform: "mastodon".to_string(),
        scheduled_for,
        content_preview,
    })
}
