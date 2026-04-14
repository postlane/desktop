// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::commands::{save_scheduler_credential_impl, mask_credential, delete_scheduler_credential_impl};

#[cfg(test)]
mod credential_tests {
    use super::*;

    #[test]
    fn test_save_credential_validates_provider() {
        // Test: Unknown provider should return error
        let result = save_scheduler_credential_impl("invalid-provider", "test-key-123");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown provider"));
    }

    #[test]
    fn test_save_credential_accepts_valid_providers() {
        // Test: Valid providers should be accepted
        let providers = vec!["zernio", "buffer", "ayrshare"];

        for provider in providers {
            let result = save_scheduler_credential_impl(provider, "test-key-123");
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
}
