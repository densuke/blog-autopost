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

    async fn upload_drive_file_data(
        &self,
        bytes: Vec<u8>,
        mime: &str,
        sensitive: bool,
    ) -> anyhow::Result<String> {
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

        let response = self.client.post(&url).multipart(form).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "Misskey drive upload failed: {}",
                error_text
            ));
        }

        let res_json: serde_json::Value = response.json().await?;
        let file_id = res_json["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No file id returned"))?;

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
                    let upload_mime = if mime == "image/png" || mime == "image/jpeg" {
                        mime
                    } else {
                        "image/jpeg".to_string()
                    };
                    match self
                        .upload_drive_file_data(bytes, &upload_mime, content.sensitive)
                        .await
                    {
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
                        let mime = if path.ends_with(".png") {
                            "image/png"
                        } else {
                            "image/jpeg"
                        };
                        match self
                            .upload_drive_file_data(bytes, mime, content.sensitive)
                            .await
                        {
                            Ok(id) => file_ids.push(id),
                            Err(e) => {
                                println!("Warning: Failed to upload local media to Misskey: {}", e)
                            }
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

        let response = self.client.post(&url).json(&payload).send().await?;

        if response.status().is_success() {
            let res_json: serde_json::Value = response.json().await?;
            let note_id = res_json["createdNote"]["id"]
                .as_str()
                .map(|s| s.to_string());

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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// モックサーバを向いた MisskeyClient を作る。
    fn client_for(server: &MockServer) -> MisskeyClient {
        MisskeyClient::new(
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
    fn test_client_metadata() {
        let client = MisskeyClient::new(
            "https://misskey.example.com".to_string(),
            "dummy_token".to_string(),
            "test_account".to_string(),
        )
        .unwrap();

        assert_eq!(client.name(), "misskey");
        assert_eq!(client.account_name(), "test_account");
        assert_eq!(client.max_characters(), 3000);
    }

    /// インスタンスURLの末尾スラッシュは取り除かれる。
    #[test]
    fn test_new_trims_trailing_slash() {
        let client = MisskeyClient::new(
            "https://misskey.example.com/".to_string(),
            "t".to_string(),
            "a".to_string(),
        )
        .unwrap();

        assert_eq!(client.base_url, "https://misskey.example.com");
    }

    /// Misskey は URL を実際の文字数で数える(既定の実装をそのまま使う)。
    #[test]
    fn test_url_char_weight_is_actual_length() {
        let client = MisskeyClient::new(
            "https://misskey.example.com".to_string(),
            "t".to_string(),
            "a".to_string(),
        )
        .unwrap();

        let url = "https://example.com/article/1";
        assert_eq!(client.url_char_weight(url), url.chars().count());
    }

    #[tokio::test]
    async fn test_post_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "createdNote": { "id": "note-123" }
            })))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト投稿"))
            .await
            .expect("投稿でエラーが発生した");

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("note-123"));
        assert!(result.error_message.is_none());
    }

    /// link_url が指定された場合、本文の末尾へ半角スペース区切りで連結される。
    #[tokio::test]
    async fn test_post_appends_link_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "i": "dummy_token",
                "text": "本文 https://example.com/a"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "createdNote": { "id": "note-456" }
            })))
            .mount(&server)
            .await;

        let content = PostContent {
            text: "本文".to_string(),
            link_url: Some("https://example.com/a".to_string()),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("note-456"));
    }

    /// 認証エラー。実装はエラー応答を Err ではなく success:false として返す。
    #[tokio::test]
    async fn test_post_returns_failure_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .respond_with(ResponseTemplate::new(401).set_body_string("authentication failed"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .expect("HTTPエラーは Err ではなく PostResult で表現される");

        assert!(!result.success);
        assert!(result.post_id.is_none());
        assert_eq!(
            result.error_message.as_deref(),
            Some("authentication failed")
        );
    }

    #[tokio::test]
    async fn test_post_returns_failure_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
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

    /// 応答に createdNote が含まれない場合でも success:true のまま post_id が None になる。
    #[tokio::test]
    async fn test_post_success_without_note_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
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
            .and(path("/api/notes/create"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "createdNote": { "id": "note-789" }
            })))
            .mount(&server)
            .await;

        let content = PostContent {
            text: "テスト".to_string(),
            media_paths: Some(vec!["/存在しないパス/image.png".to_string()]),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("note-789"));
    }

    /// 画像の添付に成功すると fileIds を含めて投稿される。
    #[tokio::test]
    async fn test_post_with_image_uploads_and_attaches_file_id() {
        let server = MockServer::start().await;

        // 1x1 の PNG を返す画像エンドポイント
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
            .and(path("/api/drive/files/create"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "id": "file-1" })),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "i": "dummy_token",
                "text": "画像付き",
                "fileIds": ["file-1"]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "createdNote": { "id": "note-img" }
            })))
            .mount(&server)
            .await;

        let content = PostContent {
            text: "画像付き".to_string(),
            image_url: Some(format!("{}/image.png", server.uri())),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("note-img"));
    }

    /// 画像URLが画像でない場合は添付せず、本文のみで投稿される。
    #[tokio::test]
    async fn test_post_skips_non_image_url() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/not-image"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("<html>これは画像ではない</html>")
                    .insert_header("content-type", "text/html"),
            )
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/notes/create"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "i": "dummy_token",
                "text": "本文のみ"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "createdNote": { "id": "note-noimg" }
            })))
            .mount(&server)
            .await;

        let content = PostContent {
            text: "本文のみ".to_string(),
            image_url: Some(format!("{}/not-image", server.uri())),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("note-noimg"));
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
