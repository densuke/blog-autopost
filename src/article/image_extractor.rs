use super::traits::ImageExtractor;
use async_trait::async_trait;
use reqwest::Client;
use regex::Regex;

pub struct OgpImageExtractor {
    client: Client,
    og_image_regex: Regex,
}

impl OgpImageExtractor {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            og_image_regex: Regex::new(r#"<meta[^>]*property="og:image"[^>]*content="([^"]+)""#).unwrap(),
        }
    }
}

#[async_trait]
impl ImageExtractor for OgpImageExtractor {
    async fn extract_image(&self, article_url: &str) -> anyhow::Result<Option<String>> {
        let response = self.client.get(article_url).send().await?;
        
        if !response.status().is_success() {
            return Ok(None);
        }

        let html = response.text().await?;
        
        // 正規表現で og:image を探す
        if let Some(captures) = self.og_image_regex.captures(&html) {
            if let Some(url_match) = captures.get(1) {
                return Ok(Some(url_match.as_str().to_string()));
            }
        }

        // og:image が無ければ twitter:image なども探すように拡張可能
        Ok(None)
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
}
