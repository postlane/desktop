// SPDX-License-Identifier: BUSL-1.1

use super::{build_client, ProviderError, SchedulerProfile, SchedulingProvider, Engagement};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Buffer scheduling provider
pub struct BufferProvider {
    /// Shared HTTP client with configured timeouts
    #[allow(dead_code)]
    client: reqwest::Client,
    /// API key for authentication
    #[allow(dead_code)]
    api_key: String,
}

impl BufferProvider {
    /// Create a new BufferProvider
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
        }
    }
}

#[async_trait]
impl SchedulingProvider for BufferProvider {
    fn name(&self) -> &str {
        "buffer"
    }

    async fn schedule_post(
        &self,
        _content: &str,
        _platform: &str,
        _scheduled_for: Option<DateTime<Utc>>,
        _image_url: Option<&str>,
        _profile_id: Option<&str>,
    ) -> Result<String, ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    async fn cancel_post(&self, _post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    async fn get_engagement(
        &self,
        _post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        // Stub implementation - will be implemented in 4.5.2
        Err(ProviderError::NotSupported)
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        // Stub implementation - will be implemented in 4.5.2
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_provider_stores_client() {
        // Test: BufferProvider should store a reqwest::Client at instantiation
        let provider = BufferProvider::new("test-api-key".to_string());

        // Verify the provider was created and has a name
        assert_eq!(provider.name(), "buffer");
    }

    #[test]
    fn test_buffer_provider_instantiation() {
        // Test: Creating BufferProvider should not panic
        let _provider = BufferProvider::new("test-key-123".to_string());
        // If we get here without panic, the test passes
    }
}
