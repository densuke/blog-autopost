use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

pub struct MisskeyClient {
    client: Client,
    instance_url: String,
    access_token: String,
    account_name: String,
}

impl MisskeyClient {
    pub fn new(instance_url: String, access_token: String, account_name: String) -> anyhow::Result<Self> {
        let client = Client::new();
            
        Ok(Self {
            client,
            instance_url,
            access_token,
            account_name,
        })
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
        let url = format!("{}/api/notes/create", self.instance_url.trim_end_matches('/'));
        
        let payload = json!({
            "i": self.access_token,
            "text": content.text,
            "visibility": "public"
        });

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
