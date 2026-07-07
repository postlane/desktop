// SPDX-License-Identifier: BUSL-1.1

use crate::security::api_error::format_api_error;
use serde::Deserialize;
use std::path::{Path, PathBuf};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ProjectInfo {
    pub id: String,
    pub provider_org_login: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollTarget {
    pub repo_path: PathBuf,
    pub project_id: String,
}

// ── HTTP ──────────────────────────────────────────────────────────────────────

/// Calls `GET {api_base}/v1/projects` and returns lightweight project info.
pub async fn fetch_projects(
    api_base: &str,
    token: &str,
) -> Result<Vec<ProjectInfo>, String> {
    #[derive(Deserialize)]
    struct Body { projects: Vec<ProjectInfo> }

    let client = reqwest::Client::new();
    let url = format!("{}/v1/projects", api_base);
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("fetch_projects request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format_api_error("fetch_projects", resp.status().as_u16(), ""));
    }

    resp.json::<Body>().await
        .map(|b| b.projects)
        .map_err(|e| format!("fetch_projects parse failed: {}", e))
}

// ── Git remote parsing ────────────────────────────────────────────────────────

fn extract_org_from_url(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) {
        // https://github.com/org/repo  →  ["github.com", "org", "repo"]
        let mut parts = rest.splitn(3, '/');
        parts.next(); // domain
        return parts.next().filter(|s| !s.is_empty()).map(|s| s.to_string());
    }
    if let Some(colon_pos) = url.find(':') {
        // git@github.com:org/repo
        let path = &url[colon_pos + 1..];
        return path.split('/').next().filter(|s| !s.is_empty()).map(|s| s.to_string());
    }
    None
}

/// Reads the `origin` remote URL from a repo's `.git/config` and extracts
/// the org login (first path segment after the domain).
pub fn git_remote_org(repo_path: &Path) -> Option<String> {
    let config_path = repo_path.join(".git").join("config");
    let content = std::fs::read_to_string(&config_path).ok()?;
    let mut in_origin = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == r#"[remote "origin"]"# {
            in_origin = true;
            continue;
        }
        if in_origin {
            if trimmed.starts_with('[') { break; }
            if let Some(url) = trimmed.strip_prefix("url = ") {
                return extract_org_from_url(url);
            }
        }
    }
    None
}

// ── Poll target resolution ────────────────────────────────────────────────────

/// Builds the list of `(repo_path, project_id)` pairs that the poller should
/// query for.  Two sources are merged and deduplicated:
/// 1. Config-based: repo has `.postlane/config.json` with `project_id`.
/// 2. App-based: repo's git remote org matches a project's `provider_org_login`.
pub fn all_poll_targets(
    local_repos: &[PathBuf],
    projects: &[ProjectInfo],
) -> Vec<PollTarget> {
    let mut targets: Vec<PollTarget> = Vec::new();
    for repo_path in local_repos {
        let project_id = resolve_project_id(repo_path, projects);
        if let Some(id) = project_id {
            let already = targets.iter().any(|t| t.repo_path == *repo_path && t.project_id == id);
            if !already {
                targets.push(PollTarget { repo_path: repo_path.clone(), project_id: id });
            }
        }
    }
    targets
}

fn resolve_project_id(repo_path: &Path, projects: &[ProjectInfo]) -> Option<String> {
    if let Some(id) = config_project_id(repo_path) {
        return Some(id);
    }
    let org = git_remote_org(repo_path)?;
    projects.iter()
        .find(|p| p.provider_org_login.as_deref() == Some(org.as_str()))
        .map(|p| p.id.clone())
}

fn config_project_id(repo_path: &Path) -> Option<String> {
    let path = repo_path.join(".postlane").join("config.json");
    let val: serde_json::Value = crate::init::read_json_file(&path).ok()?;
    val.get("project_id")?.as_str().map(|s| s.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn write_git_config(repo_path: &Path, url: &str) {
        let git_dir = repo_path.join(".git");
        std::fs::create_dir_all(&git_dir).expect("create .git");
        let config = format!(
            "[core]\n\trepositoryformatversion = 0\n[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
            url
        );
        std::fs::write(git_dir.join("config"), config).expect("write git config");
    }

    // ── git_remote_org ───────────────────────────────────────────────────────

    #[test]
    fn test_git_remote_org_extracts_org_from_https_url() {
        let dir = std::env::temp_dir().join("pl_poll_routing_https");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        write_git_config(&dir, "https://github.com/acme-org/my-repo.git");

        let result = git_remote_org(&dir);
        assert_eq!(result, Some("acme-org".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_git_remote_org_extracts_org_from_ssh_url() {
        let dir = std::env::temp_dir().join("pl_poll_routing_ssh");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        write_git_config(&dir, "git@github.com:acme-org/my-repo.git");

        let result = git_remote_org(&dir);
        assert_eq!(result, Some("acme-org".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_git_remote_org_returns_none_when_no_git_dir() {
        let dir = std::env::temp_dir().join("pl_poll_routing_no_git");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");

        let result = git_remote_org(&dir);
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_git_remote_org_returns_none_when_no_origin_remote() {
        let dir = std::env::temp_dir().join("pl_poll_routing_no_origin");
        let _ = std::fs::remove_dir_all(&dir);
        let git_dir = dir.join(".git");
        std::fs::create_dir_all(&git_dir).expect("create .git");
        std::fs::write(git_dir.join("config"), "[core]\n\trepositoryformatversion = 0\n")
            .expect("write config");

        let result = git_remote_org(&dir);
        assert!(result.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── fetch_projects ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_projects_returns_projects_on_200() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(200).json_body(serde_json::json!({
                "projects": [
                    { "id": "proj-1", "provider_org_login": "acme-org",
                      "name": "Acme", "workspace_type": "organization",
                      "tier": "free", "billing_active": true, "is_owner": true },
                    { "id": "proj-2", "provider_org_login": null,
                      "name": "Personal", "workspace_type": "personal",
                      "tier": "free", "billing_active": true, "is_owner": true },
                ]
            }));
        });

        let result = fetch_projects(&server.base_url(), "tok").await.expect("should succeed");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "proj-1");
        assert_eq!(result[0].provider_org_login, Some("acme-org".to_string()));
        assert!(result[1].provider_org_login.is_none());
    }

    #[tokio::test]
    async fn test_fetch_projects_returns_err_on_401() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/v1/projects");
            then.status(401).json_body(serde_json::json!({ "error": "unauthorized" }));
        });

        let result = fetch_projects(&server.base_url(), "bad-tok").await;
        assert!(result.is_err());
    }

    // ── all_poll_targets ─────────────────────────────────────────────────────

    fn make_config_repo(base: &Path, project_id: &str) -> PathBuf {
        let postlane = base.join(".postlane");
        std::fs::create_dir_all(&postlane).expect("create .postlane");
        std::fs::write(
            postlane.join("config.json"),
            format!(r#"{{"project_id":"{}"}}"#, project_id),
        ).expect("write config.json");
        base.to_path_buf()
    }

    fn make_github_app_repo(base: &Path, org: &str) -> PathBuf {
        std::fs::create_dir_all(base).expect("create dir");
        write_git_config(base, &format!("https://github.com/{}/repo.git", org));
        base.to_path_buf()
    }

    #[test]
    fn test_all_poll_targets_uses_config_json_for_cli_repos() {
        let dir = std::env::temp_dir().join("pl_poll_routing_cli_repo");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        make_config_repo(&dir, "proj-cli-1");

        let projects = vec![
            ProjectInfo { id: "proj-cli-1".to_string(), provider_org_login: None },
        ];
        let targets = all_poll_targets(std::slice::from_ref(&dir), &projects);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].project_id, "proj-cli-1");
        assert_eq!(targets[0].repo_path, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_all_poll_targets_uses_remote_org_for_github_app_repos() {
        let dir = std::env::temp_dir().join("pl_poll_routing_app_repo");
        let _ = std::fs::remove_dir_all(&dir);
        make_github_app_repo(&dir, "acme-org");

        let projects = vec![
            ProjectInfo { id: "proj-app-1".to_string(), provider_org_login: Some("acme-org".to_string()) },
        ];
        let targets = all_poll_targets(std::slice::from_ref(&dir), &projects);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].project_id, "proj-app-1");
        assert_eq!(targets[0].repo_path, dir);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_all_poll_targets_deduplicates_config_and_remote_match() {
        let dir = std::env::temp_dir().join("pl_poll_routing_dedup");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create dir");
        // Both config.json AND git remote org match the same project
        make_config_repo(&dir, "proj-both");
        write_git_config(&dir, "https://github.com/acme-org/repo.git");

        let projects = vec![
            ProjectInfo { id: "proj-both".to_string(), provider_org_login: Some("acme-org".to_string()) },
        ];
        let targets = all_poll_targets(std::slice::from_ref(&dir), &projects);

        assert_eq!(targets.len(), 1, "should deduplicate when both sources match same repo");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_all_poll_targets_skips_repos_with_no_matching_project() {
        let dir = std::env::temp_dir().join("pl_poll_routing_no_match");
        let _ = std::fs::remove_dir_all(&dir);
        make_github_app_repo(&dir, "unknown-org");

        let projects = vec![
            ProjectInfo { id: "proj-other".to_string(), provider_org_login: Some("different-org".to_string()) },
        ];
        let targets = all_poll_targets(std::slice::from_ref(&dir), &projects);

        assert!(targets.is_empty(), "no match → no target");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
