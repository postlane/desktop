// SPDX-License-Identifier: BUSL-1.1

/// Validates that `instance` is a plain hostname (no scheme prefix, no injection characters).
/// Accepts optional port suffix (e.g. `mastodon.social:8080`).
/// Call this before any OAuth or HTTP operation that embeds the hostname in a URL.
pub fn validate_instance_hostname(instance: &str) -> Result<(), String> {
    if instance.is_empty() {
        return Err("Instance hostname cannot be empty".to_string());
    }
    if instance.contains("://") {
        return Err(format!(
            "Instance must be a hostname only (e.g. mastodon.social), not a URL. Got: {}",
            instance
        ));
    }
    for ch in ['/', '@', '?', '#', '\n', '\r'] {
        if instance.contains(ch) {
            return Err(format!(
                "Instance hostname contains invalid character {:?}. Use a plain hostname like mastodon.social",
                ch
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_url_with_scheme() {
        assert!(validate_instance_hostname("https://mastodon.social").is_err());
        assert!(validate_instance_hostname("http://mastodon.social").is_err());
        assert!(validate_instance_hostname("ftp://example.com").is_err());
    }

    #[test]
    fn test_accepts_bare_hostname() {
        assert!(validate_instance_hostname("mastodon.social").is_ok());
        assert!(validate_instance_hostname("fosstodon.org").is_ok());
    }

    #[test]
    fn test_accepts_hostname_with_port() {
        assert!(validate_instance_hostname("mastodon.social:8080").is_ok());
    }

    #[test]
    fn test_rejects_empty() {
        assert!(validate_instance_hostname("").is_err());
    }

    #[test]
    fn test_rejects_path_component() {
        assert!(validate_instance_hostname("mastodon.social/evil").is_err());
        assert!(validate_instance_hostname("mastodon.social/../../etc/passwd").is_err());
    }

    #[test]
    fn test_rejects_at_sign() {
        assert!(validate_instance_hostname("user@mastodon.social").is_err());
    }

    #[test]
    fn test_rejects_query_and_fragment() {
        assert!(validate_instance_hostname("mastodon.social?evil=1").is_err());
        assert!(validate_instance_hostname("mastodon.social#evil").is_err());
    }

    #[test]
    fn test_rejects_newline_injection() {
        assert!(validate_instance_hostname("mastodon.social\nevil: injected").is_err());
    }

    #[test]
    fn test_error_message_is_actionable() {
        let err = validate_instance_hostname("https://mastodon.social").unwrap_err();
        assert!(err.contains("hostname"), "error must tell user what to provide: {}", err);
    }
}
