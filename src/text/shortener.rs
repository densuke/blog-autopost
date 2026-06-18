use super::traits::UrlShortener;
use async_trait::async_trait;
use reqwest::Client;

pub struct IsGdUrlShortener {
    client: Client,
}

impl IsGdUrlShortener {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl UrlShortener for IsGdUrlShortener {
    async fn shorten(&self, url: &str) -> anyhow::Result<String> {
        let api_url = format!(
            "https://is.gd/create.php?format=simple&url={}",
            urlencoding::encode(url)
        );

        let response = self.client.get(&api_url).send().await?;

        if response.status().is_success() {
            let short_url = response.text().await?;
            Ok(short_url.trim().to_string())
        } else {
            // エラーの場合は元のURLをそのまま返す
            let error_text = response.text().await?;
            println!("is.gd error: {}", error_text);
            Ok(url.to_string())
        }
    }
}
