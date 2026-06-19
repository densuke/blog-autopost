use super::traits::UrlShortener;
use async_trait::async_trait;
use reqwest::Client;

pub struct IsGdUrlShortener {
    client: Client,
    base_url: String,
}

impl IsGdUrlShortener {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://is.gd".to_string(),
        }
    }

    #[cfg(test)]
    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
}

#[async_trait]
impl UrlShortener for IsGdUrlShortener {
    async fn shorten(&self, url: &str) -> anyhow::Result<String> {
        let api_url = format!(
            "{}/create.php?format=simple&url={}",
            self.base_url,
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[tokio::test]
    async fn test_is_gd_shortener_success() {
        let mock_server = MockServer::start().await;
        let original_url = "https://example.com/very/long/url";
        let expected_short_url = "https://is.gd/xyz";

        Mock::given(method("GET"))
            .and(path("/create.php"))
            .and(query_param("format", "simple"))
            .and(query_param("url", original_url))
            .respond_with(ResponseTemplate::new(200).set_body_string(expected_short_url))
            .mount(&mock_server)
            .await;

        let shortener = IsGdUrlShortener::with_base_url(mock_server.uri());
        let result = shortener.shorten(original_url).await.unwrap();

        assert_eq!(result, expected_short_url);
    }

    #[tokio::test]
    async fn test_is_gd_shortener_error_fallback() {
        let mock_server = MockServer::start().await;
        let original_url = "https://example.com/very/long/url";

        Mock::given(method("GET"))
            .and(path("/create.php"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let shortener = IsGdUrlShortener::with_base_url(mock_server.uri());
        let result = shortener.shorten(original_url).await.unwrap();

        // エラー時は元のURLが返る
        assert_eq!(result, original_url);
    }
}
