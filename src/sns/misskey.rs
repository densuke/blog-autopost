use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

pub struct MisskeyClient {
    client: Client,
    base_url: String,
    access_token: String,
    account_name: String,
}

impl MisskeyClient {
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

    async fn upload_drive_file(&self, image_url: &str) -> anyhow::Result<String> {
        let (bytes, mime) = super::download_image(&self.client, image_url).await?;
        
        let url = format!("{}/api/drive/files/create", self.base_url);
        
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name("image.jpg")
            .mime_str(&mime)?;
            
        let form = reqwest::multipart::Form::new()
            .text("i", self.access_token.clone())
            .part("file", part);
        
        let response = self.client.post(&url)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Misskey drive upload failed: {}", error_text));
        }

        let res_json: serde_json::Value = response.json().await?;
        let file_id = res_json["id"].as_str().ok_or_else(|| anyhow::anyhow!("No file id returned"))?;
        
        Ok(file_id.to_string())
    }
}

#[async_trait]
impl SnsClient for MisskeyClient {
    fn name(&self) -> &str {
        "misskey"
    }

    fn account_name(&self) -> &str {
        &self.account_name
    }

    async fn post(&self, content: &PostContent) -> anyhow::Result<PostResult> {
        let mut file_ids = Vec::new();
        
        if let Some(img_url) = &content.image_url {
            match self.upload_drive_file(img_url).await {
                Ok(id) => file_ids.push(id),
                Err(e) => println!("Warning: Failed to upload file to Misskey: {}", e),
            }
        }

        let url = format!("{}/api/notes/create", self.base_url);
        let mut payload = json!({
            "i": self.access_token,
            "text": content.text,
        });

        if !file_ids.is_empty() {
            payload["fileIds"] = json!(file_ids);
        }

        let response = self.client.post(&url)
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            let note_id = res_json["createdNote"]["id"].as_str().map(|s| s.to_string());
            
            Ok(PostResult {
                success: true,
                post_id: note_id,
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
        3000 // Misskeyは一般的に長文が可能 (インスタンスによるが3000文字程度が標準)
    }
}
