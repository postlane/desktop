// SPDX-License-Identifier: BUSL-1.1
//
// Upload-Post provider (https://upload-post.com)
//
// Auth:     Authorization: Apikey {key}  (NOT Bearer)
// Schedule: POST https://api.upload-post.com/api/upload_text
//           Required body fields: user (username), platform[] (array), title (content)
//           Optional: scheduled_date (ISO-8601 UTC), timezone (IANA)
// 202 response (scheduled): { "success": true, "job_id": "..." }
// 200 response (immediate): { "success": true, "request_id": "...", "results": {...} }
// 429: monthly quota exceeded (free tier = 10/month) — not a per-hour rate limit
// The `user` field is the Upload-Post connected-account username, resolved via
// list_profiles() → GET /api/uploadposts/users; never entered manually by the user.

use super::{
    build_client, parse_retry_after, with_retry, Engagement, PostScheduleResult, ProviderError,
    SchedulerProfile, SchedulingProvider,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

const BASE_URL: &str = "https://api.upload-post.com/api";

pub struct UploadPostProvider {
    client: reqwest::Client,
    api_key: String,
    #[cfg(test)]
    pub(crate) base_url: String,
}

impl UploadPostProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: build_client(),
            api_key,
            #[cfg(test)]
            base_url: BASE_URL.to_string(),
        }
    }

    #[cfg(not(test))]
    fn base_url(&self) -> &str {
        BASE_URL
    }

    #[cfg(test)]
    fn base_url(&self) -> &str {
        &self.base_url
    }

    fn auth_header(&self) -> String {
        format!("Apikey {}", self.api_key)
    }

    fn check_status(response: &reqwest::Response) -> Result<(), ProviderError> {
        match response.status().as_u16() {
            401 => Err(ProviderError::AuthError("Invalid API key".to_string())),
            429 => Err(ProviderError::RateLimit(parse_retry_after(response))),
            _ => Ok(()),
        }
    }

    fn parse_scheduler_id(json: &serde_json::Value) -> Result<String, ProviderError> {
        json["job_id"]
            .as_str()
            .or_else(|| json["request_id"].as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                ProviderError::Unknown(format!(
                    "Missing job_id or request_id in response: {}",
                    json
                ))
            })
    }

    fn parse_platform_list(json: &serde_json::Value) -> Vec<String> {
        if let Some(arr) = json["social_accounts"].as_array() {
            return arr
                .iter()
                .filter_map(|a| a["platform"].as_str().map(str::to_string))
                .collect();
        }
        json["platforms"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    }

    pub async fn validate_profile(&self, username: &str) -> Result<Vec<String>, ProviderError> {
        let url = format!("{}/uploadposts/users/{}", self.base_url(), username);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_status(&resp)?;
        if resp.status().as_u16() == 404 {
            return Err(ProviderError::HttpError {
                status: 404,
                body: format!(
                    "Username '{}' not found. Usernames are case-sensitive.",
                    username
                ),
            });
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
        log::debug!("UploadPost validate_profile response: {}", json);
        Ok(Self::parse_platform_list(&json))
    }

    fn parse_profiles(json: &serde_json::Value) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let arr = json["profiles"].as_array().ok_or_else(|| {
            ProviderError::Unknown(format!("Missing profiles array in response: {}", json))
        })?;
        Ok(arr
            .iter()
            .filter_map(|p| {
                let id = p["username"].as_str().or_else(|| p["id"].as_str())?.to_string();
                let name = id.clone();
                let platforms: Vec<String> = p["platforms"]
                    .as_array()
                    .map(|v| v.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
                    .unwrap_or_default();
                Some(SchedulerProfile { id, name, platforms })
            })
            .collect())
    }
}

#[async_trait]
impl SchedulingProvider for UploadPostProvider {
    fn name(&self) -> &str {
        "upload_post"
    }

    async fn schedule_post(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: Option<&str>,
        profile_id: Option<&str>,
    ) -> Result<PostScheduleResult, ProviderError> {
        let user = profile_id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ProviderError::AuthError(
                    "Upload Post requires a connected account username. \
                     Set one in Settings \u{2192} Accounts."
                        .to_string(),
                )
            })?;
        match image_url {
            Some(url) => self.schedule_with_image(content, platform, scheduled_for, url, user).await,
            None => self.schedule_text_only(content, platform, scheduled_for, user).await,
        }
    }

    async fn list_profiles(&self) -> Result<Vec<SchedulerProfile>, ProviderError> {
        let url = format!("{}/uploadposts/users", self.base_url());
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_status(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
        log::debug!("UploadPost list_profiles response: {}", json);
        Self::parse_profiles(&json)
    }

    async fn cancel_post(&self, post_id: &str, _platform: &str) -> Result<(), ProviderError> {
        let url = format!("{}/uploadposts/schedule/{}", self.base_url(), post_id);
        let resp = self
            .client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_status(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        Ok(())
    }

    async fn get_queue(&self) -> Result<Vec<crate::types::QueuedPost>, ProviderError> {
        Err(ProviderError::NotSupported(
            "Upload Post does not expose a queue listing endpoint".to_string(),
        ))
    }

    async fn test_connection(&self) -> Result<(), ProviderError> {
        let url = format!("{}/uploadposts/me", self.base_url());
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_status(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        Ok(())
    }

    async fn get_engagement(
        &self,
        post_id: &str,
        _platform: &str,
    ) -> Result<Engagement, ProviderError> {
        let url = format!("{}/uploadposts/post-analytics/{}", self.base_url(), post_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        Self::check_status(&resp)?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
        let m = &json["post_metrics"];
        Ok(Engagement {
            likes: m["likes"].as_u64().unwrap_or(0),
            reposts: m["shares"].as_u64().unwrap_or(0),
            replies: m["comments"].as_u64().unwrap_or(0),
            impressions: m["views"].as_u64(),
            platform_url: None,
        })
    }

    fn post_url(&self, _platform: &str, _post_id: &str) -> Option<String> {
        None
    }
}

impl UploadPostProvider {
    async fn schedule_text_only(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        user: &str,
    ) -> Result<PostScheduleResult, ProviderError> {
        let user_owned = user.to_string();
        let platform_owned = platform.to_string();
        let content_owned = content.to_string();
        with_retry(
            || async {
                let mut form = reqwest::multipart::Form::new()
                    .text("user", user_owned.clone())
                    .text("platform[]", platform_owned.clone())
                    .text("title", content_owned.clone());
                if let Some(dt) = scheduled_for {
                    form = form.text("scheduled_date", dt.to_rfc3339());
                }
                let url = format!("{}/upload_text", self.base_url());
                let resp = self
                    .client
                    .post(&url)
                    .header("Authorization", self.auth_header())
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
                Self::check_status(&resp)?;
                if !resp.status().is_success() {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(ProviderError::HttpError { status, body });
                }
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
                log::debug!("UploadPost schedule_text_only response: {}", json);
                let scheduler_id = Self::parse_scheduler_id(&json)?;
                Ok(PostScheduleResult { scheduler_id, platform_url: None })
            },
            3,
        )
        .await
    }

    async fn download_image_bytes(&self, url: &str) -> Result<(Vec<u8>, String), ProviderError> {
        let resp = self.client.get(url).send()
            .await.map_err(|e| ProviderError::NetworkError(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::HttpError { status, body });
        }
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp.bytes().await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?.to_vec();
        Ok((bytes, content_type))
    }

    async fn schedule_with_image(
        &self,
        content: &str,
        platform: &str,
        scheduled_for: Option<DateTime<Utc>>,
        image_url: &str,
        user: &str,
    ) -> Result<PostScheduleResult, ProviderError> {
        let (bytes, content_type) = self.download_image_bytes(image_url).await?;
        let filename = image_url.split('/').next_back().unwrap_or("image.jpg").to_string();
        let user_owned = user.to_string();
        let platform_owned = platform.to_string();
        let content_owned = content.to_string();
        with_retry(
            || async {
                let file_part = reqwest::multipart::Part::bytes(bytes.clone())
                    .file_name(filename.clone())
                    .mime_str(&content_type)
                    .map_err(|e| ProviderError::Unknown(e.to_string()))?;
                let mut form = reqwest::multipart::Form::new()
                    .text("user", user_owned.clone())
                    .text("platform[]", platform_owned.clone())
                    .text("title", content_owned.clone())
                    .part("photos[]", file_part);
                if let Some(dt) = scheduled_for {
                    form = form.text("scheduled_date", dt.to_rfc3339());
                }
                let url = format!("{}/upload_photos", self.base_url());
                let resp = self
                    .client
                    .post(&url)
                    .header("Authorization", self.auth_header())
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|e| ProviderError::NetworkError(e.to_string()))?;
                Self::check_status(&resp)?;
                if !resp.status().is_success() {
                    let status = resp.status().as_u16();
                    let body = resp.text().await.unwrap_or_default();
                    return Err(ProviderError::HttpError { status, body });
                }
                let json: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| ProviderError::Unknown(format!("Failed to parse response: {}", e)))?;
                log::debug!("UploadPost schedule_with_image response: {}", json);
                let scheduler_id = Self::parse_scheduler_id(&json)?;
                Ok(PostScheduleResult { scheduler_id, platform_url: None })
            },
            3,
        )
        .await
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_schedule;
