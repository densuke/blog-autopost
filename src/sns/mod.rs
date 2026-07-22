pub mod bluesky;
pub mod mastodon;
pub mod misskey;
pub mod models;
pub mod traits;
pub mod x;

use reqwest::Client;

/// バイト列が対応する画像形式かどうかをマジックバイトで判定する。
///
/// HTMLやリダイレクト結果(例: og:imageに動画URLが入っていた場合)など、
/// 画像でないものを弾くために使う。
pub fn is_supported_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes).is_ok()
}

/// 画像URLからバイナリとMIMEタイプをダウンロードする。
///
/// - ネットワーク/HTTPエラー時は `Err`
/// - 取得できたが中身が画像でない場合は `Ok(None)`(呼び出し側で静かにスキップ)
/// - 画像の場合は `Ok(Some((bytes, content_type)))`
pub async fn download_image(
    client: &Client,
    url: &str,
) -> anyhow::Result<Option<(Vec<u8>, String)>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image: {}",
            response.status()
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = response.bytes().await?.to_vec();

    if !is_supported_image(&bytes) {
        return Ok(None);
    }

    Ok(Some((bytes, content_type)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image_true_for_png() {
        // PNGのマジックバイト
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(is_supported_image(&png));
    }

    #[test]
    fn test_is_supported_image_false_for_html() {
        let html = b"<!DOCTYPE html><html><head></head></html>";
        assert!(!is_supported_image(html));
    }
}
