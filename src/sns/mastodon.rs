use async_trait::async_trait;
use reqwest::{Client, header};
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
    pub fn new(
        instance_url: String,
        access_token: String,
        account_name: String,
    ) -> anyhow::Result<Self> {
        let client = Client::new();
        let base_url = instance_url.trim_end_matches('/').to_string();
        Ok(Self {
            client,
            base_url,
            access_token,
            account_name,
        })
    }

    async fn upload_media_data(&self, bytes: Vec<u8>, mime: &str) -> anyhow::Result<String> {
        let resizer = crate::image_resizer::ImageResizer::new(false);
        let resized_bytes = resizer.resize_image_data(&bytes, "mastodon")?;

        let url = format!("{}/api/v2/media", self.base_url);

        let part = reqwest::multipart::Part::bytes(resized_bytes)
            .file_name("image.jpg")
            .mime_str(mime)?;

        let form = reqwest::multipart::Form::new().part("file", part);

        let response = self
            .client
            .post(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "Mastodon media upload failed: {}",
                error_text
            ));
        }

        let res_json: serde_json::Value = response.json().await?;
        let media_id = res_json["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No media id returned"))?;

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

        // 1. image_urlの処理
        if let Some(img_url) = &content.image_url {
            match super::download_image(&self.client, img_url).await {
                Ok(Some((bytes, mime))) => {
                    let upload_mime = if mime == "image/png" || mime == "image/jpeg" {
                        mime
                    } else {
                        "image/jpeg".to_string()
                    };
                    match self.upload_media_data(bytes, &upload_mime).await {
                        Ok(id) => media_ids.push(id),
                        Err(e) => println!("Warning: Failed to upload media to Mastodon: {}", e),
                    }
                }
                Ok(None) => println!("[Mastodon] 画像ではないためスキップしました: {}", img_url),
                Err(e) => println!("Warning: Failed to download image for Mastodon: {}", e),
            }
        }

        // 2. media_pathsの処理
        if let Some(paths) = &content.media_paths {
            for path in paths {
                match std::fs::read(path) {
                    Ok(bytes) => {
                        let mime = if path.ends_with(".png") {
                            "image/png"
                        } else {
                            "image/jpeg"
                        };
                        match self.upload_media_data(bytes, mime).await {
                            Ok(id) => media_ids.push(id),
                            Err(e) => {
                                println!("Warning: Failed to upload local media to Mastodon: {}", e)
                            }
                        }
                    }
                    Err(e) => println!("Warning: Failed to read local media file {}: {}", path, e),
                }
            }
        }

        let url = format!("{}/api/v1/statuses", self.base_url);
        let mut post_text = content.text.clone();
        if let Some(link_url) = &content.link_url {
            post_text = format!("{} {}", post_text, link_url);
        }
        let mut payload = json!({
            "status": post_text,
        });

        if !media_ids.is_empty() {
            payload["media_ids"] = json!(media_ids);
        }

        let response = self
            .client
            .post(&url)
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.access_token),
            )
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

    /// MastodonはリンクをURLの実際の長さに関わらず一律23文字としてカウントする
    fn url_char_weight(&self, _url: &str) -> usize {
        23
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// モックサーバを向いた MastodonClient を作る。
    fn client_for(server: &MockServer) -> MastodonClient {
        MastodonClient::new(
            server.uri(),
            "dummy_token".to_string(),
            "test_account".to_string(),
        )
        .expect("クライアントの生成に失敗")
    }

    fn text_content(text: &str) -> PostContent {
        PostContent {
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_mastodon_client_creation() {
        let client = MastodonClient::new(
            "https://mstdn.example.com".to_string(),
            "dummy_token".to_string(),
            "dummy_account".to_string(),
        )
        .unwrap();

        assert_eq!(client.name(), "mastodon");
        assert_eq!(client.account_name(), "dummy_account");
        assert_eq!(client.max_characters(), 500);
    }

    /// インスタンスURLの末尾スラッシュは取り除かれる。
    #[test]
    fn test_new_trims_trailing_slash() {
        let client = MastodonClient::new(
            "https://mstdn.example.com/".to_string(),
            "t".to_string(),
            "a".to_string(),
        )
        .unwrap();

        assert_eq!(client.base_url, "https://mstdn.example.com");
    }

    /// Mastodon はURLの実長に関わらず一律23文字として数える。
    #[test]
    fn test_url_char_weight_is_always_23() {
        let client = MastodonClient::new(
            "https://mstdn.example.com".to_string(),
            "t".to_string(),
            "a".to_string(),
        )
        .unwrap();

        assert_eq!(
            client.url_char_weight("https://a.example.com/b/c/d/e/f/g"),
            23
        );
        assert_eq!(client.url_char_weight("http://x.jp"), 23);
    }

    #[tokio::test]
    async fn test_post_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(header("authorization", "Bearer dummy_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "url": "https://mstdn.example.com/@user/1"
            })))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト投稿"))
            .await
            .expect("投稿でエラーが発生した");

        assert!(result.success);
        assert_eq!(
            result.post_id.as_deref(),
            Some("https://mstdn.example.com/@user/1")
        );
        assert!(result.error_message.is_none());
    }

    /// link_url が指定された場合、status の末尾へ連結される。
    #[tokio::test]
    async fn test_post_appends_link_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "status": "本文 https://example.com/a"
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/2" })),
            )
            .mount(&server)
            .await;

        let content = PostContent {
            text: "本文".to_string(),
            link_url: Some("https://example.com/a".to_string()),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
    }

    /// 認証エラー。実装はエラー応答を Err ではなく success:false として返す。
    #[tokio::test]
    async fn test_post_returns_failure_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(401).set_body_string("The access token is invalid"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.post_id.is_none());
        assert_eq!(
            result.error_message.as_deref(),
            Some("The access token is invalid")
        );
    }

    #[tokio::test]
    async fn test_post_returns_failure_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.error_message.as_deref(), Some("internal error"));
    }

    /// 応答に url が含まれない場合でも success:true のまま post_id が None になる。
    #[tokio::test]
    async fn test_post_success_without_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.post_id.is_none());
    }

    /// 読み込めないローカルメディアを指定しても投稿自体は継続する。
    #[tokio::test]
    async fn test_post_continues_when_local_media_missing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/3" })),
            )
            .mount(&server)
            .await;

        let content = PostContent {
            text: "テスト".to_string(),
            media_paths: Some(vec!["/存在しないパス/image.png".to_string()]),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
    }

    /// 画像の添付に成功すると media_ids を含めて投稿される。
    #[tokio::test]
    async fn test_post_with_image_attaches_media_id() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/image.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(tiny_png())
                    .insert_header("content-type", "image/png"),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v2/media"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "media-1" })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "status": "画像付き",
                "media_ids": ["media-1"]
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/4" })),
            )
            .mount(&server)
            .await;

        let content = PostContent {
            text: "画像付き".to_string(),
            image_url: Some(format!("{}/image.png", server.uri())),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
    }

    /// 画像URLが画像でない場合は添付せず、本文のみで投稿される。
    #[tokio::test]
    async fn test_post_skips_non_image_url() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/not-image"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html>画像ではない</html>")
                    .insert_header("content-type", "text/html"),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v1/statuses"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "status": "本文のみ"
            })))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "url": "https://mstdn.example.com/@u/5" })),
            )
            .mount(&server)
            .await;

        let content = PostContent {
            text: "本文のみ".to_string(),
            image_url: Some(format!("{}/not-image", server.uri())),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
    }

    /// テスト用の 1x1 PNG バイト列を作る。
    fn tiny_png() -> Vec<u8> {
        use image::{ImageFormat, RgbaImage};
        let img = RgbaImage::new(1, 1);
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .expect("PNGの生成に失敗");
        buf.into_inner()
    }
}
