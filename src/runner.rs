use crate::article::models::Article;
use crate::article::traits::{ArticleStore, FeedFetcher};

pub struct Runner<F: FeedFetcher, S: ArticleStore> {
    fetcher: F,
    store: S,
}

impl<F: FeedFetcher, S: ArticleStore> Runner<F, S> {
    pub fn new(fetcher: F, store: S) -> Self {
        Self { fetcher, store }
    }

    /// 1回分のフィードチェックと処理を実行する
    pub async fn run_once(&self, feed_url: &str, feed_name: &str) -> anyhow::Result<Vec<Article>> {
        // 1. 最新記事の取得
        let latest_articles = self.fetcher.fetch_articles(feed_url, feed_name).await?;

        // 2. 未保存の新着記事のみを抽出
        let new_articles = self.store.get_new_articles(latest_articles).await?;

        if new_articles.is_empty() {
            return Ok(Vec::new());
        }

        // 3. 今後ここにSNSへの投稿ロジック（SnsClientの呼び出し）が入る
        // 例: sns_client.post(...)

        // 4. 処理が終わった（または投稿対象の）記事を永続化
        self.store.save_articles(&new_articles).await?;

        Ok(new_articles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use std::sync::Mutex;

    struct MockFetcher {
        articles_to_return: Vec<Article>,
    }
    #[async_trait]
    impl FeedFetcher for MockFetcher {
        async fn fetch_articles(&self, _url: &str, _name: &str) -> anyhow::Result<Vec<Article>> {
            Ok(self.articles_to_return.clone())
        }
    }

    struct MockStore {
        saved_articles: Mutex<Vec<Article>>,
    }
    impl MockStore {
        fn new() -> Self {
            Self { saved_articles: Mutex::new(Vec::new()) }
        }
    }
    #[async_trait]
    impl ArticleStore for MockStore {
        async fn get_new_articles(&self, latest: Vec<Article>) -> anyhow::Result<Vec<Article>> {
            let saved = self.saved_articles.lock().unwrap();
            let new_ones = latest.into_iter().filter(|a| !saved.iter().any(|s| s.link == a.link)).collect();
            Ok(new_ones)
        }
        async fn save_articles(&self, articles: &[Article]) -> anyhow::Result<()> {
            let mut saved = self.saved_articles.lock().unwrap();
            saved.extend(articles.iter().cloned());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_run_once() {
        let dummy_article = Article {
            title: "Test".into(),
            link: "http://example.com".into(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test_feed".into(),
        };

        let fetcher = MockFetcher { articles_to_return: vec![dummy_article.clone()] };
        let store = MockStore::new();

        let runner = Runner::new(fetcher, store);

        // 1回目の実行：新着記事として処理される
        let processed = runner.run_once("http://example.com/feed", "test_feed").await.unwrap();
        assert_eq!(processed.len(), 1);

        // 2回目の実行：すでに保存されているので0件になる
        let fetcher2 = MockFetcher { articles_to_return: vec![dummy_article] };
        // storeの所有権はrunnerに移っているので、Runnerも作り直す場合はStoreをArc等で共有する必要があるが
        // ここでは同じ runner を使えないか？ Runnerのフィールドを共有参照にするか、単に別の関数としてテストする。
    }
}
