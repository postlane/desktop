// SPDX-License-Identifier: BUSL-1.1

use std::{cmp::Ordering, fs, path::Path};

/// Collect posts from a directory by applying `parse` to each entry path.
/// Returns an empty vec if the directory cannot be read.
pub(crate) fn collect_posts_from_dir<T>(
    posts_dir: &Path,
    parse: impl Fn(&Path) -> Option<T>,
) -> Vec<T> {
    match fs::read_dir(posts_dir) {
        Ok(entries) => entries.flatten().filter_map(|e| parse(&e.path())).collect(),
        Err(_) => vec![],
    }
}

/// Sort a slice so that items with `priority_status` come before items with `other_status`,
/// then by `get_timestamp` descending within each group.
pub(crate) fn sort_by_status_priority_then_timestamp<T>(
    items: &mut [T],
    priority_status: &str,
    other_status: &str,
    get_status: impl Fn(&T) -> &str,
    get_timestamp: impl Fn(&T) -> Option<&str>,
) {
    items.sort_by(|a, b| {
        let sa = get_status(a);
        let sb = get_status(b);
        if sa == priority_status && sb == other_status {
            return Ordering::Less;
        }
        if sa == other_status && sb == priority_status {
            return Ordering::Greater;
        }
        compare_timestamps_desc(get_timestamp(b), get_timestamp(a))
    });
}

fn compare_timestamps_desc(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(ta), Some(tb)) => ta.cmp(tb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // --- collect_posts_from_dir ---

    #[test]
    fn collect_posts_from_dir_returns_empty_for_missing_dir() {
        let result = collect_posts_from_dir(Path::new("/nonexistent/path/xyz"), |_| Some(1u32));
        assert!(result.is_empty());
    }

    #[test]
    fn collect_posts_from_dir_returns_parsed_entries() {
        let dir = std::env::temp_dir().join("postlane_test_collect_posts_dir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "").unwrap();
        fs::write(dir.join("b.txt"), "").unwrap();

        let mut results: Vec<String> = collect_posts_from_dir(&dir, |p| {
            p.file_name().and_then(|n| n.to_str()).map(String::from)
        });
        results.sort();

        assert_eq!(results, vec!["a.txt", "b.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn collect_posts_from_dir_skips_entries_returning_none() {
        let dir = std::env::temp_dir().join("postlane_test_collect_posts_skip");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep.txt"), "").unwrap();
        fs::write(dir.join("skip.txt"), "").unwrap();

        let results: Vec<String> = collect_posts_from_dir(&dir, |p| {
            let name = p.file_name()?.to_str()?.to_string();
            if name.starts_with("skip") { None } else { Some(name) }
        });

        assert_eq!(results, vec!["keep.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    // --- sort_by_status_priority_then_timestamp ---

    #[derive(Debug, PartialEq)]
    struct Item { status: String, ts: Option<String> }

    fn item(status: &str, ts: Option<&str>) -> Item {
        Item { status: status.into(), ts: ts.map(String::from) }
    }

    #[test]
    fn priority_status_comes_before_other_status() {
        let mut items = vec![item("ready", Some("2024-01-02")), item("failed", Some("2024-01-01"))];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert_eq!(items[0].status, "failed");
        assert_eq!(items[1].status, "ready");
    }

    #[test]
    fn same_status_sorted_by_timestamp_descending() {
        let mut items = vec![
            item("ready", Some("2024-01-01")),
            item("ready", Some("2024-01-03")),
            item("ready", Some("2024-01-02")),
        ];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert_eq!(items[0].ts.as_deref(), Some("2024-01-03"));
        assert_eq!(items[1].ts.as_deref(), Some("2024-01-02"));
        assert_eq!(items[2].ts.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn none_timestamp_sorted_before_some() {
        // Matches original sort_drafts / sort_published_by_status_then_sent_at behaviour:
        // items with no timestamp sort ahead of those with one (None treated as "earliest"
        // in the descending key, so it wins the Less branch).
        let mut items = vec![item("ready", Some("2024-01-01")), item("ready", None)];
        sort_by_status_priority_then_timestamp(
            &mut items, "failed", "ready",
            |i| &i.status, |i| i.ts.as_deref(),
        );
        assert!(items[0].ts.is_none());
        assert_eq!(items[1].ts.as_deref(), Some("2024-01-01"));
    }
}
