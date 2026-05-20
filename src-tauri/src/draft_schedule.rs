// SPDX-License-Identifier: BUSL-1.1

pub use crate::schedule_time::compute_schedule_utc;

/// Production entry point: reads app state from disk and uses the current time.
pub fn pre_populate_schedule_if_needed(meta_path: &std::path::Path) -> Result<(), String> {
    let app_state = crate::app_state::read_app_state();
    pre_populate_schedule_from_state(meta_path, &app_state, chrono::Utc::now())
}

/// Testable variant that accepts injected state and reference time.
pub(crate) fn pre_populate_schedule_from_state(
    meta_path: &std::path::Path,
    app_state: &crate::app_state::AppStateFile,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), String> {
    pre_populate_with_version_lookup(
        meta_path, app_state, now,
        crate::voice_guide_versions::lookup_version,
    )
}

/// Injectable variant: `lookup_fn` is called with the project_id to get the voice guide version.
/// Callers in tests can pass a closure; production callers use the real lookup.
pub(crate) fn pre_populate_with_version_lookup(
    meta_path: &std::path::Path,
    app_state: &crate::app_state::AppStateFile,
    now: chrono::DateTime<chrono::Utc>,
    lookup_fn: impl Fn(&str) -> Option<String>,
) -> Result<(), String> {
    let mut meta = crate::post_mutations::read_post_meta(meta_path)?;
    let mut dirty = false;

    if meta.voice_guide_version.is_none() {
        if let Some(pid) = project_id_for_meta(meta_path) {
            meta.voice_guide_version = lookup_fn(&pid);
            dirty |= meta.voice_guide_version.is_some();
        }
    }

    if meta.llm_model.is_none() {
        if let Some(model) = llm_model_for_meta(meta_path) {
            meta.llm_model = Some(model);
            dirty = true;
        }
    }

    if meta.created_at.is_none() {
        meta.created_at = Some(now.to_rfc3339());
        dirty = true;
    }

    let post_folder = meta_path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let schedule_is_set = meta.schedule.as_deref().is_some_and(|s| !s.is_empty());
    if !schedule_is_set {
        if let Some(ref dpt) = app_state.default_post_time {
            let tz = if dpt.timezone.is_empty() { &app_state.timezone } else { &dpt.timezone };
            let base = compute_schedule_utc(dpt, tz, now)?;
            meta.schedule = Some(apply_schedule_jitter(&base, post_folder));
            meta.schedule_source = Some("default".to_string());
            dirty = true;
        }
    }

    if dirty {
        crate::post_mutations::write_post_meta(meta_path, &meta)?;
    }
    Ok(())
}

/// Apply a deterministic ±5-minute jitter to a schedule string based on a seed.
/// Uses FNV-1a 64-bit hash of the seed, mapped to the range [-300, +300] seconds.
/// Returns the original string unchanged if it cannot be parsed.
pub(crate) fn apply_schedule_jitter(schedule: &str, seed: &str) -> String {
    let offset_secs = seed_to_offset(seed);
    match schedule.parse::<chrono::DateTime<chrono::Utc>>() {
        Ok(dt) => (dt + chrono::Duration::seconds(offset_secs)).to_rfc3339(),
        Err(_) => schedule.to_string(),
    }
}

fn seed_to_offset(seed: &str) -> i64 {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    for byte in seed.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    (hash % 601) as i64 - 300
}

fn config_for_meta(meta_path: &std::path::Path) -> Option<serde_json::Value> {
    // meta.json lives at {repo}/.postlane/posts/{folder}/meta.json — 4 levels up is repo root
    let repo_root = meta_path.parent()?.parent()?.parent()?.parent()?;
    let content = std::fs::read_to_string(repo_root.join(".postlane/config.json")).ok()?;
    serde_json::from_str(&content).ok()
}

fn project_id_for_meta(meta_path: &std::path::Path) -> Option<String> {
    config_for_meta(meta_path)?.get("project_id").and_then(|p| p.as_str()).map(String::from)
}

fn llm_model_for_meta(meta_path: &std::path::Path) -> Option<String> {
    config_for_meta(meta_path)?
        .get("llm").and_then(|l| l.get("model")).and_then(|m| m.as_str()).map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;
    use std::path::Path;

    fn write_meta(dir: &Path, json: &str) -> std::path::PathBuf {
        let p = dir.join("meta.json");
        fs::write(&p, json).expect("write meta.json");
        p
    }

    fn state_with_dpt(hour: u8, minute: u8, tz: &str) -> crate::app_state::AppStateFile {
        crate::app_state::AppStateFile {
            default_post_time: Some(crate::app_state::DefaultPostTime { hour, minute, timezone: tz.to_string() }),
            timezone: tz.to_string(),
            ..crate::app_state::AppStateFile::default()
        }
    }

    fn utc(y: i32, mo: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(y, mo, d, h, min, 0).unwrap()
    }

    // ── pre_populate_schedule_from_state ─────────────────────────────────────

    #[test]
    fn test_pre_populate_does_nothing_when_schedule_already_set() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(),
            r#"{"status":"ready","platforms":["x"],"schedule":"2026-05-05T09:30:00Z"}"#);
        let state = state_with_dpt(10, 0, "UTC");
        pre_populate_schedule_from_state(&meta_path, &state, utc(2026, 5, 5, 8, 0)).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.schedule.as_deref(), Some("2026-05-05T09:30:00Z"));
    }

    #[test]
    fn test_pre_populate_does_nothing_when_default_post_time_null() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = crate::app_state::AppStateFile {
            default_post_time: None,
            timezone: "UTC".to_string(),
            ..crate::app_state::AppStateFile::default()
        };
        pre_populate_schedule_from_state(&meta_path, &state, utc(2026, 5, 5, 8, 0)).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert!(meta.schedule.is_none());
    }

    #[test]
    fn test_pre_populate_sets_schedule_when_default_set() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = state_with_dpt(9, 30, "UTC");
        pre_populate_schedule_from_state(&meta_path, &state, utc(2026, 5, 5, 8, 0)).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        let schedule_str = meta.schedule.as_deref().unwrap_or("");
        let scheduled: chrono::DateTime<chrono::Utc> = schedule_str.parse()
            .expect("schedule should be a valid ISO 8601 datetime");
        let expected: chrono::DateTime<chrono::Utc> = "2026-05-05T09:30:00Z".parse().unwrap();
        let diff = (scheduled - expected).num_seconds().abs();
        assert!(diff <= 300, "schedule '{}' is more than 5 min from 09:30 UTC (diff: {}s)", schedule_str, diff);
    }

    #[test]
    fn test_pre_populate_writes_atomically_no_tmp_file_left() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = state_with_dpt(9, 0, "UTC");
        pre_populate_schedule_from_state(&meta_path, &state, utc(2026, 5, 5, 8, 0)).unwrap();
        assert!(!dir.path().join("meta.json.tmp").exists(), "tmp file must not remain");
    }

    #[test]
    fn test_pre_populate_captures_voice_guide_version_when_available() {
        let base = tempfile::TempDir::new().expect("create temp dir");
        let post_folder = base.path().join("repo/.postlane/posts/my-post");
        fs::create_dir_all(&post_folder).unwrap();
        let config_dir = base.path().join("repo/.postlane");
        fs::write(config_dir.join("config.json"), r#"{"project_id":"proj-test"}"#).unwrap();
        let meta_path = post_folder.join("meta.json");
        fs::write(&meta_path, r#"{"status":"ready","platforms":["x"]}"#).unwrap();
        let state = state_with_dpt(9, 0, "UTC");
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |pid| {
            if pid == "proj-test" { Some("2026-05-05T10:00:00Z".to_string()) } else { None }
        }).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.voice_guide_version.as_deref(), Some("2026-05-05T10:00:00Z"));
    }

    #[test]
    fn test_pre_populate_leaves_voice_guide_version_none_when_lookup_returns_none() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = state_with_dpt(9, 0, "UTC");
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert!(meta.voice_guide_version.is_none(), "voice_guide_version should be None when lookup returns None");
    }

    #[test]
    fn test_pre_populate_sets_schedule_source_to_default() {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = state_with_dpt(9, 0, "UTC");
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.schedule_source.as_deref(), Some("default"), "schedule_source should be 'default' when auto-populated");
    }

    #[test]
    fn test_apply_schedule_jitter_same_seed_is_deterministic() {
        let s = "2026-06-01T09:00:00Z";
        let j1 = apply_schedule_jitter(s, "my-post-001");
        let j2 = apply_schedule_jitter(s, "my-post-001");
        assert_eq!(j1, j2, "same seed must produce same jitter");
    }

    #[test]
    fn test_apply_schedule_jitter_different_seeds_differ() {
        let s = "2026-06-01T09:00:00Z";
        let j1 = apply_schedule_jitter(s, "my-post-001");
        let j2 = apply_schedule_jitter(s, "my-post-002");
        assert_ne!(j1, j2, "different seeds should (almost always) produce different schedules");
    }

    #[test]
    fn test_apply_schedule_jitter_stays_within_five_minutes() {
        let base: chrono::DateTime<chrono::Utc> = "2026-06-01T09:00:00Z".parse().unwrap();
        for seed in &["post-001", "post-002", "post-aaa", "abcdefgh"] {
            let jittered_str = apply_schedule_jitter("2026-06-01T09:00:00Z", seed);
            let jittered: chrono::DateTime<chrono::Utc> = jittered_str.parse().unwrap();
            let diff = (jittered - base).num_seconds().abs();
            assert!(diff <= 300, "seed '{}' jitter {} seconds exceeds ±5 min", seed, diff);
        }
    }

    fn setup_repo_meta(dir: &Path, config_json: &str, meta_json: &str) -> std::path::PathBuf {
        let post_dir = dir.join(".postlane/posts/p");
        fs::create_dir_all(&post_dir).unwrap();
        fs::write(dir.join(".postlane/config.json"), config_json).unwrap();
        let p = post_dir.join("meta.json");
        fs::write(&p, meta_json).unwrap();
        p
    }

    #[test]
    fn test_pre_populate_sets_llm_model_from_config() {
        let base = tempfile::TempDir::new().unwrap();
        let meta_path = setup_repo_meta(base.path(),
            r#"{"llm":{"model":"claude-sonnet-4-6","provider":"anthropic"}}"#,
            r#"{"status":"ready","platforms":["x"]}"#);
        let state = crate::app_state::AppStateFile::default();
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.llm_model.as_deref(), Some("claude-sonnet-4-6"));
    }

    #[test]
    fn test_pre_populate_does_not_overwrite_llm_model_when_set() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"],"llm_model":"already-set"}"#);
        let state = crate::app_state::AppStateFile::default();
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.llm_model.as_deref(), Some("already-set"), "must not overwrite existing llm_model");
    }

    #[test]
    fn test_pre_populate_sets_created_at_from_now() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        let state = crate::app_state::AppStateFile::default();
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.created_at.as_deref(), Some("2026-05-05T08:00:00+00:00"));
    }

    #[test]
    fn test_pre_populate_does_not_overwrite_created_at_when_set() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"],"created_at":"2026-04-01T00:00:00Z"}"#);
        let state = crate::app_state::AppStateFile::default();
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| None).unwrap();
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.created_at.as_deref(), Some("2026-04-01T00:00:00Z"), "must not overwrite existing created_at");
    }

    #[test]
    fn test_pre_populate_schedule_if_needed_returns_ok_when_meta_exists() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta_path = write_meta(dir.path(), r#"{"status":"ready","platforms":["x"]}"#);
        // pre_populate_schedule_if_needed reads app state from disk — it will read whatever
        // state is present on disk (may have no default_post_time), and should return Ok(()).
        let result = pre_populate_schedule_if_needed(&meta_path);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    }

    #[test]
    fn test_pre_populate_skips_voice_guide_lookup_when_already_set() {
        let dir = tempfile::TempDir::new().unwrap();
        let meta_path = write_meta(dir.path(),
            r#"{"status":"ready","platforms":["x"],"voice_guide_version":"already-set"}"#);
        let state = crate::app_state::AppStateFile::default();
        // When voice_guide_version is already Some, the lookup closure must not be called.
        let called = std::sync::atomic::AtomicBool::new(false);
        pre_populate_with_version_lookup(&meta_path, &state, utc(2026, 5, 5, 8, 0), |_| {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            None
        }).unwrap();
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst),
            "lookup must not be called when voice_guide_version is already set");
        let meta = crate::post_mutations::read_post_meta(&meta_path).unwrap();
        assert_eq!(meta.voice_guide_version.as_deref(), Some("already-set"));
    }

    #[test]
    fn test_apply_schedule_jitter_returns_original_on_invalid_datetime() {
        let bad = "not-a-datetime";
        let result = apply_schedule_jitter(bad, "any-seed");
        assert_eq!(result, bad, "invalid datetime string must be returned unchanged");
    }
}
