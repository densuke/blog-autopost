use super::traits::ImageExtractor;
use async_trait::async_trait;
use reqwest::Client;
use regex::Regex;

pub struct OgpImageExtractor {
    client: Client,
    og_image_regex: Regex,
    twitter_image_regex: Regex,
}

impl OgpImageExtractor {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            og_image_regex: Regex::new(r#"<meta[^>]*property="og:image"[^>]*content="([^"]+)""#).unwrap(),
            twitter_image_regex: Regex::new(r#"<meta[^>]*name="twitter:image"[^>]*content="([^"]+)""#).unwrap(),
        }
    }

    /// HTMLから画像URLを抽出する。og:image を優先し、無ければ twitter:image を見る。
    /// 動画など画像でないURLは除外する(厳密な判定はダウンロード時に行う)。
    fn extract_from_html(&self, html: &str) -> Option<String> {
        for regex in [&self.og_image_regex, &self.twitter_image_regex] {
            if let Some(captures) = regex.captures(html) {
                if let Some(m) = captures.get(1) {
                    let url = m.as_str().to_string();
                    if is_probable_image_url(&url) {
                        return Some(url);
                    }
                }
            }
        }
        None
    }
}

/// URLが画像である見込みが高いかを簡易判定する。
///
/// YouTube等の動画URLや動画拡張子を除外する。フィードの media や og:image に
/// 動画URLが入っている場合があるため、それらを画像として扱わないようにする。
/// 厳密な判定(マジックバイト)はダウンロード時に行う。
pub fn is_probable_image_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();

    const NON_IMAGE_HOSTS: [&str; 3] = ["youtube.com", "youtu.be", "vimeo.com"];
    if NON_IMAGE_HOSTS.iter().any(|h| lower.contains(h)) {
        return false;
    }

    const VIDEO_EXTS: [&str; 5] = [".mp4", ".webm", ".mov", ".m3u8", ".avi"];
    let path = lower.split(['?', '#']).next().unwrap_or(&lower);
    if VIDEO_EXTS.iter().any(|e| path.ends_with(e)) {
        return false;
    }

    true
}

#[async_trait]
impl ImageExtractor for OgpImageExtractor {
    async fn extract_image(&self, article_url: &str) -> anyhow::Result<Option<String>> {
        let response = self.client.get(article_url).send().await?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let html = response.text().await?;
        Ok(self.extract_from_html(&html))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path};

    #[tokio::test]
    async fn test_extract_image_success() {
        let mock_server = MockServer::start().await;
        let html_content = r#"
            <html>
            <head>
                <title>Test Page</title>
                <meta property="og:image" content="https://example.com/images/thumb.jpg" />
            </head>
            <body>Content</body>
            </html>
        "#;

        Mock::given(method("GET"))
            .and(path("/article"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html_content))
            .mount(&mock_server)
            .await;

        let extractor = OgpImageExtractor::new();
        let article_url = format!("{}/article", mock_server.uri());
        let result = extractor.extract_image(&article_url).await.unwrap();

        assert_eq!(result, Some("https://example.com/images/thumb.jpg".to_string()));
    }

    #[tokio::test]
    async fn test_extract_image_no_og_image() {
        let mock_server = MockServer::start().await;
        let html_content = r#"
            <html>
            <head>
                <title>Test Page</title>
            </head>
            <body>Content</body>
            </html>
        "#;

        Mock::given(method("GET"))
            .and(path("/article"))
            .respond_with(ResponseTemplate::new(200).set_body_string(html_content))
            .mount(&mock_server)
            .await;

        let extractor = OgpImageExtractor::new();
        let article_url = format!("{}/article", mock_server.uri());
        let result = extractor.extract_image(&article_url).await.unwrap();

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_extract_image_http_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/article"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let extractor = OgpImageExtractor::new();
        let article_url = format!("{}/article", mock_server.uri());
        let result = extractor.extract_image(&article_url).await.unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn test_is_probable_image_url() {
        assert!(is_probable_image_url("https://example.com/a.jpg"));
        assert!(is_probable_image_url("https://example.com/a.webp?v=1"));
        // 動画ホスト・動画拡張子は除外
        assert!(!is_probable_image_url(
            "https://www.youtube.com/v/U83bjgF69g4?version=3"
        ));
        assert!(!is_probable_image_url("https://youtu.be/abcd"));
        assert!(!is_probable_image_url("https://example.com/movie.mp4"));
    }

    #[test]
    fn test_extract_from_html_falls_back_to_twitter_image() {
        let extractor = OgpImageExtractor::new();
        let html = r#"<meta name="twitter:image" content="https://example.com/t.png">"#;
        assert_eq!(
            extractor.extract_from_html(html),
            Some("https://example.com/t.png".to_string())
        );
    }

    #[test]
    fn test_extract_from_html_rejects_non_image_og() {
        let extractor = OgpImageExtractor::new();
        let html =
            r#"<meta property="og:image" content="https://www.youtube.com/v/x?version=3">"#;
        assert_eq!(extractor.extract_from_html(html), None);
    }
}
