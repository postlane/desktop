// SPDX-License-Identifier: BUSL-1.1

//! Copies Postlane skill files (`.md`/`.prompt`/`run.ts`/`preview-template.html`)
//! from the bundled Tauri resource dir into a newly-discovered child repo
//! (checklist 24.3.1). Called once per child repo during `setup_workspace`
//! (24.3.3), not once per workspace.
//!
//! A missing source directory or missing individual file degrades gracefully
//! (skips, does not error) rather than failing the whole workspace setup --
//! the `postlane/prompts` bundle is a sibling package this crate does not
//! control, and a partially-populated resource dir should not block setup.
//!
//! `tauri.conf.json` currently has NO `bundle.resources` entry at all --
//! production builds do not embed any skill files yet, and `copy_to_repo`
//! is a no-op until `init_skills_source_dir` is wired up for real. Two
//! separate problems block wiring this in, discovered building this item:
//!
//! 1. Tauri's build script hard-fails at compile time on a resource glob
//!    that matches zero files (not just a literal missing path) -- and
//!    `runner/run.ts`/`preview-template.html` don't exist in
//!    `postlane/prompts` yet regardless.
//! 2. More fundamentally: `postlane/desktop`'s CI (`.github/workflows/ci.yml`,
//!    `test-rust` job) only checks out `postlane/desktop` itself -- there is
//!    no sibling checkout of `postlane/prompts` at build time in CI at all,
//!    unlike a local dev machine where both repos sit side by side. A
//!    `bundle.resources` entry that works locally (`../../prompts/...`)
//!    will always fail CI until that workflow checks out `postlane/prompts`
//!    as a sibling directory -- which needs a cross-repo access token
//!    (`postlane/prompts` isn't public), a secrets-provisioning decision
//!    for whoever owns this repo's CI, not something to wire up silently.
//!
//! Once both are resolved, add back:
//! `"../../prompts/commands/*": "prompts/commands/"`,
//! `"../../prompts/runner/run.ts": "prompts/runner/run.ts"`,
//! `"../../prompts/preview-template.html": "prompts/preview-template.html"`
//! to `bundle.resources` -- this module's copy logic already handles all
//! three (see `copy_to_repo`) and needs no further change.

use std::path::{Path, PathBuf};

static SKILLS_SOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Called once at app startup with the Tauri resource dir's `prompts/`
/// subdirectory (populated at build time by `tauri.conf.json`'s
/// `bundle.resources`). A pure function like `copy_to_repo` cannot resolve
/// this itself -- only the `#[tauri::command]`/setup layer has an `AppHandle`.
pub fn init_skills_source_dir(path: PathBuf) {
    let _ = SKILLS_SOURCE_DIR.set(path);
}

// Thread-local, not a global static: cargo test runs many tests concurrently
// on separate threads, and most callers of `copy_to_repo` (e.g. every
// `workspace_setup` test that isn't specifically about skill files) never
// set an override at all -- a shared global would let one test's override
// leak into another running concurrently on a different thread. An
// RAII guard resets the value on scope exit (including on a panicking
// assertion, since Drop still runs during unwind), so a test can't leak
// its override into a later test that happens to reuse the same thread.
#[cfg(test)]
thread_local! {
    static TEST_SKILLS_SOURCE_OVERRIDE: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub struct SkillsSourceOverrideGuard;

#[cfg(test)]
impl Drop for SkillsSourceOverrideGuard {
    fn drop(&mut self) {
        TEST_SKILLS_SOURCE_OVERRIDE.with(|cell| *cell.borrow_mut() = None);
    }
}

#[cfg(test)]
#[must_use = "the override resets when this guard drops -- bind it (`let _guard = ...`), don't discard it"]
pub fn set_test_skills_source_override(path: Option<PathBuf>) -> SkillsSourceOverrideGuard {
    TEST_SKILLS_SOURCE_OVERRIDE.with(|cell| *cell.borrow_mut() = path);
    SkillsSourceOverrideGuard
}

/// Resolves the bundled-skills source directory: a test override in tests
/// (a definitely-nonexistent placeholder when no override is set, so
/// `copy_to_repo`'s missing-source-dir handling degrades gracefully for the
/// many tests of dependent modules that don't care about skill files at
/// all), the Tauri resource dir (`{app}/prompts`) in production.
fn resolve_skills_source_dir() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let path = TEST_SKILLS_SOURCE_OVERRIDE
            .with(|cell| cell.borrow().clone())
            .unwrap_or_else(|| PathBuf::from("/nonexistent/postlane-test-skills-source"));
        Ok(path)
    }
    #[cfg(not(test))]
    SKILLS_SOURCE_DIR.get().cloned().ok_or_else(|| {
        "skills source dir not initialized -- init_skills_source_dir must be called at app startup".to_string()
    })
}

fn copy_file_if_exists(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(source)
        .map_err(|e| format!("failed to read {}: {}", source.display(), e))?;
    crate::init::atomic_write(target, &bytes)
        .map_err(|e| format!("failed to write {}: {}", target.display(), e))
}

fn copy_commands_dir(source_dir: &Path, target_dir: &Path) -> Result<(), String> {
    let commands_dir = source_dir.join("commands");
    if !commands_dir.is_dir() {
        return Ok(());
    }
    let claude_dir = target_dir.join(".claude").join("commands");
    let postlane_dir = target_dir.join(".postlane").join("commands");

    let entries = std::fs::read_dir(&commands_dir)
        .map_err(|e| format!("failed to read {}: {}", commands_dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read dir entry: {}", e))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        match path.extension().and_then(|e| e.to_str()) {
            Some("md") => copy_file_if_exists(&path, &claude_dir.join(name))?,
            Some("prompt") => copy_file_if_exists(&path, &postlane_dir.join(name))?,
            _ => {}
        }
    }
    Ok(())
}

/// Copies every bundled skill file into `target_dir` (a discovered child repo):
/// `.md` commands to `.claude/commands/`, `.prompt` commands to
/// `.postlane/commands/`, plus `preview-template.html` and `runner/run.ts`
/// into their `.postlane/` locations. Writes are atomic (tmp + rename).
pub fn copy_to_repo(target_dir: &Path) -> Result<(), String> {
    let source_dir = resolve_skills_source_dir()?;

    copy_commands_dir(&source_dir, target_dir)?;
    copy_file_if_exists(
        &source_dir.join("preview-template.html"),
        &target_dir.join(".postlane").join("prompts").join("preview-template.html"),
    )?;
    copy_file_if_exists(
        &source_dir.join("runner").join("run.ts"),
        &target_dir.join(".postlane").join("runner").join("run.ts"),
    )?;

    Ok(())
}

#[cfg(test)]
#[path = "bundle_skills_tests.rs"]
mod tests;
