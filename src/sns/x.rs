use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use reqwest::header::AUTHORIZATION;
use anyhow::{Result, Context, anyhow};

#[derive(oauth1_request::Request)]
struct EmptyRequest {}

use super::traits::SnsClient;
use super::models::{PostContent, PostResult};

pub struct XClient {
    client: Client,
    consumer_key: String,
    consumer_secret: String,
    access_token: String,
    access_token_secret: String,
    name: String,
}

#[derive(Deserialize)]
struct MediaUploadResponse {
    media_id_string: String,
}

#[derive(Deserialize)]
struct TweetResponseData {
    id: String,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct TweetResponse {
    data: Option<TweetResponseData>,
    detail: Option<String>,
}

impl XClient {
    pub fn new(
        consumer_key: String,
        consumer_secret: String,
        access_token: String,
        access_token_secret: String,
        name: String,
    ) -> Result<Self> {
        let client = Client::builder()
            .build()
            .context("Failed to build HTTP client for X")?;

        Ok(Self {
            client,
            consumer_key,
            consumer_secret,
            access_token,
            access_token_secret,
            name,
        })
    }

    /// OAuth 1.0a Authorizationヘッダを生成する (POST用)
    fn generate_post_auth_header(&self, url: &str) -> String {
        let token = oauth1_request::Token::from_parts(
            &self.consumer_key,
            &self.consumer_secret,
            &self.access_token,
            &self.access_token_secret,
        );
        oauth1_request::post(url, &EmptyRequest {}, &token, oauth1_request::HMAC_SHA1)
    }

    /// 画像をアップロードして media_id_string を取得する
    async fn upload_media(&self, image_data: Vec<u8>) -> Result<String> {
        let resizer = crate::image_resizer::ImageResizer::new(false);
        let resized_bytes = resizer.resize_image_data(&image_data, "x")?;

        let upload_url = "https://upload.twitter.com/1.1/media/upload.json";
        
        let auth_header = self.generate_post_auth_header(upload_url);
        
        let part = reqwest::multipart::Part::bytes(resized_bytes)
            .file_name("image.jpg")
            .mime_str("image/jpeg")?; 

        let form = reqwest::multipart::Form::new().part("media", part);

        let res = self.client.post(upload_url)
            .header(AUTHORIZATION, auth_header)
            .multipart(form)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Failed to upload media to X: HTTP {}, body: {}", status, body));
        }

        let upload_res: MediaUploadResponse = serde_json::from_str(&body)
            .context("Failed to parse media upload response from X")?;

        Ok(upload_res.media_id_string)
    }
}

#[async_trait]
impl SnsClient for XClient {
    fn name(&self) -> &str {
        "x"
    }

    fn account_name(&self) -> &str {
        &self.name
    }

    fn max_characters(&self) -> usize {
        280
    }

    /// Xはt.coによりURLを実際の長さに関わらず一律23文字としてカウントする
    fn url_char_weight(&self, _url: &str) -> usize {
        23
    }

    async fn post(&self, content: &PostContent) -> Result<PostResult, anyhow::Error> {
        let mut media_ids = Vec::new();

        // 1. image_urlの処理
        if let Some(url) = &content.image_url {
            match super::download_image(&self.client, url).await {
                Ok(Some((bytes, _mime))) => {
                    match self.upload_media(bytes).await {
                        Ok(media_id) => media_ids.push(media_id),
                        Err(e) => {
                            println!("[X] Warning: Failed to upload image: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    println!("[X] 画像ではないためスキップしました: {}", url);
                }
                Err(e) => {
                    println!("[X] Warning: Failed to download image: {}", e);
                }
            }
        }

        // 2. media_pathsの処理
        if let Some(paths) = &content.media_paths {
            for path in paths {
                match std::fs::read(path) {
                    Ok(bytes) => {
                        match self.upload_media(bytes).await {
                            Ok(media_id) => media_ids.push(media_id),
                            Err(e) => {
                                println!("[X] Warning: Failed to upload local media: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("[X] Warning: Failed to read local media file {}: {}", path, e);
                    }
                }
            }
        }

        // 2. ツイート投稿
        let tweet_url = "https://api.twitter.com/2/tweets";
        let auth_header = self.generate_post_auth_header(tweet_url);

        let mut post_text = content.text.clone();
        if let Some(link_url) = &content.link_url {
            post_text = format!("{} {}", post_text, link_url);
        }
        let mut payload = json!({
            "text": post_text,
        });

        if !media_ids.is_empty() {
            payload["media"] = json!({
                "media_ids": media_ids
            });
        }

        let res = self.client.post(tweet_url)
            .header(AUTHORIZATION, auth_header)
            .json(&payload)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if !status.is_success() {
            return Ok(PostResult {
                success: false,
                post_id: None,
                error_message: Some(format!("HTTP {}: {}", status, body)),
            });
        }

        let tweet_res: TweetResponse = serde_json::from_str(&body).unwrap_or(TweetResponse { data: None, detail: None });

        let post_id = tweet_res.data.map(|d| d.id);

        Ok(PostResult {
            success: true,
            post_id,
            error_message: None,
        })
    }
}
