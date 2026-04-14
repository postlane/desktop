// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::commands::{
    save_scheduler_credential_impl,
    mask_credential,
    delete_scheduler_credential_impl,
    get_scheduler_credential_impl,
    check_libsecret_before_save
};

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[test]
    fn test_save_credential_validates_provider() {
        // Test: Unknown provider should return error
        let result = save_scheduler_credential_impl("invalid-provider", "test-key-123", None);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn test_save_credential_accepts_valid_providers() {
        // Test: Valid providers should be accepted
        let providers = vec!["zernio", "buffer", "ayrshare"];

        for provider in providers {
            let result = save_scheduler_credential_impl(provider, "test-key-123", None);
            assert!(result.is_ok(), "Provider {} should be valid", provider);
        }
    }

    #[test]
    fn test_mask_credential_shows_last_four() {
        // Test: Credential masking shows ••••••••{last4}
        let credential = "sk_test_1234567890abcdef";
        let masked = mask_credential(credential);

        assert_eq!(masked, "••••••••cdef");
    }

    #[test]
    fn test_mask_credential_short_credential() {
        // Test: Credentials shorter than 4 chars are fully masked
        let credential = "abc";
        let masked = mask_credential(credential);

        assert_eq!(masked, "••••••••");
    }

    #[test]
    fn test_mask_credential_exactly_four_chars() {
        // Test: Exactly 4 chars shows last 4
        let credential = "1234";
        let masked = mask_credential(credential);

        assert_eq!(masked, "••••••••1234");
    }

    #[test]
    fn test_delete_credential_validates_provider() {
        // Test: Unknown provider should return error
        let result = delete_scheduler_credential_impl("invalid-provider");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn test_delete_credential_accepts_valid_providers() {
        // Test: Valid providers should be accepted
        let providers = vec!["zernio", "buffer", "ayrshare"];

        for provider in providers {
            let result = delete_scheduler_credential_impl(provider);
            assert!(result.is_ok(), "Provider {} should be valid", provider);
        }
    }

    #[test]
    fn test_get_credential_validates_provider() {
        // Test: Unknown provider should return error
        let result = get_scheduler_credential_impl("invalid-provider");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn test_get_credential_accepts_valid_providers() {
        // Test: Valid providers should be accepted
        let providers = vec!["zernio", "buffer", "ayrshare"];

        for provider in providers {
            let result = get_scheduler_credential_impl(provider);
            assert!(result.is_ok(), "Provider {} should be valid", provider);
        }
    }

    #[test]
    fn test_check_libsecret_before_save_when_unavailable() {
        // Test: Should return error when libsecret is unavailable
        let result = check_libsecret_before_save(Some(false));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("libsecret not available"));
    }

    #[test]
    fn test_check_libsecret_before_save_when_available() {
        // Test: Should return Ok when libsecret is available
        let result = check_libsecret_before_save(Some(true));

        assert!(result.is_ok());
    }

    #[test]
    fn test_check_libsecret_before_save_when_not_checked() {
        // Test: Should return Ok when not checked yet (None) - will check on first use
        let result = check_libsecret_before_save(None);

        assert!(result.is_ok());
    }

    #[test]
    fn test_save_credential_checks_libsecret_flag() {
        // Test: save_scheduler_credential_impl should call check_libsecret_before_save
        // and return error if libsecret is unavailable

        // Case 1: libsecret unavailable - should return error
        let result = save_scheduler_credential_impl("zernio", "test-key", Some(false));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("libsecret not available"));

        // Case 2: libsecret available - should succeed
        let result = save_scheduler_credential_impl("zernio", "test-key", Some(true));
        assert!(result.is_ok());

        // Case 3: not checked yet (None) - should succeed
        let result = save_scheduler_credential_impl("zernio", "test-key", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_credential_keyring_key_format() {
        use postlane_desktop_lib::commands::get_credential_keyring_key;

        // Test: Per-repo override key should be checked first
        // Format: postlane/{provider}/{repo_id} then postlane/{provider}

        // Case 1: With repo_id - should return per-repo key first
        let keys = get_credential_keyring_key("zernio", Some("repo-123"));
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], "zernio/repo-123");
        assert_eq!(keys[1], "zernio");

        // Case 2: Without repo_id - should only return global key
        let keys = get_credential_keyring_key("buffer", None);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "buffer");
    }

    #[test]
    fn test_get_credential_impl_validates_provider() {
        use postlane_desktop_lib::commands::get_scheduler_credential_impl;

        // Test: get_scheduler_credential_impl should validate provider
        let result = get_scheduler_credential_impl("invalid-provider");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));

        // Valid providers should pass validation
        let result = get_scheduler_credential_impl("zernio");
        assert!(result.is_ok());
    }
}
