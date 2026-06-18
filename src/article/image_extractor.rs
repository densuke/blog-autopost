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
