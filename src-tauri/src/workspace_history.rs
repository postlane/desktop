// SPDX-License-Identifier: BUSL-1.1

//! Append-only post-send history log for workspace repos.
//!
//! On each successful `approve_post_impl`, a newline-delimited JSON entry is
//! appended to `{workspace}/history/{posts_dir}/sent.jsonl`. Rotation occurs
//! when the file exceeds 10,000 lines: the file is renamed to `sent.jsonl.1`
//! and a new `sent.jsonl` is started. Only one backup is retained.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

const ROTATION_THRESHOLD: u64 = 10_000;

/// A single sent-post history record appended to `sent.jsonl`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SentEntry {
    pub sent_at: String,
    pub repo_name: String,
    pub post_folder: String,
    pub platform: String,
    pub scheduler_id: String,
}

/// Appends `entry` to `{history_dir}/sent.jsonl` as a newline-delimited JSON record.
///
/// Creates `history_dir` if absent. Rotates when the line count reaches 10,000:
/// renames `sent.jsonl` → `sent.jsonl.1` (overwriting any previous backup) and
/// starts a new `sent.jsonl`. A sidecar file `sent.jsonl.count` tracks the line
/// count atomically; when absent at startup, the count is derived from the file.
pub fn append_sent_entry(history_dir: &Path, entry: &SentEntry) -> Result<(), String> {
    std::fs::create_dir_all(history_dir)
        .map_err(|e| format!("failed to create history dir: {}", e))?;

    let jsonl_path = history_dir.join("sent.jsonl");
    let count_path = history_dir.join("sent.jsonl.count");

    // Determine current line count from sidecar or file
    let current_count = if count_path.exists() {
        std::fs::read_to_string(&count_path)
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or_else(|| count_lines(&jsonl_path))
    } else {
        count_lines(&jsonl_path)
    };

    // Rotate if at or over threshold
    if current_count >= ROTATION_THRESHOLD {
        let bak_path = history_dir.join("sent.jsonl.1");
        if jsonl_path.exists() {
            std::fs::rename(&jsonl_path, &bak_path)
                .map_err(|e| format!("failed to rotate sent.jsonl: {}", e))?;
        }
        // Write the new entry as the first line of the fresh file
        let line = serialize_entry(entry)?;
        std::fs::write(&jsonl_path, format!("{}\n", line))
            .map_err(|e| format!("failed to write sent.jsonl after rotation: {}", e))?;
        write_count(&count_path, 1)?;
        return Ok(());
    }

    // Append to existing file
    let line = serialize_entry(entry)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&jsonl_path)
        .map_err(|e| format!("failed to open sent.jsonl: {}", e))?;
    writeln!(file, "{}", line)
        .map_err(|e| format!("failed to write to sent.jsonl: {}", e))?;

    // Update sidecar AFTER successful append
    write_count(&count_path, current_count + 1)?;
    Ok(())
}

fn serialize_entry(entry: &SentEntry) -> Result<String, String> {
    serde_json::to_string(entry)
        .map_err(|e| format!("failed to serialise sent entry: {}", e))
}

fn count_lines(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    std::fs::read_to_string(path)
        .map(|s| s.lines().count() as u64)
        .unwrap_or(0)
}

fn write_count(count_path: &Path, count: u64) -> Result<(), String> {
    crate::init::atomic_write(count_path, count.to_string().as_bytes())
        .map_err(|e| format!("failed to write sent.jsonl.count: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn read_lines(path: &std::path::Path) -> Vec<String> {
        fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// 22.2.18 — history entry written as valid JSON on approval success.
    #[test]
    fn test_append_sent_entry_writes_valid_json_line() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("frontend");

        let entry = SentEntry {
            sent_at: "2026-05-28T10:00:00Z".to_string(),
            repo_name: "frontend".to_string(),
            post_folder: "my-post".to_string(),
            platform: "bluesky".to_string(),
            scheduler_id: "sched-123".to_string(),
        };
        append_sent_entry(&hist_dir, &entry).expect("append");

        let path = hist_dir.join("sent.jsonl");
        assert!(path.exists(), "sent.jsonl must be created");
        let lines = read_lines(&path);
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON");
        assert_eq!(parsed["platform"].as_str(), Some("bluesky"));
        assert_eq!(parsed["repo_name"].as_str(), Some("frontend"));
        assert_eq!(parsed["post_folder"].as_str(), Some("my-post"));
    }

    /// 22.2.18 — second approval appends a second line; file never truncated.
    #[test]
    fn test_append_sent_entry_appends_not_truncates() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("backend");

        let e1 = SentEntry {
            sent_at: "2026-05-28T10:00:00Z".to_string(),
            repo_name: "backend".to_string(),
            post_folder: "post-1".to_string(),
            platform: "x".to_string(),
            scheduler_id: "s1".to_string(),
        };
        let e2 = SentEntry {
            sent_at: "2026-05-28T11:00:00Z".to_string(),
            repo_name: "backend".to_string(),
            post_folder: "post-2".to_string(),
            platform: "mastodon".to_string(),
            scheduler_id: "s2".to_string(),
        };
        append_sent_entry(&hist_dir, &e1).expect("first append");
        append_sent_entry(&hist_dir, &e2).expect("second append");

        let lines = read_lines(&hist_dir.join("sent.jsonl"));
        assert_eq!(lines.len(), 2, "second append must not truncate; file must have 2 lines");
        let p1: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        let p2: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(p1["post_folder"].as_str(), Some("post-1"));
        assert_eq!(p2["post_folder"].as_str(), Some("post-2"));
    }

    /// 22.2.19 — exactly 10,000 lines triggers rotation on next append.
    #[test]
    fn test_rotation_at_10000_lines() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("frontend");
        fs::create_dir_all(&hist_dir).unwrap();

        // Write exactly 10,000 lines to sent.jsonl
        let line = r#"{"sent_at":"2026-01-01T00:00:00Z","repo_name":"r","post_folder":"p","platform":"x","scheduler_id":"s"}"#;
        let content: String = std::iter::repeat(line)
            .take(10_000)
            .collect::<Vec<_>>()
            .join("\n") + "\n";
        fs::write(hist_dir.join("sent.jsonl"), &content).unwrap();
        // Sidecar reflecting the 10,000 line count
        fs::write(hist_dir.join("sent.jsonl.count"), "10000").unwrap();

        // Next append triggers rotation
        let entry = SentEntry {
            sent_at: "2026-05-28T00:00:00Z".to_string(),
            repo_name: "frontend".to_string(),
            post_folder: "new-post".to_string(),
            platform: "bluesky".to_string(),
            scheduler_id: "new".to_string(),
        };
        append_sent_entry(&hist_dir, &entry).expect("append after rotation");

        // Old file renamed to sent.jsonl.1
        assert!(hist_dir.join("sent.jsonl.1").exists(), "sent.jsonl.1 must exist after rotation");
        // New sent.jsonl has only the new entry
        let new_lines = read_lines(&hist_dir.join("sent.jsonl"));
        assert_eq!(new_lines.len(), 1, "new sent.jsonl must contain only the new entry");
        let parsed: serde_json::Value = serde_json::from_str(&new_lines[0]).unwrap();
        assert_eq!(parsed["post_folder"].as_str(), Some("new-post"));

        // Sidecar reset to 1
        let count: u64 = fs::read_to_string(hist_dir.join("sent.jsonl.count"))
            .unwrap().trim().parse().unwrap();
        assert_eq!(count, 1);
    }

    /// 22.2.20 — sidecar created on first append; incremented on subsequent appends.
    #[test]
    fn test_sidecar_created_and_incremented() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("repo");

        let entry = SentEntry {
            sent_at: "2026-05-28T00:00:00Z".to_string(),
            repo_name: "repo".to_string(),
            post_folder: "p1".to_string(),
            platform: "x".to_string(),
            scheduler_id: "s".to_string(),
        };

        append_sent_entry(&hist_dir, &entry).expect("first");
        let count: u64 = fs::read_to_string(hist_dir.join("sent.jsonl.count"))
            .unwrap().trim().parse().unwrap();
        assert_eq!(count, 1, "sidecar must be 1 after first append");

        append_sent_entry(&hist_dir, &entry).expect("second");
        let count: u64 = fs::read_to_string(hist_dir.join("sent.jsonl.count"))
            .unwrap().trim().parse().unwrap();
        assert_eq!(count, 2, "sidecar must be 2 after second append");
    }

    /// 22.2.20 — startup with no sidecar: count initialised from file, sidecar recreated.
    #[test]
    fn test_sidecar_recreated_from_file_when_absent() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("repo");
        fs::create_dir_all(&hist_dir).unwrap();

        // Write 3 lines, no sidecar
        let content = "line1\nline2\nline3\n";
        fs::write(hist_dir.join("sent.jsonl"), content).unwrap();

        let entry = SentEntry {
            sent_at: "2026-05-28T00:00:00Z".to_string(),
            repo_name: "repo".to_string(),
            post_folder: "p".to_string(),
            platform: "x".to_string(),
            scheduler_id: "s".to_string(),
        };
        append_sent_entry(&hist_dir, &entry).expect("append");

        let count: u64 = fs::read_to_string(hist_dir.join("sent.jsonl.count"))
            .unwrap().trim().parse().unwrap();
        assert_eq!(count, 4, "sidecar must reflect existing 3 lines plus 1 new");
    }

    /// 22.2.21 — second rotation overwrites sent.jsonl.1; no error.
    #[test]
    fn test_second_rotation_overwrites_backup() {
        let dir = TempDir::new().unwrap();
        let hist_dir = dir.path().join("history").join("repo");
        fs::create_dir_all(&hist_dir).unwrap();

        // First rotation: write 10,000 line file, rotate
        let line = r#"{"sent_at":"2026-01-01T00:00:00Z","repo_name":"r","post_folder":"p","platform":"x","scheduler_id":"s"}"#;
        let content: String = std::iter::repeat(line).take(10_000).collect::<Vec<_>>().join("\n") + "\n";
        fs::write(hist_dir.join("sent.jsonl"), &content).unwrap();
        fs::write(hist_dir.join("sent.jsonl.count"), "10000").unwrap();

        let e = SentEntry {
            sent_at: "2026-05-28T00:00:00Z".to_string(),
            repo_name: "r".to_string(), post_folder: "first-rotation".to_string(),
            platform: "x".to_string(), scheduler_id: "s".to_string(),
        };
        append_sent_entry(&hist_dir, &e).expect("first rotation");

        // Write 10,000 more lines, trigger second rotation
        let content2: String = std::iter::repeat(line).take(10_000).collect::<Vec<_>>().join("\n") + "\n";
        fs::write(hist_dir.join("sent.jsonl"), &content2).unwrap();
        fs::write(hist_dir.join("sent.jsonl.count"), "10000").unwrap();

        let e2 = SentEntry {
            sent_at: "2026-05-28T00:00:00Z".to_string(),
            repo_name: "r".to_string(), post_folder: "second-rotation".to_string(),
            platform: "x".to_string(), scheduler_id: "s".to_string(),
        };
        append_sent_entry(&hist_dir, &e2).expect("second rotation");

        // sent.jsonl.1 should now contain the second batch (overwritten)
        assert!(hist_dir.join("sent.jsonl.1").exists(), "sent.jsonl.1 must still exist");
        let bak_content = fs::read_to_string(hist_dir.join("sent.jsonl.1")).unwrap();
        // The second rotation content (10,000 lines) is now in .1
        assert_eq!(bak_content.lines().count(), 10_000);

        // New sent.jsonl has only the newest entry
        let new_lines = read_lines(&hist_dir.join("sent.jsonl"));
        assert_eq!(new_lines.len(), 1);
    }
}
