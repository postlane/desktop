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
//! `tauri.conf.json`'s `bundle.resources` currently bundles only
//! `prompts/commands/*` -- `runner/run.ts` and `preview-template.html` are
//! NOT bundled, because Tauri's build script hard-fails at compile time on a
//! literal (non-glob) resource path that doesn't exist on disk, and neither
//! file exists in `postlane/prompts` as of this writing. Once that sibling
//! repo adds them, add `"../../prompts/runner/run.ts": "prompts/runner/run.ts"`
//! and `"../../prompts/preview-template.html": "prompts/preview-template.html"`
//! back to `bundle.resources` -- this module's copy logic already handles
//! them (see `copy_to_repo`) and needs no change.

use std::path::{Path, PathBuf};

static SKILLS_SOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Called once at app startup with the Tauri resource dir's `prompts/`
/// subdirectory (populated at build time by `tauri.conf.json`'s
/// `bundle.resources`). A pure function like `copy_to_repo` cannot resolve
/// this itself -- only the `#[tauri::command]`/setup layer has an `AppHandle`.
pub fn init_skills_source_dir(path: PathBuf) {
    let _ = SKILLS_SOURCE_DIR.set(path);
}

#[cfg(test)]
static TEST_SKILLS_SOURCE_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<PathBuf>>> =
    std::sync::OnceLock::new();

#[cfg(test)]
pub fn set_test_skills_source_override(path: Option<PathBuf>) {
    *TEST_SKILLS_SOURCE_OVERRIDE
        .get_or_init(|| std::sync::Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner()) = path;
}

/// Resolves the bundled-skills source directory: a test override in tests,
/// the Tauri resource dir (`{app}/prompts`) in production.
fn resolve_skills_source_dir() -> Result<PathBuf, String> {
    #[cfg(test)]
    {
        let maybe = TEST_SKILLS_SOURCE_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(path) = maybe {
            return Ok(path);
        }
    }
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
