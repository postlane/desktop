// SPDX-License-Identifier: BUSL-1.1

/// Classification of an incoming `postlane://` deep link URL by host+path.
/// Query strings are never included in any variant — callers must not log the raw URL.
#[derive(Debug, PartialEq)]
pub enum DeepLinkPath {
    /// `postlane://activate` — handled by license activation flow.
    Activate,
    /// `postlane://draft` — stub for v2 weekly-review feature.
    Draft,
    /// `postlane://account-updated` — stub for v2 "Connected accounts" Disconnect
    /// flow (checklist 24.1.6a); the actual refetch-and-rerender lands with the
    /// Settings -- Account UI in Workstream 1/24.4, not yet built.
    AccountUpdated,
    /// `postlane://oauth/callback` — stub for v3 OAuth flow.
    OauthCallback,
    /// `postlane://billing-complete` — Stripe Checkout/Portal return redirect
    /// (24.4.2/24.4.4, both success and cancel land here since Stripe is given
    /// the same URL for both); triggers a workspace-status refresh in the
    /// Settings — Account tab (24.4.9/24.4.5a).
    BillingComplete,
    /// Any other host/path — logged at `warn!` level, no action taken.
    Unknown { path: String },
}

/// Classifies a `postlane://` URL by host and path, ignoring the query string.
/// Returns `Unknown` for non-`postlane` schemes.
pub fn classify(url: &str) -> DeepLinkPath {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return DeepLinkPath::Unknown { path: url.to_owned() },
    };

    if parsed.scheme() != "postlane" {
        return DeepLinkPath::Unknown { path: String::new() };
    }

    let host = parsed.host_str().unwrap_or("");
    let path = parsed.path().trim_matches('/');

    match (host, path) {
        ("activate", _) => DeepLinkPath::Activate,
        ("draft", "") => DeepLinkPath::Draft,
        ("account-updated", "") => DeepLinkPath::AccountUpdated,
        ("oauth", "callback") => DeepLinkPath::OauthCallback,
        ("billing-complete", "") => DeepLinkPath::BillingComplete,
        _ => {
            let full = if path.is_empty() {
                host.to_owned()
            } else {
                format!("{}/{}", host, path)
            };
            DeepLinkPath::Unknown { path: full }
        }
    }
}

/// Extracts a valid GitHub App installation ID from `postlane://oauth/callback?installation_id=...`.
/// Returns `None` if the parameter is absent, non-numeric, or zero.
pub fn installation_id_from_url(url: &str) -> Option<u64> {
    let parsed = url::Url::parse(url).ok()?;
    let id_str = parsed.query_pairs().find(|(k, _)| k == "installation_id")?.1;
    let id: u64 = id_str.parse().ok()?;
    if id == 0 { None } else { Some(id) }
}

/// Extracts `project_id` from `postlane://billing-complete?project_id=...`.
/// Returns `None` if the parameter is absent or empty.
pub fn billing_project_id_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let id = parsed.query_pairs().find(|(k, _)| k == "project_id")?.1.into_owned();
    if id.is_empty() { None } else { Some(id) }
}

/// Extracts the first `postlane://` URL from a list of process arguments.
/// Used by the single-instance callback to re-dispatch a deep link that arrived
/// as a process argument in the second instance (Windows path).
pub fn deep_link_from_args(args: &[String]) -> Option<String> {
    args.iter().find(|a| a.starts_with("postlane://")).cloned()
}

/// Returns a log-safe representation of a deep link URL: `scheme://host/path` only.
/// The query string and fragment are never included.
pub fn log_safe_url(url: &str) -> String {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return String::new(),
    };
    let path = parsed.path().trim_matches('/');
    if path.is_empty() {
        format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""))
    } else {
        format!("{}://{}/{}", parsed.scheme(), parsed.host_str().unwrap_or(""), path)
    }
}

/// A `DeepLinkPath` reduced to a frontend-consumable shape: a stable string
/// tag (never the Rust variant name, which is free to change) plus whatever
/// per-kind data the frontend needs to react — currently only `project_id`
/// for `billing_complete`.
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct ClassifiedDeepLink {
    pub kind: String,
    pub project_id: Option<String>,
}

fn classify_deep_link_impl(url: &str) -> ClassifiedDeepLink {
    match classify(url) {
        DeepLinkPath::BillingComplete => ClassifiedDeepLink {
            kind: "billing_complete".to_string(),
            project_id: billing_project_id_from_url(url),
        },
        DeepLinkPath::Activate => ClassifiedDeepLink { kind: "activate".to_string(), project_id: None },
        DeepLinkPath::Draft => ClassifiedDeepLink { kind: "draft".to_string(), project_id: None },
        DeepLinkPath::AccountUpdated => {
            ClassifiedDeepLink { kind: "account_updated".to_string(), project_id: None }
        }
        DeepLinkPath::OauthCallback => {
            ClassifiedDeepLink { kind: "oauth_callback".to_string(), project_id: None }
        }
        DeepLinkPath::Unknown { .. } => ClassifiedDeepLink { kind: "unknown".to_string(), project_id: None },
    }
}

/// Frontend entry point for the deep-link plugin's `deep-link://new-url` event:
/// classifies a raw URL so the frontend can dispatch on `kind` without
/// duplicating the host/path matching logic in TypeScript.
#[tauri::command]
pub fn classify_deep_link(url: String) -> ClassifiedDeepLink {
    classify_deep_link_impl(&url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_link_handler_registered() {
        // No panic — bare `postlane://` is classified without crashing.
        let result = classify("postlane://");
        assert!(matches!(result, DeepLinkPath::Unknown { .. }));
    }

    #[test]
    fn test_deep_link_handler_warns_on_unknown_path() {
        let result = classify("postlane://unknown-path-xyz");
        assert_eq!(result, DeepLinkPath::Unknown { path: "unknown-path-xyz".to_owned() });
    }

    #[test]
    fn test_deep_link_handler_does_not_log_query_params() {
        let safe = log_safe_url("postlane://oauth/callback?code=SECRET");
        assert!(!safe.contains("SECRET"), "log_safe_url must not include query params");
        assert_eq!(safe, "postlane://oauth/callback");
    }

    #[test]
    fn test_classify_activate() {
        assert_eq!(classify("postlane://activate?token=abc.def.ghi"), DeepLinkPath::Activate);
    }

    #[test]
    fn test_classify_draft() {
        assert_eq!(classify("postlane://draft"), DeepLinkPath::Draft);
    }

    #[test]
    fn test_classify_account_updated() {
        assert_eq!(classify("postlane://account-updated"), DeepLinkPath::AccountUpdated);
    }

    #[test]
    fn test_classify_oauth_callback() {
        assert_eq!(classify("postlane://oauth/callback"), DeepLinkPath::OauthCallback);
        assert_eq!(classify("postlane://oauth/callback?code=xyz"), DeepLinkPath::OauthCallback);
    }

    #[test]
    fn test_classify_billing_complete() {
        assert_eq!(
            classify("postlane://billing-complete?project_id=proj-1"),
            DeepLinkPath::BillingComplete
        );
    }

    // ── billing_project_id_from_url ──────────────────────────────────────────

    #[test]
    fn test_billing_project_id_from_url_extracts_id() {
        let id = billing_project_id_from_url("postlane://billing-complete?project_id=proj-1");
        assert_eq!(id, Some("proj-1".to_string()));
    }

    #[test]
    fn test_billing_project_id_from_url_returns_none_when_missing() {
        let id = billing_project_id_from_url("postlane://billing-complete");
        assert_eq!(id, None);
    }

    #[test]
    fn test_billing_project_id_from_url_returns_none_when_empty() {
        let id = billing_project_id_from_url("postlane://billing-complete?project_id=");
        assert_eq!(id, None);
    }

    // ── classify_deep_link command ────────────────────────────────────────────

    #[test]
    fn test_classify_deep_link_billing_complete_includes_project_id() {
        let result = classify_deep_link_impl("postlane://billing-complete?project_id=proj-1");
        assert_eq!(result.kind, "billing_complete");
        assert_eq!(result.project_id, Some("proj-1".to_string()));
    }

    #[test]
    fn test_classify_deep_link_activate_has_no_project_id() {
        let result = classify_deep_link_impl("postlane://activate?token=abc");
        assert_eq!(result.kind, "activate");
        assert_eq!(result.project_id, None);
    }

    #[test]
    fn test_classify_deep_link_unknown_path() {
        let result = classify_deep_link_impl("postlane://unknown-path-xyz");
        assert_eq!(result.kind, "unknown");
    }

    #[test]
    fn test_log_safe_url_strips_query_from_activate() {
        let safe = log_safe_url("postlane://activate?token=SECRET.JWT.TOKEN");
        assert!(!safe.contains("SECRET"), "token must not appear in log");
        assert_eq!(safe, "postlane://activate");
    }

    #[test]
    fn test_installation_id_from_url_parses_valid_id() {
        let id = installation_id_from_url("postlane://oauth/callback?installation_id=12345678");
        assert_eq!(id, Some(12345678));
    }

    #[test]
    fn test_installation_id_from_url_returns_none_when_param_missing() {
        let id = installation_id_from_url("postlane://oauth/callback");
        assert_eq!(id, None);
    }

    #[test]
    fn test_installation_id_from_url_returns_none_for_non_numeric() {
        let id = installation_id_from_url("postlane://oauth/callback?installation_id=abc");
        assert_eq!(id, None);
    }

    #[test]
    fn test_installation_id_from_url_returns_none_for_zero() {
        let id = installation_id_from_url("postlane://oauth/callback?installation_id=0");
        assert_eq!(id, None);
    }

    #[test]
    fn test_installation_id_from_url_ignores_other_params() {
        let id = installation_id_from_url("postlane://oauth/callback?setup_action=install&installation_id=99001122");
        assert_eq!(id, Some(99001122));
    }

    // ── deep_link_from_args ───────────────────────────────────────────────────

    #[test]
    fn test_deep_link_from_args_returns_url_when_present() {
        let args = vec![
            "postlane".to_string(),
            "postlane://oauth/callback?installation_id=12345678".to_string(),
        ];
        assert_eq!(
            deep_link_from_args(&args),
            Some("postlane://oauth/callback?installation_id=12345678".to_string())
        );
    }

    #[test]
    fn test_deep_link_from_args_returns_none_when_absent() {
        let args = vec!["postlane".to_string(), "--defaults".to_string()];
        assert_eq!(deep_link_from_args(&args), None);
    }

    #[test]
    fn test_deep_link_from_args_handles_empty_slice() {
        assert_eq!(deep_link_from_args(&[]), None);
    }

    #[test]
    fn test_deep_link_from_args_ignores_non_postlane_args() {
        let args = vec![
            "/usr/bin/postlane".to_string(),
            "https://example.com".to_string(),
            "file:///some/path".to_string(),
        ];
        assert_eq!(deep_link_from_args(&args), None);
    }

    #[test]
    fn test_deep_link_from_args_returns_first_when_multiple_present() {
        let args = vec![
            "postlane".to_string(),
            "postlane://activate?token=abc".to_string(),
            "postlane://oauth/callback?installation_id=99".to_string(),
        ];
        assert_eq!(
            deep_link_from_args(&args),
            Some("postlane://activate?token=abc".to_string())
        );
    }
}
