// SPDX-License-Identifier: BUSL-1.1

/// Returns a user-facing error string including only the HTTP status code.
/// The raw response body is logged at DEBUG level and never included in the output.
pub fn format_api_error(operation: &str, status: u16, raw_body: &str) -> String {
    log::debug!("{} — raw response body: {}", operation, raw_body);
    format!("{} (HTTP {})", operation, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_api_error_excludes_raw_body() {
        let err = format_api_error("App registration failed", 422, "secret_internal_config:abc123");
        assert!(!err.contains("secret_internal_config"), "must not expose raw body: {}", err);
        assert!(err.contains("422"), "must include HTTP status: {}", err);
    }

    #[test]
    fn test_format_api_error_includes_operation_context() {
        let err = format_api_error("Token exchange failed", 400, "any body");
        assert!(err.starts_with("Token exchange failed"), "must include operation: {}", err);
    }

    #[test]
    fn test_format_api_error_empty_body_is_accepted() {
        let err = format_api_error("some operation", 503, "");
        assert!(err.contains("503"), "must include status with empty body: {}", err);
    }

    #[test]
    fn test_format_api_error_format_is_operation_then_http_status_in_parens() {
        let err = format_api_error("fetch_projects", 429, "rate limited");
        assert_eq!(err, "fetch_projects (HTTP 429)");
    }
}
