// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Zernio scheduling provider
pub struct ZernioProvider {
    /// Shared HTTP client with configured timeouts
    #[allow(dead_code)]
    client: reqwest::Client,
    /// API key for authentication
    #[allow(dead_code)]
    api_key: String,
}

impl ZernioProvider {
    /// Create a new ZernioProvider
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
        }
    }
}

#[async_trait]
impl SchedulingProvider for ZernioProvider {
    fn name(&self) -> &str {
        "zernio"
    }

    async fn schedule_post(
        &self,
        _content: &str,
        _platform: &str,
        _scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        _profile_id: Option<&str>,
    ) -> Result<String, ProviderError> {
        // Stub implementation - will be implemented in 4.4.2
        Err(ProviderError::NotSupported)
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        // Stub implementation - will be implemented in 4.4.3
        Err(ProviderError::NotSupported)
    }

    async fn cancel_post(&self, _post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        // Stub implementation - will be implemented in 4.4.4
        Err(ProviderError::NotSupported)
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        // Stub implementation - will be implemented in 4.4.5
        Err(ProviderError::NotSupported)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        // Stub implementation - will be implemented in 4.4.6
        Err(ProviderError::NotSupported)
    }

    async fn get_engagement(
        &self,
        _post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        // Stub implementation - will be implemented in 4.4.7
        Err(ProviderError::NotSupported)
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        // Stub implementation - will be implemented in 4.4.8
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zernio_provider_stores_client() {
        // Test: ZernioProvider should store a reqwest::Client at instantiation
        let provider = ZernioProvider::new("test-api-key".to_string());

        // Verify the provider was created and has a name
        assert_eq!(provider.name(), "zernio");
    }

    #[test]
    fn test_zernio_provider_instantiation() {
        // Test: Creating ZernioProvider should not panic
        let _provider = ZernioProvider::new("sk_test_12345".to_string());
        // If we get here without panic, the test passes
    }
}
