use super::models::Article;
use async_trait::async_trait;

/// フィードURLから記事リストを取得するインターフェース
#[async_trait]
pub trait FeedFetcher {
    async fn fetch_articles(&self, feed_url: &str, feed_name: &str)
    -> anyhow::Result<Vec<Article>>;
}

/// 既読記事の保存や、新着記事のみを抽出するインターフェース
#[async_trait]
pub trait ArticleStore {
    /// 与えられた最新記事のリストから、まだ保存されていない新着記事だけを抽出して返す
    async fn get_new_articles(&self, latest_articles: Vec<Article>) -> anyhow::Result<Vec<Article>>;

    /// 投稿済みの記事として保存する
    async fn save_articles(&self, articles: &[Article]) -> anyhow::Result<()>;
}

/// 記事のURLや内容から代表画像（OGP等）を抽出するインターフェース
#[allow(dead_code)]
#[async_trait]
pub trait ImageExtractor {
    async fn extract_image(&self, article_url: &str) -> anyhow::Result<Option<String>>;
}
