// SPDX-License-Identifier: BUSL-1.1

use std::path::{Component, Path, PathBuf};

/// Resolve the filesystem path where a `markdown_file` skill should write its output.
///
/// Uses the `draft_output_dir` field from `config.json` (default: `.postlane/drafts`).
/// The directory is always relative to `repo_path`; absolute paths and `..` traversal
/// in `draft_output_dir` are rejected (Security Rule 2).
pub fn resolve_draft_output_path(
    repo_path: &Path,
    config: &serde_json::Value,
    filename: &str,
) -> Result<PathBuf, String> {
    let dir = config["draft_output_dir"]
        .as_str()
        .unwrap_or(".postlane/drafts");

    validate_draft_output_dir(dir)?;

    Ok(repo_path.join(dir).join(filename))
}

/// Resolve draft output path for workspace repos (22.2.3).
///
/// Defaults to `{workspace_path}/drafts/{filename}` when `draft_output_dir` is absent.
/// When `draft_output_dir` is present in config, resolves it relative to `repo_path`.
pub fn resolve_workspace_draft_output_path(
    workspace_path: &Path,
    repo_path: &Path,
    config: &serde_json::Value,
    filename: &str,
) -> Result<PathBuf, String> {
    if let Some(dir) = config["draft_output_dir"].as_str() {
        validate_draft_output_dir(dir)?;
        Ok(repo_path.join(dir).join(filename))
    } else {
        Ok(workspace_path.join("drafts").join(filename))
    }
}

fn validate_draft_output_dir(dir: &str) -> Result<(), String> {
    if dir.starts_with('/') || dir.starts_with("\\\\") {
        return Err(format!(
            "draft_output_dir must be a relative path, got absolute: {}",
            dir
        ));
    }
    for component in Path::new(dir).components() {
        if component == Component::ParentDir {
            return Err(format!(
                "draft_output_dir must not contain '..': {}",
                dir
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 20.10.3 — default path
    #[test]
    fn test_resolve_draft_output_path_defaults_to_postlane_drafts() {
        let repo = Path::new("/tmp/my-repo");
        let config = serde_json::json!({});
        let path = resolve_draft_output_path(repo, &config, "newsletter.md").expect("resolve");
        assert_eq!(path, PathBuf::from("/tmp/my-repo/.postlane/drafts/newsletter.md"));
    }

    // 20.10.3 — config override
    #[test]
    fn test_resolve_draft_output_path_respects_config_override() {
        let repo = Path::new("/tmp/my-repo");
        let config = serde_json::json!({"draft_output_dir": "content/drafts"});
        let path = resolve_draft_output_path(repo, &config, "newsletter.md").expect("resolve");
        assert_eq!(path, PathBuf::from("/tmp/my-repo/content/drafts/newsletter.md"));
    }

    // Security Rule 2 — absolute draft_output_dir rejected
    #[test]
    fn test_resolve_draft_output_path_rejects_absolute_dir() {
        let repo = Path::new("/tmp/my-repo");
        let config = serde_json::json!({"draft_output_dir": "/etc/passwd"});
        let result = resolve_draft_output_path(repo, &config, "file.md");
        assert!(result.is_err(), "absolute draft_output_dir must be rejected");
        assert!(result.unwrap_err().contains("absolute"), "error must mention 'absolute'");
    }

    // Security Rule 2 — parent traversal rejected
    #[test]
    fn test_resolve_draft_output_path_rejects_parent_traversal() {
        let repo = Path::new("/tmp/my-repo");
        let config = serde_json::json!({"draft_output_dir": "../../etc"});
        let result = resolve_draft_output_path(repo, &config, "file.md");
        assert!(result.is_err(), "traversal in draft_output_dir must be rejected");
        assert!(result.unwrap_err().contains(".."), "error must mention '..'");
    }

    // 20.10.15 — CLI cwd context (absolute repo path from canonicalised cwd)
    #[test]
    fn test_resolve_draft_output_path_from_cli_cwd_context() {
        let repo = Path::new("/Users/dev/projects/my-repo");
        let config = serde_json::json!({});
        let path = resolve_draft_output_path(repo, &config, "investor-update.md").expect("resolve");
        assert_eq!(
            path,
            PathBuf::from("/Users/dev/projects/my-repo/.postlane/drafts/investor-update.md"),
        );
    }

    // 20.10.15 — watcher absolute path context (absolute path from repos.json)
    #[test]
    fn test_resolve_draft_output_path_from_watcher_absolute_path() {
        let repo = Path::new("/home/user/workspace/postlane-docs");
        let config = serde_json::json!({"draft_output_dir": ".postlane/drafts"});
        let path = resolve_draft_output_path(repo, &config, "blog-post.md").expect("resolve");
        assert_eq!(
            path,
            PathBuf::from("/home/user/workspace/postlane-docs/.postlane/drafts/blog-post.md"),
        );
    }

    // ── 22.2.3 workspace draft output dir ────────────────────────────────────

    /// 22.2.3 — defaults to {workspace}/drafts/ when workspace_path is Some and no override.
    #[test]
    fn test_resolve_workspace_draft_output_defaults_to_workspace_drafts() {
        let repo = Path::new("/code/myorg/frontend");
        let ws = Path::new("/code/myorg");
        let config = serde_json::json!({});
        let path = super::resolve_workspace_draft_output_path(ws, repo, &config, "post.md")
            .expect("resolve");
        assert_eq!(path, PathBuf::from("/code/myorg/drafts/post.md"));
    }

    /// 22.2.3 — per-repo draft_output_dir override still honoured.
    #[test]
    fn test_resolve_workspace_draft_output_respects_override() {
        let repo = Path::new("/code/myorg/frontend");
        let ws = Path::new("/code/myorg");
        let config = serde_json::json!({"draft_output_dir": "content/drafts"});
        let path = super::resolve_workspace_draft_output_path(ws, repo, &config, "post.md")
            .expect("resolve");
        assert_eq!(path, PathBuf::from("/code/myorg/frontend/content/drafts/post.md"));
    }

    // Different filenames route to the same dir
    #[test]
    fn test_resolve_draft_output_path_different_filenames() {
        let repo = Path::new("/tmp/repo");
        let config = serde_json::json!({});
        let newsletter = resolve_draft_output_path(repo, &config, "newsletter.md").expect("resolve");
        let investor = resolve_draft_output_path(repo, &config, "investor-update.md").expect("resolve");
        let blog = resolve_draft_output_path(repo, &config, "blog-post.md").expect("resolve");
        assert!(newsletter.ends_with(".postlane/drafts/newsletter.md"));
        assert!(investor.ends_with(".postlane/drafts/investor-update.md"));
        assert!(blog.ends_with(".postlane/drafts/blog-post.md"));
    }
}
