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

    async fn upload_drive_file_data(&self, bytes: Vec<u8>, mime: &str, sensitive: bool) -> anyhow::Result<String> {
        let resizer = crate::image_resizer::ImageResizer::new(false);
        let resized_bytes = resizer.resize_image_data(&bytes, "misskey")?;

        let url = format!("{}/api/drive/files/create", self.base_url);

        let part = reqwest::multipart::Part::bytes(resized_bytes)
            .file_name("image.jpg")
            .mime_str(mime)?;

        let form = reqwest::multipart::Form::new()
            .text("i", self.access_token.clone())
            .text("isSensitive", if sensitive { "true" } else { "false" })
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
        
        // 1. image_urlの処理
        if let Some(img_url) = &content.image_url {
            match super::download_image(&self.client, img_url).await {
                Ok(Some((bytes, mime))) => {
                    let upload_mime = if mime == "image/png" || mime == "image/jpeg" { mime } else { "image/jpeg".to_string() };
                    match self.upload_drive_file_data(bytes, &upload_mime, content.sensitive).await {
                        Ok(id) => file_ids.push(id),
                        Err(e) => println!("Warning: Failed to upload file to Misskey: {}", e),
                    }
                }
                Ok(None) => println!("[Misskey] 画像ではないためスキップしました: {}", img_url),
                Err(e) => println!("Warning: Failed to download image for Misskey: {}", e),
            }
        }

        // 2. media_pathsの処理
        if let Some(paths) = &content.media_paths {
            for path in paths {
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
                        match self.upload_drive_file_data(bytes, mime, content.sensitive).await {
                            Ok(id) => file_ids.push(id),
                            Err(e) => println!("Warning: Failed to upload local media to Misskey: {}", e),
                        }
                    }
                    Err(e) => println!("Warning: Failed to read local media file {}: {}", path, e),
                }
            }
        }

        let url = format!("{}/api/notes/create", self.base_url);
        let mut post_text = content.text.clone();
        if let Some(link_url) = &content.link_url {
            post_text = format!("{} {}", post_text, link_url);
        }
        let mut payload = json!({
            "i": self.access_token,
            "text": post_text,
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
