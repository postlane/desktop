// SPDX-License-Identifier: BUSL-1.1

use super::*;
use std::fs;
use tempfile::TempDir;

fn make_fixture_source() -> TempDir {
    let source = TempDir::new().unwrap();
    let commands_dir = source.path().join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(commands_dir.join("draft-post.md"), "# draft-post skill").unwrap();
    fs::write(commands_dir.join("draft-post.prompt"), "draft-post prompt body").unwrap();
    fs::write(commands_dir.join("draft-bluesky.md"), "# draft-bluesky skill").unwrap();
    fs::write(commands_dir.join("draft-bluesky.prompt"), "draft-bluesky prompt body").unwrap();
    fs::write(source.path().join("preview-template.html"), "<html></html>").unwrap();
    let runner_dir = source.path().join("runner");
    fs::create_dir_all(&runner_dir).unwrap();
    fs::write(runner_dir.join("run.ts"), "// runner").unwrap();
    source
}

// ── 24.3.1 ── copies .md/.prompt/run.ts/preview-template.html into the repo ──

#[test]
fn test_skill_files_copy_to_repo() {
    let source = make_fixture_source();
    set_test_skills_source_override(Some(source.path().to_path_buf()));

    let target = TempDir::new().unwrap();
    let result = copy_to_repo(target.path());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);

    let claude_md = target.path().join(".claude").join("commands").join("draft-post.md");
    assert!(claude_md.exists(), ".claude/commands/draft-post.md must exist");
    assert_eq!(fs::read_to_string(&claude_md).unwrap(), "# draft-post skill");

    let postlane_prompt = target.path().join(".postlane").join("commands").join("draft-post.prompt");
    assert!(postlane_prompt.exists(), ".postlane/commands/draft-post.prompt must exist");
    assert_eq!(fs::read_to_string(&postlane_prompt).unwrap(), "draft-post prompt body");

    let claude_md_2 = target.path().join(".claude").join("commands").join("draft-bluesky.md");
    assert!(claude_md_2.exists(), ".claude/commands/draft-bluesky.md must exist");

    let preview = target.path().join(".postlane").join("prompts").join("preview-template.html");
    assert!(preview.exists(), ".postlane/prompts/preview-template.html must exist");

    let run_ts = target.path().join(".postlane").join("runner").join("run.ts");
    assert!(run_ts.exists(), ".postlane/runner/run.ts must exist");

    set_test_skills_source_override(None);
}

// ── 24.3.1 ── calling twice produces identical content, no error ────────────

#[test]
fn test_copy_is_idempotent() {
    let source = make_fixture_source();
    set_test_skills_source_override(Some(source.path().to_path_buf()));

    let target = TempDir::new().unwrap();
    copy_to_repo(target.path()).expect("first copy must succeed");
    let first = fs::read_to_string(target.path().join(".claude").join("commands").join("draft-post.md")).unwrap();

    let result = copy_to_repo(target.path());
    assert!(result.is_ok(), "second copy must also succeed, got: {:?}", result);
    let second = fs::read_to_string(target.path().join(".claude").join("commands").join("draft-post.md")).unwrap();

    assert_eq!(first, second, "content must be identical after a second copy");

    set_test_skills_source_override(None);
}

// ── missing source dir is not a hard failure — degrades gracefully ──────────

#[test]
fn test_copy_to_repo_missing_source_dir_is_not_an_error() {
    let missing = TempDir::new().unwrap().path().join("does-not-exist");
    set_test_skills_source_override(Some(missing));

    let target = TempDir::new().unwrap();
    let result = copy_to_repo(target.path());
    assert!(result.is_ok(), "a missing source dir must degrade gracefully, not error: {:?}", result);
    assert!(
        !target.path().join(".claude").join("commands").exists(),
        "no commands dir should be created when there's nothing to copy"
    );

    set_test_skills_source_override(None);
}
