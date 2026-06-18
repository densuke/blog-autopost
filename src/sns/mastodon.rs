use async_trait::async_trait;
use reqwest::{header, Client};
use serde_json::json;

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

pub struct MastodonClient {
    client: Client,
    instance_url: String,
    account_name: String,
}

impl MastodonClient {
    pub fn new(instance_url: String, access_token: String, account_name: String) -> anyhow::Result<Self> {
        let mut headers = header::HeaderMap::new();
        let auth_value = header::HeaderValue::from_str(&format!("Bearer {}", access_token))?;
        headers.insert(header::AUTHORIZATION, auth_value);
        
        let client = Client::builder()
            .default_headers(headers)
            .build()?;
            
        Ok(Self {
            client,
            instance_url,
            account_name,
        })
    }
}

#[async_trait]
impl SnsClient for MastodonClient {
    fn name(&self) -> &str {
        "mastodon"
    }

    fn account_name(&self) -> &str {
        &self.account_name
    }

    async fn post(&self, content: &PostContent) -> anyhow::Result<PostResult> {
        let url = format!("{}/api/v1/statuses", self.instance_url.trim_end_matches('/'));
        
        // TODO: 画像(image_url)がある場合は /api/v1/media に投げてメディアIDを取得する処理を将来追加する
        let payload = json!({
            "status": content.text
        });

        let response = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            let post_id = res_json["url"].as_str().map(|s| s.to_string());
            
            Ok(PostResult {
                success: true,
                post_id,
                error_message: None,
            })
        } else {
            let error_text = response.text().await?;
            Ok(PostResult {
                success: false,
                post_id: None,
                error_message: Some(error_text),
            })
        }
    }

    fn max_characters(&self) -> usize {
        500
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mastodon_client_creation() {
        let client = MastodonClient::new(
            "https://mstdn.example.com".to_string(),
            "dummy_token".to_string(),
            "dummy_account".to_string()
        ).unwrap();

        assert_eq!(client.name(), "mastodon");
        assert_eq!(client.account_name(), "dummy_account");
        assert_eq!(client.max_characters(), 500);
    }
}
