use async_trait::async_trait;

/// テキストの最適化（文字数制限やレイアウト調整）を担うインターフェース
#[async_trait]
pub trait TextOptimizer {
    async fn optimize(
        &self,
        title: &str,
        link: &str,
        sns_type: &str,
        announcement: Option<&str>,
        max_length: usize,
    ) -> anyhow::Result<String>;
}

/// URLの短縮（is.gd等）を担うインターフェース
#[async_trait]
pub trait UrlShortener {
    async fn shorten(&self, url: &str) -> anyhow::Result<String>;
}
