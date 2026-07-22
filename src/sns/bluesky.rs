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

        let response = self.client.post(url).json(&payload).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Bluesky login failed: {}", error_text));
        }

        let res_json: serde_json::Value = response.json().await?;
        let did = res_json["did"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No did in response"))?;
        let access_jwt = res_json["accessJwt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No accessJwt in response"))?;

        Ok((did.to_string(), access_jwt.to_string()))
    }
    async fn upload_blob_data(
        &self,
        bytes: Vec<u8>,
        mime: &str,
        access_jwt: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let resizer = crate::image_resizer::ImageResizer::new(false);
        let resized_bytes = resizer.resize_image_data(&bytes, "bluesky")?;

        let url = "https://bsky.social/xrpc/com.atproto.repo.uploadBlob";

        let response = self
            .client
            .post(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", access_jwt),
            )
            .header(reqwest::header::CONTENT_TYPE, mime.to_string())
            .body(resized_bytes)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!(
                "Bluesky blob upload failed: {}",
                error_text
            ));
        }

        let res_json: serde_json::Value = response.json().await?;
        let blob = res_json
            .get("blob")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No blob object in response"))?;

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
            Err(e) => {
                return Ok(PostResult {
                    success: false,
                    post_id: None,
                    error_message: Some(format!("Login error: {}", e)),
                });
            }
        };

        let mut embed_blobs = Vec::new();

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
                        .upload_blob_data(bytes, &upload_mime, &access_jwt)
                        .await
                    {
                        Ok(blob) => embed_blobs.push(blob),
                        Err(e) => println!("Warning: Failed to upload blob to Bluesky: {}", e),
                    }
                }
                Ok(None) => println!("[Bluesky] 画像ではないためスキップしました: {}", img_url),
                Err(e) => println!("Warning: Failed to download image for Bluesky: {}", e),
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
                        match self.upload_blob_data(bytes, mime, &access_jwt).await {
                            Ok(blob) => embed_blobs.push(blob),
                            Err(e) => {
                                println!("Warning: Failed to upload local media to Bluesky: {}", e)
                            }
                        }
                    }
                    Err(e) => println!("Warning: Failed to read local media file {}: {}", path, e),
                }
            }
        }

        // リンクカードのURLは、明示指定(link_url)が無ければ本文中の最初のURLを使う。
        // 自動投稿ではURLが本文インラインに入るため、ここで拾ってカード化する。
        let card_url = content
            .link_url
            .clone()
            .or_else(|| first_url_in_text(&content.text));

        let mut embed_external = None;
        if embed_blobs.is_empty()
            && let Some(link_url) = &card_url
        {
            let ogp = fetch_ogp(&self.client, link_url).await;
            let thumb_blob = if let Some(thumb_url) = ogp.image_url {
                match super::download_image(&self.client, &thumb_url).await {
                    Ok(Some((bytes, mime))) => {
                        self.upload_blob_data(bytes, &mime, &access_jwt).await.ok()
                    }
                    Ok(None) | Err(_) => None,
                }
            } else {
                None
            };

            let mut external = json!({
                "uri": link_url,
                "title": ogp.title.unwrap_or_else(|| "ブログ記事".to_string()),
                "description": ogp.description.unwrap_or_default(),
            });

            if let Some(blob) = thumb_blob {
                external["thumb"] = blob;
            }

            embed_external = Some(json!({
                "$type": "app.bsky.embed.external",
                "external": external
            }));
        }

        // 3. レコードを作成して投稿
        let url = "https://bsky.social/xrpc/com.atproto.repo.createRecord";
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let mut record = json!({
            "$type": "app.bsky.feed.post",
            "text": content.text,
            "createdAt": now
        });

        // 本文中のURLとハッシュタグを facets として付与する。
        // Blueskyは本文のURL/タグを自動でリンク化しないため、これが無いと
        // ただの文字列になる。
        let mut facets = build_link_facets(&content.text);
        facets.extend(build_tag_facets(&content.text));
        if !facets.is_empty() {
            record["facets"] = json!(facets);
        }

        if !embed_blobs.is_empty() {
            let images_json: Vec<serde_json::Value> = embed_blobs
                .into_iter()
                .map(|blob| {
                    json!({
                        "alt": "",
                        "image": blob
                    })
                })
                .collect();

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

        let response = self
            .client
            .post(url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", access_jwt),
            )
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
    let mut meta = OgpMetadata {
        title: None,
        description: None,
        image_url: None,
    };
    let response = match client
        .get(url)
        .header(
            reqwest::header::USER_AGENT,
            "Mozilla/5.0 (compatible; Blog-AutoPost/1.0)",
        )
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => return meta,
    };

    if !response.status().is_success() {
        return meta;
    }

    let html = response.text().await.unwrap_or_default();

    let re_title = regex::Regex::new(
        r#"<meta\s+[^>]*property=["']og:title["']\s+[^>]*content=["']([^"']*)["']"#,
    )
    .ok();
    let re_desc = regex::Regex::new(
        r#"<meta\s+[^>]*property=["']og:description["']\s+[^>]*content=["']([^"']*)["']"#,
    )
    .ok();
    let re_image = regex::Regex::new(
        r#"<meta\s+[^>]*property=["']og:image["']\s+[^>]*content=["']([^"']*)["']"#,
    )
    .ok();
    let re_meta_desc = regex::Regex::new(
        r#"<meta\s+[^>]*name=["']description["']\s+[^>]*content=["']([^"']*)["']"#,
    )
    .ok();
    let re_html_title = regex::Regex::new(r#"<title[^>]*>([^<]*)</title>"#).ok();

    meta.title = re_title
        .and_then(|r| r.captures(&html))
        .map(|c| c[1].to_string())
        .or_else(|| {
            re_html_title
                .and_then(|r| r.captures(&html))
                .map(|c| c[1].trim().to_string())
        });

    meta.description = re_desc
        .and_then(|r| r.captures(&html))
        .map(|c| c[1].to_string())
        .or_else(|| {
            re_meta_desc
                .and_then(|r| r.captures(&html))
                .map(|c| c[1].to_string())
        });

    meta.image_url = re_image
        .and_then(|r| r.captures(&html))
        .map(|c| c[1].to_string());

    meta
}

/// 本文中のURLを検出し、Blueskyの facets(リンク)配列を生成する。
///
/// `index` のオフセットは UTF-8 バイト単位で指定する必要がある(日本語など
/// マルチバイト文字が含まれるため、文字数ではなくバイト数で計算する)。
/// URL末尾に付きやすい句読点や閉じ括弧はリンクから除外する。
fn build_link_facets(text: &str) -> Vec<serde_json::Value> {
    let re = match regex::Regex::new(r"https?://[^\s]+") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let trailing: &[char] = &[
        '.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '"', '」', '』', '、', '。',
    ];

    let mut facets = Vec::new();
    for m in re.find_iter(text) {
        let trimmed = m.as_str().trim_end_matches(trailing);
        if trimmed.is_empty() {
            continue;
        }
        let byte_start = m.start();
        let byte_end = byte_start + trimmed.len();
        facets.push(json!({
            "index": { "byteStart": byte_start, "byteEnd": byte_end },
            "features": [{ "$type": "app.bsky.richtext.facet#link", "uri": trimmed }]
        }));
    }
    facets
}

/// 本文中のハッシュタグを検出し、Blueskyの tag facet を生成する。
///
/// `index` は UTF-8 バイト範囲で `#タグ` 全体を指し、`tag` には `#` を除いた
/// タグ名を入れる。半角 `#`・全角 `＃` の両方に対応する。
fn build_tag_facets(text: &str) -> Vec<serde_json::Value> {
    let re = match regex::Regex::new(r"[#＃](\w+)") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut facets = Vec::new();
    for cap in re.captures_iter(text) {
        let whole = cap.get(0).unwrap();
        let name = cap.get(1).unwrap().as_str();
        // 数字のみのタグは抽出側と同様に除外する
        if name.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        facets.push(json!({
            "index": { "byteStart": whole.start(), "byteEnd": whole.end() },
            "features": [{ "$type": "app.bsky.richtext.facet#tag", "tag": name }]
        }));
    }
    facets
}

/// 本文中の最初のURLを返す(リンクカードのURL導出用)。
fn first_url_in_text(text: &str) -> Option<String> {
    build_link_facets(text)
        .into_iter()
        .find_map(|f| f["features"][0]["uri"].as_str().map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_link_facets_byte_range() {
        let text = "更新!→ タイトル https://blog.example.com/post/1";
        let facets = build_link_facets(text);
        assert_eq!(facets.len(), 1);
        let f = &facets[0];
        let start = f["index"]["byteStart"].as_u64().unwrap() as usize;
        let end = f["index"]["byteEnd"].as_u64().unwrap() as usize;
        // バイト範囲が実際のURLを正しく指していること
        assert_eq!(
            &text.as_bytes()[start..end],
            b"https://blog.example.com/post/1"
        );
        assert_eq!(f["features"][0]["uri"], "https://blog.example.com/post/1");
    }

    #[test]
    fn test_build_link_facets_trims_trailing_punct() {
        let text = "見て (https://example.com/a)";
        let facets = build_link_facets(text);
        assert_eq!(facets.len(), 1);
        assert_eq!(facets[0]["features"][0]["uri"], "https://example.com/a");
    }

    #[test]
    fn test_build_tag_facets() {
        let text = "本文 #Rust と ＃技術";
        let facets = build_tag_facets(text);
        assert_eq!(facets.len(), 2);
        // 1つ目: #Rust
        let f0 = &facets[0];
        let s0 = f0["index"]["byteStart"].as_u64().unwrap() as usize;
        let e0 = f0["index"]["byteEnd"].as_u64().unwrap() as usize;
        assert_eq!(&text.as_bytes()[s0..e0], "#Rust".as_bytes());
        assert_eq!(f0["features"][0]["$type"], "app.bsky.richtext.facet#tag");
        assert_eq!(f0["features"][0]["tag"], "Rust");
        // 2つ目: ＃技術(全角#) → tagは"技術"
        assert_eq!(facets[1]["features"][0]["tag"], "技術");
    }

    #[test]
    fn test_first_url_in_text() {
        assert_eq!(first_url_in_text("no url here"), None);
        assert_eq!(
            first_url_in_text("a https://x.com/y b"),
            Some("https://x.com/y".to_string())
        );
    }
}
