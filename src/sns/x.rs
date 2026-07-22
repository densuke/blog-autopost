use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use serde_json::json;

#[derive(oauth1_request::Request)]
struct EmptyRequest {}

use super::models::{PostContent, PostResult};
use super::traits::SnsClient;

/// X のメディアアップロード先(本番)
const DEFAULT_UPLOAD_URL: &str = "https://upload.twitter.com/1.1/media/upload.json";
/// X のツイート投稿先(本番)
const DEFAULT_TWEET_URL: &str = "https://api.twitter.com/2/tweets";

pub struct XClient {
    client: Client,
    consumer_key: String,
    consumer_secret: String,
    access_token: String,
    access_token_secret: String,
    name: String,
    /// メディアアップロードのエンドポイント。テストでモックサーバへ差し替える。
    upload_url: String,
    /// ツイート投稿のエンドポイント。テストでモックサーバへ差し替える。
    tweet_url: String,
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
            upload_url: DEFAULT_UPLOAD_URL.to_string(),
            tweet_url: DEFAULT_TWEET_URL.to_string(),
        })
    }

    /// エンドポイントを差し替えた XClient を返す。
    ///
    /// 本番のURLはX固有のホスト名で固定されているため、テストから
    /// モックサーバを向けるための入口として用意している。
    #[cfg(test)]
    fn with_endpoints(mut self, upload_url: String, tweet_url: String) -> Self {
        self.upload_url = upload_url;
        self.tweet_url = tweet_url;
        self
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

        let upload_url = self.upload_url.as_str();

        let auth_header = self.generate_post_auth_header(upload_url);

        let part = reqwest::multipart::Part::bytes(resized_bytes)
            .file_name("image.jpg")
            .mime_str("image/jpeg")?;

        let form = reqwest::multipart::Form::new().part("media", part);

        let res = self
            .client
            .post(upload_url)
            .header(AUTHORIZATION, auth_header)
            .multipart(form)
            .send()
            .await?;

        let status = res.status();
        let body = res.text().await?;

        if !status.is_success() {
            return Err(anyhow!(
                "Failed to upload media to X: HTTP {}, body: {}",
                status,
                body
            ));
        }

        let upload_res: MediaUploadResponse =
            serde_json::from_str(&body).context("Failed to parse media upload response from X")?;

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
                Ok(Some((bytes, _mime))) => match self.upload_media(bytes).await {
                    Ok(media_id) => media_ids.push(media_id),
                    Err(e) => {
                        println!("[X] Warning: Failed to upload image: {}", e);
                    }
                },
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
                    Ok(bytes) => match self.upload_media(bytes).await {
                        Ok(media_id) => media_ids.push(media_id),
                        Err(e) => {
                            println!("[X] Warning: Failed to upload local media: {}", e);
                        }
                    },
                    Err(e) => {
                        println!(
                            "[X] Warning: Failed to read local media file {}: {}",
                            path, e
                        );
                    }
                }
            }
        }

        // 2. ツイート投稿
        let tweet_url = self.tweet_url.as_str();
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

        let res = self
            .client
            .post(tweet_url)
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

        let tweet_res: TweetResponse = serde_json::from_str(&body).unwrap_or(TweetResponse {
            data: None,
            detail: None,
        });

        let post_id = tweet_res.data.map(|d| d.id);

        Ok(PostResult {
            success: true,
            post_id,
            error_message: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn base_client() -> XClient {
        XClient::new(
            "ck".to_string(),
            "cs".to_string(),
            "at".to_string(),
            "ats".to_string(),
            "test_account".to_string(),
        )
        .expect("クライアントの生成に失敗")
    }

    /// モックサーバを向いた XClient を作る。
    fn client_for(server: &MockServer) -> XClient {
        base_client().with_endpoints(
            format!("{}/1.1/media/upload.json", server.uri()),
            format!("{}/2/tweets", server.uri()),
        )
    }

    fn text_content(text: &str) -> PostContent {
        PostContent {
            text: text.to_string(),
            ..Default::default()
        }
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

    #[test]
    fn test_client_metadata() {
        let client = base_client();

        assert_eq!(client.name(), "x");
        assert_eq!(client.account_name(), "test_account");
        assert_eq!(client.max_characters(), 280);
    }

    /// 既定のエンドポイントは本番のURLを指す。
    #[test]
    fn test_new_uses_production_endpoints() {
        let client = base_client();

        assert_eq!(client.upload_url, DEFAULT_UPLOAD_URL);
        assert_eq!(client.tweet_url, DEFAULT_TWEET_URL);
    }

    /// X は t.co によりURLを一律23文字として数える。
    #[test]
    fn test_url_char_weight_is_always_23() {
        let client = base_client();

        assert_eq!(
            client.url_char_weight("https://a.example.com/b/c/d/e/f"),
            23
        );
        assert_eq!(client.url_char_weight("http://x.jp"), 23);
    }

    /// OAuth 1.0a の Authorization ヘッダが生成される。
    #[test]
    fn test_generate_post_auth_header() {
        let header = base_client().generate_post_auth_header("https://api.example.com/x");

        assert!(header.starts_with("OAuth "), "実際の値: {}", header);
        assert!(header.contains("oauth_consumer_key"));
        assert!(header.contains("oauth_signature"));
    }

    #[tokio::test]
    async fn test_post_success() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-123", "text": "テスト投稿" }
            })))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト投稿"))
            .await
            .expect("投稿でエラーが発生した");

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("tweet-123"));
        assert!(result.error_message.is_none());
    }

    /// link_url が指定された場合、本文の末尾へ連結される。
    #[tokio::test]
    async fn test_post_appends_link_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "text": "本文 https://example.com/a"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-456" }
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
        assert_eq!(result.post_id.as_deref(), Some("tweet-456"));
    }

    /// 認証エラー。実装はエラー応答を Err ではなく success:false として返す。
    #[tokio::test]
    async fn test_post_returns_failure_on_401() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.post_id.is_none());
        let msg = result.error_message.expect("エラーメッセージが必要");
        assert!(msg.contains("401"), "実際の値: {}", msg);
        assert!(msg.contains("Unauthorized"), "実際の値: {}", msg);
    }

    #[tokio::test]
    async fn test_post_returns_failure_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.error_message.unwrap().contains("500"));
    }

    /// 応答のJSONが壊れていても success:true のまま post_id が None になる。
    #[tokio::test]
    async fn test_post_success_with_unparsable_body() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .respond_with(ResponseTemplate::new(201).set_body_string("not a json"))
            .mount(&server)
            .await;

        let result = client_for(&server)
            .post(&text_content("テスト"))
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.post_id.is_none());
    }

    /// 画像の添付に成功すると media.media_ids を含めて投稿される。
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
            .and(path("/1.1/media/upload.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "media_id_string": "media-1"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "text": "画像付き",
                "media": { "media_ids": ["media-1"] }
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-img" }
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
        assert_eq!(result.post_id.as_deref(), Some("tweet-img"));
    }

    /// メディアのアップロードに失敗しても、本文のみの投稿は継続される。
    #[tokio::test]
    async fn test_post_continues_when_media_upload_fails() {
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
            .and(path("/1.1/media/upload.json"))
            .respond_with(ResponseTemplate::new(400).set_body_string("upload rejected"))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "text": "画像は落ちた"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-nomedia" }
            })))
            .mount(&server)
            .await;

        let content = PostContent {
            text: "画像は落ちた".to_string(),
            image_url: Some(format!("{}/image.png", server.uri())),
            ..Default::default()
        };

        let result = client_for(&server).post(&content).await.unwrap();

        assert!(result.success);
        assert_eq!(result.post_id.as_deref(), Some("tweet-nomedia"));
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
            .and(path("/2/tweets"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "text": "本文のみ"
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-noimg" }
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
    }

    /// 読み込めないローカルメディアを指定しても投稿自体は継続する。
    #[tokio::test]
    async fn test_post_continues_when_local_media_missing() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/2/tweets"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "data": { "id": "tweet-nolocal" }
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
    }
}
