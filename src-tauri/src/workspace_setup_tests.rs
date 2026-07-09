// SPDX-License-Identifier: BUSL-1.1

use super::*;
use crate::child_repo_discovery::ChildRepo;
use std::fs;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

fn make_git_repo(dir: &std::path::Path) {
    fs::create_dir_all(dir.join(".git")).expect("create .git");
}

fn sample_config(project_id: &str) -> WorkspaceConfig {
    WorkspaceConfig {
        project_id: project_id.to_string(),
        base_url: None,
        platforms: vec!["x".to_string(), "bluesky".to_string()],
        mastodon_instance: None,
        llm_provider: "anthropic".to_string(),
        llm_model: "claude-sonnet-4-6".to_string(),
        author: "Jordan Reyes".to_string(),
        style: "Direct, no jargon".to_string(),
        utm_campaign: None,
        attribution: true,
        scheduler_provider: "zernio".to_string(),
        scheduler_api_key: "sk-test-secret-value".to_string(),
        scheduler_profile_id: None,
    }
}

fn noop_keyring() -> impl Fn(&str, &str) -> Result<(), String> {
    |_, _| Ok(())
}

/// Runs `setup_workspace_impl` against a fresh single-child-repo tempdir
/// workspace with `config` and returns the parsed `config.json`. Shared by
/// the config.json-shape tests to keep each test's own cognitive complexity
/// (clippy's 12-branch limit) to just its assertions.
fn setup_and_parse_config_json(config: &WorkspaceConfig) -> serde_json::Value {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    let child = ws.path().join("frontend");
    make_git_repo(&child);
    let child_repos = vec![ChildRepo {
        name: "frontend".to_string(),
        path: child.to_string_lossy().to_string(),
        posts_dir: "frontend".to_string(),
    }];

    setup_workspace_impl(ws.path(), &repos_path, config, &child_repos, &noop_keyring())
        .expect("setup must succeed");
    serde_json::from_str(&fs::read_to_string(ws.path().join("config.json")).unwrap()).unwrap()
}

// ── 24.3.3 ── writes config.json with the full field set ────────────────────

#[test]
fn test_setup_writes_correct_config_json_identity_fields() {
    let parsed = setup_and_parse_config_json(&sample_config("proj-abc"));
    assert_eq!(parsed["version"], 1);
    assert_eq!(parsed["project_id"], "proj-abc");
    assert_eq!(parsed["base_url"], "https://postlane.dev");
    assert_eq!(parsed["platforms"], serde_json::json!(["x", "bluesky"]));
}

#[test]
fn test_setup_writes_correct_config_json_content_fields() {
    let parsed = setup_and_parse_config_json(&sample_config("proj-abc"));
    assert_eq!(parsed["llm"]["provider"], "anthropic");
    assert_eq!(parsed["llm"]["model"], "claude-sonnet-4-6");
    assert_eq!(parsed["author"], "Jordan Reyes");
    assert_eq!(parsed["style"], "Direct, no jargon");
}

#[test]
fn test_setup_writes_correct_config_json_omits_unset_optional_fields() {
    let parsed = setup_and_parse_config_json(&sample_config("proj-abc"));
    assert!(parsed.get("mastodon_instance").is_none(), "mastodon_instance omitted when not set");
    assert!(parsed.get("utm_campaign").is_none(), "utm_campaign omitted when empty");
    assert!(parsed.get("attribution").is_none(), "attribution key omitted when on (default)");
}

#[test]
fn test_setup_config_json_write_is_atomic_no_tmp_leftover() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    let child = ws.path().join("frontend");
    make_git_repo(&child);
    let child_repos = vec![ChildRepo {
        name: "frontend".to_string(),
        path: child.to_string_lossy().to_string(),
        posts_dir: "frontend".to_string(),
    }];
    let config = sample_config("proj-atomic");

    setup_workspace_impl(ws.path(), &repos_path, &config, &child_repos, &noop_keyring())
        .expect("setup must succeed");

    let leftover_tmp: Vec<_> = fs::read_dir(ws.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftover_tmp.is_empty(), "no .tmp files should remain after atomic write");
}

// ── 24.3.3 ── Mastodon instance written only when set, omitted otherwise ────

#[test]
fn test_setup_mastodon_instance_written() {
    let mut config = sample_config("proj-mastodon");
    config.mastodon_instance = Some("mastodon.social".to_string());
    let parsed = setup_and_parse_config_json(&config);
    assert_eq!(parsed["mastodon_instance"], "mastodon.social");
}

#[test]
fn test_setup_mastodon_instance_omitted_when_not_checked() {
    let config = sample_config("proj-no-mastodon"); // mastodon_instance: None
    let parsed = setup_and_parse_config_json(&config);
    assert!(parsed.get("mastodon_instance").is_none());
}

// ── 24.3.3 ── attribution: false only present when opted out ────────────────

#[test]
fn test_setup_attribution_false_when_opted_out() {
    let mut config = sample_config("proj-no-attr");
    config.attribution = false;
    let parsed = setup_and_parse_config_json(&config);
    assert_eq!(parsed["attribution"], false);
}

// ── 24.3.3 ── skill files copied into every discovered child repo ───────────

#[test]
fn test_setup_copies_skill_files_to_each_child_repo() {
    let source = TempDir::new().unwrap();
    let commands_dir = source.path().join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(commands_dir.join("draft-post.md"), "# draft-post").unwrap();
    fs::write(commands_dir.join("draft-post.prompt"), "draft-post body").unwrap();
    let _guard = crate::bundle_skills::set_test_skills_source_override(Some(source.path().to_path_buf()));

    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    let child_a = ws.path().join("frontend");
    let child_b = ws.path().join("backend");
    make_git_repo(&child_a);
    make_git_repo(&child_b);
    let child_repos = vec![
        ChildRepo { name: "frontend".to_string(), path: child_a.to_string_lossy().to_string(), posts_dir: "frontend".to_string() },
        ChildRepo { name: "backend".to_string(), path: child_b.to_string_lossy().to_string(), posts_dir: "backend".to_string() },
    ];
    let config = sample_config("proj-skills");

    let result = setup_workspace_impl(ws.path(), &repos_path, &config, &child_repos, &noop_keyring());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    for child in [&child_a, &child_b] {
        assert!(
            child.join(".claude").join("commands").join("draft-post.md").exists(),
            "{} must have .claude/commands/draft-post.md", child.display()
        );
        assert!(
            child.join(".postlane").join("commands").join("draft-post.prompt").exists(),
            "{} must have .postlane/commands/draft-post.prompt", child.display()
        );
    }
}

// ── 24.3.3 ── scheduler API key goes to keyring, never to any file ──────────

#[test]
fn test_setup_api_key_not_written_to_disk() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    let child = ws.path().join("frontend");
    make_git_repo(&child);
    let child_repos = vec![ChildRepo {
        name: "frontend".to_string(),
        path: child.to_string_lossy().to_string(),
        posts_dir: "frontend".to_string(),
    }];
    let config = sample_config("proj-secret");
    let secret = config.scheduler_api_key.clone();

    let calls: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();
    let set_keyring = move |key: &str, val: &str| -> Result<(), String> {
        calls_clone.lock().unwrap().push((key.to_string(), val.to_string()));
        Ok(())
    };

    setup_workspace_impl(ws.path(), &repos_path, &config, &child_repos, &set_keyring)
        .expect("setup must succeed");

    let recorded = calls.lock().unwrap();
    assert_eq!(recorded.len(), 1, "keyring must be called exactly once");
    assert_eq!(recorded[0].0, "zernio/proj-secret");
    assert_eq!(recorded[0].1, secret);
    drop(recorded);

    // Grep the entire workspace tree for the literal secret — must appear nowhere.
    fn contains_secret(dir: &std::path::Path, secret: &str) -> bool {
        let Ok(entries) = fs::read_dir(dir) else { return false };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if contains_secret(&path, secret) {
                    return true;
                }
            } else if let Ok(content) = fs::read_to_string(&path) {
                if content.contains(secret) {
                    return true;
                }
            }
        }
        false
    }
    assert!(!contains_secret(ws.path(), &secret), "API key must not appear in any file on disk");
}

// ── 24.3.3 ── workspace + child repos registered ─────────────────────────────

#[test]
fn test_setup_registers_workspace() {
    let ws = TempDir::new().unwrap();
    let repos_dir = TempDir::new().unwrap();
    let repos_path = repos_dir.path().join("repos.json");
    let child_a = ws.path().join("frontend");
    let child_b = ws.path().join("backend");
    make_git_repo(&child_a);
    make_git_repo(&child_b);
    let child_repos = vec![
        ChildRepo { name: "frontend".to_string(), path: child_a.to_string_lossy().to_string(), posts_dir: "frontend".to_string() },
        ChildRepo { name: "backend".to_string(), path: child_b.to_string_lossy().to_string(), posts_dir: "backend-2".to_string() },
    ];
    let config = sample_config("proj-register");

    let result = setup_workspace_impl(ws.path(), &repos_path, &config, &child_repos, &noop_keyring());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    // Global ~/.postlane/repos.json gets a new WorkspaceEntry.
    let global: crate::storage::ReposConfig =
        serde_json::from_str(&fs::read_to_string(&repos_path).unwrap()).unwrap();
    assert_eq!(global.workspaces.len(), 1);
    assert_eq!(global.workspaces[0].id, "proj-register");

    // {workspace}/repos.json gets the pre-assigned posts_dir values unchanged (no re-dedup).
    let ws_repos: crate::workspace_repos::WorkspaceReposConfig =
        serde_json::from_str(&fs::read_to_string(ws.path().join("repos.json")).unwrap()).unwrap();
    assert_eq!(ws_repos.repos.len(), 2);
    assert!(ws_repos.repos.iter().any(|r| r.name == "frontend" && r.posts_dir == "frontend"));
    assert!(ws_repos.repos.iter().any(|r| r.name == "backend" && r.posts_dir == "backend-2"));

    assert!(ws.path().join("posts").is_dir(), "posts/ must be created eagerly");
    assert!(ws.path().join("drafts").is_dir(), "drafts/ must be created eagerly");
}
