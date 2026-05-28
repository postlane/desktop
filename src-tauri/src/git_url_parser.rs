// SPDX-License-Identifier: BUSL-1.1

//! Git filesystem utilities: remote URL normalisation, `.git/config` parsing,
//! and directory scanning.
//!
//! [include] and [includeIf] git config directives are not followed (21.10.4).
//! Symlinks are not followed during directory scanning (21.10.11).

use std::path::{Path, PathBuf};

/// Normalises a GitHub remote URL to a lowercase `owner/repo` slug.
/// Returns `None` for non-GitHub remotes.
pub fn normalize_github_url(url: &str) -> Option<String> {
    let url = url.trim();
    let slug = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest.strip_suffix(".git").unwrap_or(rest)
    } else if let Some(rest) = url.strip_prefix("https://github.com/") {
        rest.strip_suffix(".git").unwrap_or(rest).trim_end_matches('/')
    } else {
        return None;
    };
    let (owner, repo) = slug.split_once('/')?;
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some(slug.to_lowercase())
}

/// Reads all remote URL values from a `.git/config` file.
/// Returns an empty vec on read error; never panics.
pub fn read_git_remote_urls(git_config_path: &Path) -> Vec<String> {
    match std::fs::read_to_string(git_config_path) {
        Ok(content) => extract_remote_urls_from_config(&content),
        Err(_) => vec![],
    }
}

// [include] and [includeIf] are not followed — repos connected only via a
// conditional include are excluded from discovery (documented in 21.10.4).
fn extract_remote_urls_from_config(content: &str) -> Vec<String> {
    let mut in_remote = false;
    let mut urls = Vec::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') {
            in_remote = section_is_remote(line);
            continue;
        }
        if in_remote {
            if let Some(url) = extract_url_key_value(line) {
                urls.push(url);
            }
        }
    }
    urls
}

fn section_is_remote(header: &str) -> bool {
    let inner = header.trim_start_matches('[').split(']').next().unwrap_or("").trim();
    let mut parts = inner.splitn(2, '"');
    matches!(parts.next().map(str::trim), Some("remote")) && parts.next().is_some()
}

fn extract_url_key_value(line: &str) -> Option<String> {
    let (k, v) = line.split_once('=')?;
    if k.trim() == "url" { Some(v.trim().to_string()) } else { None }
}

/// Scans `base_dirs` for directories containing `.git/`, up to 2 levels deep.
/// Stops after examining `limit` total directories. Symlinks are not followed.
pub fn scan_for_git_dirs(base_dirs: &[PathBuf], limit: usize) -> Vec<PathBuf> {
    use std::collections::VecDeque;
    let mut found = Vec::new();
    let mut queue: VecDeque<(PathBuf, u8)> = VecDeque::new();
    for base in base_dirs {
        if base.is_dir() {
            queue.push_back((base.clone(), 0));
        }
    }
    let mut count = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        if count >= limit {
            break;
        }
        count += 1;
        if dir.join(".git").is_dir() {
            found.push(dir);
            continue;
        }
        if depth >= 2 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            log::warn!("repo_discovery: cannot read {:?}", dir);
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_symlink() && meta.is_dir() {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── normalize_github_url ──────────────────────────────────────────────────

    #[test]
    fn test_normalize_ssh_url() {
        assert_eq!(
            normalize_github_url("git@github.com:my-org/my-repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_https_url() {
        assert_eq!(
            normalize_github_url("https://github.com/my-org/my-repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_https_no_git_suffix() {
        assert_eq!(
            normalize_github_url("https://github.com/my-org/my-repo"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_lowercases() {
        assert_eq!(
            normalize_github_url("git@github.com:My-Org/My-Repo.git"),
            Some("my-org/my-repo".to_string())
        );
    }

    #[test]
    fn test_normalize_gitlab_returns_none() {
        assert_eq!(normalize_github_url("git@gitlab.com:org/repo.git"), None);
    }

    #[test]
    fn test_normalize_bitbucket_https_returns_none() {
        assert_eq!(normalize_github_url("https://bitbucket.org/org/repo.git"), None);
    }

    // ── read_git_remote_urls ─────────────────────────────────────────────────

    #[test]
    fn test_read_ssh_remote_from_git_config() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(&cfg, "[remote \"origin\"]\n\turl = git@github.com:org/repo.git\n").unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:org/repo.git"]);
    }

    #[test]
    fn test_read_returns_empty_for_missing_file() {
        assert!(read_git_remote_urls(Path::new("/no/such/file")).is_empty());
    }

    #[test]
    fn test_read_skips_non_remote_sections() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(
            &cfg,
            "[core]\n\turl = not-a-real-url\n[remote \"origin\"]\n\turl = git@github.com:org/r.git\n",
        ).unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:org/r.git"]);
    }

    #[test]
    fn test_read_skips_comment_lines() {
        let tmp = TempDir::new().unwrap();
        let cfg = tmp.path().join("config");
        std::fs::write(
            &cfg,
            "[remote \"origin\"]\n# url = git@github.com:bad/repo.git\n\turl = git@github.com:good/repo.git\n",
        ).unwrap();
        assert_eq!(read_git_remote_urls(&cfg), vec!["git@github.com:good/repo.git"]);
    }

    // ── scan_for_git_dirs ────────────────────────────────────────────────────

    #[test]
    fn test_scan_finds_git_dir_at_depth_1() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("my-repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.contains(&repo));
    }

    #[test]
    fn test_scan_finds_git_dir_at_depth_2() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("projects").join("my-repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.contains(&repo));
    }

    #[test]
    fn test_scan_does_not_exceed_limit() {
        let tmp = TempDir::new().unwrap();
        for i in 0..600u32 {
            std::fs::create_dir_all(tmp.path().join(format!("dir-{i}")).join(".git")).unwrap();
        }
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.len() <= 500, "must not exceed 500");
    }

    #[test]
    fn test_scan_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let real = tmp.path().join("real-repo");
        std::fs::create_dir_all(real.join(".git")).unwrap();
        let link = tmp.path().join("link-repo");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let found = scan_for_git_dirs(&[tmp.path().to_path_buf()], 500);
        assert!(found.contains(&real));
        assert!(!found.contains(&link));
    }
}
