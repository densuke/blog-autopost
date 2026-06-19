use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

pub struct BlueskyClient {
    client: Client,
    identifier: String,
    password: String,
    account_name: String,
}

impl BlueskyClient {
    pub fn new(identifier: String, password: String, account_name: String) -> anyhow::Result<Self> {
        let client = Client::new();
        Ok(Self {
            client,
            identifier,
            password,
            account_name,
        })
    }

    /// セッションを作成し、DIDとAccess Tokenを取得する
    async fn create_session(&self) -> anyhow::Result<(String, String)> {
        let url = "https://bsky.social/xrpc/com.atproto.server.createSession";
        let payload = json!({
            "identifier": self.identifier,
            "password": self.password,
        });

        let response = self.client.post(url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Bluesky login failed: {}", error_text));
        }

        let res_json: serde_json::Value = response.json().await?;
        let did = res_json["did"].as_str().ok_or_else(|| anyhow::anyhow!("No did in response"))?;
        let access_jwt = res_json["accessJwt"].as_str().ok_or_else(|| anyhow::anyhow!("No accessJwt in response"))?;

        Ok((did.to_string(), access_jwt.to_string()))
    }
    async fn upload_blob_data(&self, bytes: Vec<u8>, mime: &str, access_jwt: &str) -> anyhow::Result<serde_json::Value> {
        let resizer = crate::image_resizer::ImageResizer::new(false);
        let resized_bytes = resizer.resize_image_data(&bytes, "bluesky")?;

        let url = "https://bsky.social/xrpc/com.atproto.repo.uploadBlob";
        
        let response = self.client.post(url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", access_jwt))
            .header(reqwest::header::CONTENT_TYPE, mime.to_string())
            .body(resized_bytes)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Bluesky blob upload failed: {}", error_text));
        }

        let res_json: serde_json::Value = response.json().await?;
        let blob = res_json.get("blob").cloned().ok_or_else(|| anyhow::anyhow!("No blob object in response"))?;
        
        Ok(blob)
    }
}

#[async_trait]
impl SnsClient for BlueskyClient {
    fn name(&self) -> &str {
        "bluesky"
    }

    fn account_name(&self) -> &str {
        &self.account_name
    }

    async fn post(&self, content: &PostContent) -> anyhow::Result<PostResult> {
        // 1. セッションを作成してトークンを取得
        let (did, access_jwt) = match self.create_session().await {
            Ok(creds) => creds,
            Err(e) => return Ok(PostResult {
                success: false,
                post_id: None,
                error_message: Some(format!("Login error: {}", e)),
            })
        };

        let mut embed_blobs = Vec::new();
        
        // 1. image_urlの処理
        if let Some(img_url) = &content.image_url {
            match super::download_image(&self.client, img_url).await {
                Ok((bytes, mime)) => {
                    let upload_mime = if mime == "image/png" || mime == "image/jpeg" { mime } else { "image/jpeg".to_string() };
                    match self.upload_blob_data(bytes, &upload_mime, &access_jwt).await {
                        Ok(blob) => embed_blobs.push(blob),
                        Err(e) => println!("Warning: Failed to upload blob to Bluesky: {}", e),
                    }
                }
                Err(e) => println!("Warning: Failed to download image for Bluesky: {}", e),
            }
        }

        // 2. media_pathsの処理
        if let Some(paths) = &content.media_paths {
            for path in paths {
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
                        match self.upload_blob_data(bytes, mime, &access_jwt).await {
                            Ok(blob) => embed_blobs.push(blob),
                            Err(e) => println!("Warning: Failed to upload local media to Bluesky: {}", e),
                        }
                    }
                    Err(e) => println!("Warning: Failed to read local media file {}: {}", path, e),
                }
            }
        }

        let mut embed_external = None;
        if embed_blobs.is_empty() {
            if let Some(link_url) = &content.link_url {
                let ogp = fetch_ogp(&self.client, link_url).await;
                let thumb_blob = if let Some(thumb_url) = ogp.image_url {
                    match super::download_image(&self.client, &thumb_url).await {
                        Ok((bytes, mime)) => {
                            self.upload_blob_data(bytes, &mime, &access_jwt).await.ok()
                        }
                        Err(_) => None,
                    }
                } else {
                    None
                };

                embed_external = Some(json!({
                    "$type": "app.bsky.embed.external",
                    "external": {
                        "uri": link_url,
                        "title": ogp.title.unwrap_or_else(|| "ブログ記事".to_string()),
                        "description": ogp.description.unwrap_or_default(),
                        "thumb": thumb_blob
                    }
                }));
            }
        }

        // 3. レコードを作成して投稿
        let url = "https://bsky.social/xrpc/com.atproto.repo.createRecord";
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        
        let mut record = json!({
            "$type": "app.bsky.feed.post",
            "text": content.text,
            "createdAt": now
        });

        if !embed_blobs.is_empty() {
            let images_json: Vec<serde_json::Value> = embed_blobs.into_iter().map(|blob| {
                json!({
                    "alt": "",
                    "image": blob
                })
            }).collect();

            record["embed"] = json!({
                "$type": "app.bsky.embed.images",
                "images": images_json
            });
        } else if let Some(ext) = embed_external {
            record["embed"] = ext;
        }

        let payload = json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": record
        });

        let response = self.client.post(url)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", access_jwt))
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            let uri = res_json["uri"].as_str().map(|s| s.to_string());
            
            Ok(PostResult {
                success: true,
                post_id: uri,
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
        300
    }
}

struct OgpMetadata {
    title: Option<String>,
    description: Option<String>,
    image_url: Option<String>,
}

async fn fetch_ogp(client: &reqwest::Client, url: &str) -> OgpMetadata {
    let mut meta = OgpMetadata { title: None, description: None, image_url: None };
    let response = match client.get(url)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (compatible; Blog-AutoPost/1.0)")
        .send().await {
            Ok(resp) => resp,
            Err(_) => return meta,
        };
    
    if !response.status().is_success() {
        return meta;
    }
    
    let html = response.text().await.unwrap_or_default();
    
    let re_title = regex::Regex::new(r#"<meta\s+[^>]*property=["']og:title["']\s+[^>]*content=["']([^"']*)["']"#).ok();
    let re_desc = regex::Regex::new(r#"<meta\s+[^>]*property=["']og:description["']\s+[^>]*content=["']([^"']*)["']"#).ok();
    let re_image = regex::Regex::new(r#"<meta\s+[^>]*property=["']og:image["']\s+[^>]*content=["']([^"']*)["']"#).ok();
    let re_meta_desc = regex::Regex::new(r#"<meta\s+[^>]*name=["']description["']\s+[^>]*content=["']([^"']*)["']"#).ok();
    let re_html_title = regex::Regex::new(r#"<title[^>]*>([^<]*)</title>"#).ok();

    meta.title = re_title.and_then(|r| r.captures(&html)).map(|c| c[1].to_string())
        .or_else(|| re_html_title.and_then(|r| r.captures(&html)).map(|c| c[1].trim().to_string()));
    
    meta.description = re_desc.and_then(|r| r.captures(&html)).map(|c| c[1].to_string())
        .or_else(|| re_meta_desc.and_then(|r| r.captures(&html)).map(|c| c[1].to_string()));
    
    meta.image_url = re_image.and_then(|r| r.captures(&html)).map(|c| c[1].to_string());

    meta
}
