// SPDX-License-Identifier: BUSL-1.1

use crate::scheduling::usage_tracker;
use std::path::Path;

/// A resolved scheduler credential selected after applying the fallback chain.
#[derive(Debug, Clone, PartialEq)]
pub struct SchedulerCredential {
    pub provider: String,
    pub api_key: String,
}

/// Reads the provider fallback order from a `config.json` file.
/// Uses `scheduler.fallback_order` when present; otherwise returns `[scheduler.provider]`.
/// Returns an empty vec if the config is missing or cannot be parsed.
pub(crate) fn read_fallback_order(config_path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(config_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let config: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    if let Some(arr) = config["scheduler"]["fallback_order"].as_array() {
        let order: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        if !order.is_empty() {
            return order;
        }
    }
    if let Some(p) = config["scheduler"]["provider"].as_str() {
        return vec![p.to_string()];
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
        publer::PublerProvider, substack_notes::SubstackNotesProvider, webhook::WebhookProvider,
        zernio::ZernioProvider,
    };
    match name {
        "zernio" => Ok(Box::new(ZernioProvider::new(api_key))),
        "buffer" => Ok(Box::new(BufferProvider::new(api_key))),
        "ayrshare" => Ok(Box::new(AyrshareProvider::new(api_key))),
        "publer" => Ok(Box::new(PublerProvider::new(api_key))),
        "outstand" => Ok(Box::new(OutstandProvider::new(api_key))),
        "substack_notes" => Ok(Box::new(SubstackNotesProvider::new(api_key))),
        "webhook" => Ok(Box::new(WebhookProvider::new(api_key))),
        _ => Err(format!("Unknown provider: {}", name)),
    }
}

/// Returns the first eligible scheduler credential for `repo_id`, applying the
/// fallback chain from `.postlane/config.json`.
pub async fn get_scheduler_credential_with_fallback(
    config_path: &Path,
    repo_id: &str,
    app: &tauri::AppHandle,
) -> Result<SchedulerCredential, String> {
    use chrono::{Datelike, Utc};
    use tauri_plugin_keyring::KeyringExt;

    let providers = read_fallback_order(config_path);
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
        let keys =
            crate::scheduler_credentials::get_credential_keyring_key(provider, Some(&repo_id));
        for key in keys {
            if let Ok(Some(cred)) = app.keyring().get_password("postlane", &key) {
                return Some(cred);
            }
        }
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::usage_tracker::{get_known_limit, record_post_at};
    use std::fs;
    use std::path::PathBuf;

    fn temp_usage(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("postlane_cr_{}", name));
        fs::create_dir_all(&dir).expect("create dir");
        dir.join("scheduler_usage.json")
    }

    fn cleanup(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    fn cred_for_all(provider: &str) -> Option<String> {
        Some(format!("key-{}", provider))
    }

    /// §13.4.2 — primary exhausted; fallback provider is returned
    #[test]
    fn test_fallback_skips_exhausted_provider() {
        let usage = temp_usage("skip_exhausted");
        let limit = get_known_limit("publer").expect("publer has a limit");
        for _ in 0..limit {
            record_post_at("publer", &usage, 4, 2026).expect("record");
        }

        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_ok(), "should fall back to zernio");
        assert_eq!(result.unwrap().provider, "zernio");

        cleanup(&usage);
    }

    /// §13.4.3 — all providers exhausted returns error
    #[test]
    fn test_all_providers_exhausted_returns_error() {
        let usage = temp_usage("all_exhausted");
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

        cleanup(&usage);
    }

    /// §13.4.4 — fallback_order is respected; first provider with capacity wins
    #[test]
    fn test_fallback_order_respects_config() {
        let usage = temp_usage("order");

        // Both have capacity; Publer is first in the list
        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, cred_for_all);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider, "publer", "first in list must win");

        cleanup(&usage);
    }

    #[test]
    fn test_no_credential_skips_provider() {
        let usage = temp_usage("no_cred");
        let providers = vec!["publer".to_string(), "zernio".to_string()];
        let result = select_provider_with_fallback(&providers, &usage, 4, 2026, |p| {
            if p == "zernio" { Some("key".to_string()) } else { None }
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider, "zernio");
        cleanup(&usage);
    }

    #[test]
    fn test_empty_provider_list_returns_error() {
        let usage = temp_usage("empty");
        let result = select_provider_with_fallback(&[], &usage, 4, 2026, cred_for_all);
        assert!(result.is_err());
        cleanup(&usage);
    }

    #[test]
    fn test_read_fallback_order_uses_fallback_order_field() {
        let dir = std::env::temp_dir().join("postlane_cr_fbo");
        fs::create_dir_all(&dir).expect("create dir");
        let config_path = dir.join("config.json");
        let config = serde_json::json!({
            "scheduler": {
                "provider": "publer",
                "fallback_order": ["publer", "zernio", "webhook"]
            }
        });
        fs::write(&config_path, config.to_string()).expect("write");
        assert_eq!(
            read_fallback_order(&config_path),
            vec!["publer", "zernio", "webhook"]
        );
        let _ = fs::remove_dir_all(&dir);
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

    #[test]
    fn test_read_fallback_order_falls_back_to_single_provider() {
        let dir = std::env::temp_dir().join("postlane_cr_single");
        fs::create_dir_all(&dir).expect("create dir");
        let config_path = dir.join("config.json");
        let config = serde_json::json!({"scheduler": {"provider": "zernio"}});
        fs::write(&config_path, config.to_string()).expect("write");
        assert_eq!(read_fallback_order(&config_path), vec!["zernio"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
