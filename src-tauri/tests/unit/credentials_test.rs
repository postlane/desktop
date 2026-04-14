// SPDX-License-Identifier: BUSL-1.1

use postlane_desktop_lib::commands::save_scheduler_credential_impl;

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
            // For now, this will fail because we haven't implemented keyring storage yet
            // In M3 this was stubbed to return Ok(()), but in M4 it needs real implementation
            assert!(result.is_ok(), "Provider {} should be valid", provider);
        }
    }
}
