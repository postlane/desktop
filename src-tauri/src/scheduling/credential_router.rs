// SPDX-License-Identifier: BUSL-1.1

use crate::scheduling::usage_tracker;
use std::path::Path;

/// A resolved scheduler credential selected after applying the fallback chain.
#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerCredential {
    pub provider: String,
    pub api_key: String,
}

/// Reads the provider fallback order from a parsed config value.
/// Uses `scheduler.fallback_order` when present; otherwise returns `[scheduler.provider]`.
/// Empty strings are filtered out. Returns an empty vec if nothing is configured.
pub(crate) fn read_fallback_order_from_value(config: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = config["scheduler"]["fallback_order"].as_array() {
        let order: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        if !order.is_empty() {
            return order;
        }
    }
    if let Some(p) = config["scheduler"]["provider"].as_str() {
        if !p.is_empty() {
            return vec![p.to_string()];
        }
    }
    vec![]
}


/// Selects the first non-exhausted provider from `providers` that has a configured credential.
///
/// Skips a provider when either:
/// - it is at 100% of its known free-tier limit, or
/// - `get_credential` returns `None` for it.
///
/// Returns `Err` with a user-facing message when no eligible provider is found.
pub(crate) fn select_provider_with_fallback<F>(
    providers: &[String],
    usage_path: &Path,
    month: u32,
    year: u32,
    get_credential: F,
) -> Result<SchedulerCredential, String>
where
    F: Fn(&str) -> Option<String>,
{
    for provider in providers {
        if usage_tracker::is_at_limit_at(provider, usage_path, month, year) {
            continue;
        }
        if let Some(api_key) = get_credential(provider) {
            if build_provider(provider, String::new()).is_err() {
                continue; // provider saved but not yet implemented — skip silently
            }
            return Ok(SchedulerCredential {
                provider: provider.clone(),
                api_key,
            });
        }
    }
    Err(
        "All configured schedulers have reached their limits or have no credentials. \
         Check Settings \u{2192} Scheduler to add capacity or upgrade a provider."
            .to_string(),
    )
}

/// Constructs a boxed `SchedulingProvider` from a provider name and API key.
pub fn build_provider(
    name: &str,
    api_key: String,
) -> Result<Box<dyn crate::providers::scheduling::SchedulingProvider>, String> {
    use crate::providers::scheduling::{
        ayrshare::AyrshareProvider, buffer::BufferProvider, outstand::OutstandProvider,
        publer::PublerProvider, substack_notes::SubstackNotesProvider,
        upload_post::UploadPostProvider, webhook::WebhookProvider, zernio::ZernioProvider,
    };
    match name {
        "zernio" => Ok(Box::new(ZernioProvider::new(api_key))),
        "buffer" => Ok(Box::new(BufferProvider::new(api_key))),
        "ayrshare" => Ok(Box::new(AyrshareProvider::new(api_key))),
        "publer" => Ok(Box::new(PublerProvider::new(api_key))),
        "outstand" => Ok(Box::new(OutstandProvider::new(api_key))),
        "substack_notes" => Ok(Box::new(SubstackNotesProvider::new(api_key))),
        "upload_post" => Ok(Box::new(UploadPostProvider::new(api_key))),
        "webhook" => Ok(Box::new(WebhookProvider::new(api_key))),
        _ => Err(format!("Unknown provider: {}", name)),
    }
}

/// Returns the first eligible scheduler credential for `repo_id`, applying the
/// fallback chain from the merged `.postlane/config.json` + `.postlane/config.local.json`.
pub async fn get_scheduler_credential_with_fallback(
    repo_path: &Path,
    repo_id: &str,
    app: &tauri::AppHandle,
) -> Result<SchedulerCredential, String> {
    use chrono::{Datelike, Utc};
    use tauri_plugin_keyring::KeyringExt;

    let config = crate::config_merge::read_merged_repo_config(repo_path)
        .map_err(|e| format!("Could not read scheduler config: {}", e))?;
    let providers = read_fallback_order_from_value(&config);
    if providers.is_empty() {
        return Err(
            "No scheduler configured. Add one in Settings \u{2192} Scheduler.".to_string(),
        );
    }

    let usage_path = usage_tracker::default_store_path()?;
    let now = Utc::now();
    let month = now.month();
    let year = now.year() as u32;
    let app = app.clone();
    let repo_id = repo_id.to_string();

    select_provider_with_fallback(&providers, &usage_path, month, year, move |provider| {
        let key =
            crate::scheduler_credentials::get_credential_keyring_key(provider, &repo_id);
        if let Ok(Some(cred)) = app.keyring().get_password("postlane", &key) {
            return Some(cred);
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::usage_tracker::{get_known_limit, record_post_at};
    use std::path::PathBuf;

    fn temp_usage(_name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let path = dir.path().join("scheduler_usage.json");
        (dir, path)
    }

    fn cred_for_all(provider: &str) -> Option<String> {
        Some(format!("key-{}", provider))
    }

    /// §13.4.2 — primary exhausted; fallback provider is returned
    #[test]
    fn test_fallback_skips_exhausted_provider() {
        let (_dir, usage) = temp_usage("skip_exhausted");
        let limit = get_known_limit("publer").expect("publer has a limit");
        for _ in 0..limit {
            record_post_at("publer", &usage, 4, 2026).expect("record");
        }

        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_ok(), "should fall back to zernio");
        assert_eq!(result.unwrap().provider, "zernio");
    }

    /// §13.4.3 — all providers exhausted returns error
    #[test]
    fn test_all_providers_exhausted_returns_error() {
        let (_dir, usage) = temp_usage("all_exhausted");
        let publer_limit = get_known_limit("publer").expect("publer has a limit");
        let webhook_limit = get_known_limit("webhook").expect("webhook has a limit");
        for _ in 0..publer_limit {
            record_post_at("publer", &usage, 4, 2026).expect("record publer");
        }
        for _ in 0..webhook_limit {
            record_post_at("webhook", &usage, 4, 2026).expect("record webhook");
        }

        let providers = vec!["publer".to_string(), "webhook".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_err(), "all exhausted must return Err");
        assert!(
            result.unwrap_err().to_lowercase().contains("limits"),
            "error must mention limits"
        );
    }

    /// §13.4.4 — fallback_order is respected; first provider with capacity wins
    #[test]
    fn test_fallback_order_respects_config() {
        let (_dir, usage) = temp_usage("order");

        // Both have capacity; Publer is first in the list
        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider, "publer", "first in list must win");
    }

    #[test]
    fn test_no_credential_skips_provider() {
        let (_dir, usage) = temp_usage("no_cred");
        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, |p| {
            if p == "zernio" { Some("key".to_string()) } else { None }
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider, "zernio");
    }

    #[test]
    fn test_empty_provider_list_returns_error() {
        let (_dir, usage) = temp_usage("empty");
        let result = select_provider_with_fallback(&[], &usage, 4, 2026, cred_for_all);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_fallback_order_from_value_uses_fallback_order_field() {
        let config = serde_json::json!({
            "scheduler": { "provider": "publer", "fallback_order": ["publer", "zernio"] }
        });
        assert_eq!(
            read_fallback_order_from_value(&config),
            vec!["publer", "zernio"]
        );
    }

    #[test]
    fn test_read_fallback_order_from_value_falls_back_to_single_provider() {
        let config = serde_json::json!({"scheduler": {"provider": "zernio"}});
        assert_eq!(read_fallback_order_from_value(&config), vec!["zernio"]);
    }

    #[test]
    fn test_read_fallback_order_from_value_filters_empty_provider() {
        let config = serde_json::json!({"scheduler": {"provider": ""}});
        assert!(
            read_fallback_order_from_value(&config).is_empty(),
            "empty provider string must not appear in fallback list"
        );
    }

    #[test]
    fn test_build_provider_returns_provider_for_known_name() {
        let result = build_provider("zernio", "test-key".to_string());
        assert!(result.is_ok(), "zernio must be recognised");
        assert_eq!(result.unwrap().name(), "zernio");
    }

    #[test]
    fn test_build_provider_errors_on_unknown_name() {
        let result = build_provider("unknown-xyz", "key".to_string());
        let err = result.err().expect("should be Err");
        assert!(err.contains("Unknown provider"), "got: {}", err);
    }

    /// Providers not yet in build_provider's match arms must be skipped silently.
    #[test]
    fn test_unimplemented_provider_skipped_falls_back_to_next() {
        let (_dir, usage) = temp_usage("unimplemented");
        // "future_provider" is not in build_provider's match arms
        let providers = vec!["future_provider".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_ok(), "must fall back past unimplemented provider");
        assert_eq!(result.unwrap().provider, "zernio");
    }

    #[test]
    fn test_all_unimplemented_providers_returns_error() {
        let (_dir, usage) = temp_usage("all_unimplemented");
        let providers = vec!["future_provider".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_err());
    }

    /// Broken config.local.json now propagates a meaningful error (via map_err + ?)
    /// rather than silently becoming an empty config that masquerades as "not configured".
    #[test]
    fn test_broken_config_local_propagates_error() {
        use std::fs;
        let dir = tempfile::TempDir::new().expect("create temp dir");
        let postlane = dir.path().join(".postlane");
        fs::create_dir_all(&postlane).unwrap();
        fs::write(postlane.join("config.json"), r#"{"version":1}"#).unwrap();
        fs::write(postlane.join("config.local.json"), "{ broken json").unwrap();
        let result = crate::config_merge::read_merged_repo_config(dir.path());
        assert!(result.is_err(), "broken config.local.json must return Err");
        let err = result.unwrap_err();
        assert!(err.contains("config.local.json") || err.contains("parse"),
            "error must describe the parse failure, got: {}", err);
    }

}
