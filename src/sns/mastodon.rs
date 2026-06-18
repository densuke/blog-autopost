use async_trait::async_trait;
use reqwest::{header, Client};
use serde_json::json;

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

pub struct MastodonClient {
    client: Client,
    base_url: String,
    access_token: String,
    account_name: String,
}

impl MastodonClient {
    pub fn new(instance_url: String, access_token: String, account_name: String) -> anyhow::Result<Self> {
        let client = Client::new();
        let base_url = instance_url.trim_end_matches('/').to_string();
        Ok(Self {
            client,
            base_url,
            access_token,
            account_name,
        })
    }

    async fn upload_media(&self, image_url: &str) -> anyhow::Result<String> {
        let (bytes, mime) = super::download_image(&self.client, image_url).await?;
        
        let url = format!("{}/api/v2/media", self.base_url);
        
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name("image.jpg") // ダミー名でOK
            .mime_str(&mime)?;
            
        let form = reqwest::multipart::Form::new().part("file", part);
        
        let response = self.client.post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.access_token))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Mastodon media upload failed: {}", error_text));
        }

        let res_json: serde_json::Value = response.json().await?;
        let media_id = res_json["id"].as_str().ok_or_else(|| anyhow::anyhow!("No media id returned"))?;
        
        Ok(media_id.to_string())
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
        let mut media_ids = Vec::new();
        
        if let Some(img_url) = &content.image_url {
            match self.upload_media(img_url).await {
                Ok(id) => media_ids.push(id),
                Err(e) => println!("Warning: Failed to upload media to Mastodon: {}", e),
            }
        }

        let url = format!("{}/api/v1/statuses", self.base_url);
        let mut payload = json!({
            "status": content.text,
        });

        if !media_ids.is_empty() {
            payload["media_ids"] = json!(media_ids);
        }

        let response = self.client.post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.access_token))
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
