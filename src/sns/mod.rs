pub mod bluesky;
pub mod mastodon;
pub mod misskey;
pub mod models;
pub mod traits;

use reqwest::Client;

/// 画像URLからバイナリとMIMEタイプをダウンロードする
pub async fn download_image(client: &Client, url: &str) -> anyhow::Result<(Vec<u8>, String)> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to download image: {}", response.status()));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = response.bytes().await?.to_vec();
    Ok((bytes, content_type))
}
