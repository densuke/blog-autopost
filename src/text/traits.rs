use async_trait::async_trait;

use crate::article::models::Article;

/// テキストの最適化（文字数制限やレイアウト調整）を担うインターフェース
#[async_trait]
pub trait TextOptimizer {
    async fn optimize(
        &self,
        article: &Article,
        template: &str,
        max_length: usize,
        announcement: Option<&str>,
        link_weight: usize,
    ) -> anyhow::Result<String>;
}
