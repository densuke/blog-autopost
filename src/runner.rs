use crate::article::models::Article;
use crate::article::traits::{ArticleStore, FeedFetcher};
use crate::config::Config;
use crate::sns::traits::SnsClient;
use crate::sns::models::PostContent;
use crate::text::traits::TextOptimizer;

pub struct Runner<F: FeedFetcher, S: ArticleStore, T: TextOptimizer> {
    fetcher: F,
    store: S,
    text_optimizer: T,
    sns_clients: Vec<Box<dyn SnsClient + Send + Sync>>,
    config: Config,
    dry_run: bool,
}

impl<F: FeedFetcher, S: ArticleStore, T: TextOptimizer> Runner<F, S, T> {
    pub fn new(
        fetcher: F,
        store: S,
        text_optimizer: T,
        sns_clients: Vec<Box<dyn SnsClient + Send + Sync>>,
        config: Config,
        dry_run: bool,
    ) -> Self {
        Self {
            fetcher,
            store,
            text_optimizer,
            sns_clients,
            config,
            dry_run,
        }
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

        // 3. SNSへの投稿ロジック
        for article in &new_articles {
            for client in &self.sns_clients {
                // テンプレートの取得
                let template_key = client.name();
                let template = self.config.templates.get(template_key)
                    .or_else(|| self.config.templates.get("default"))
                    .map(|s| s.as_str())
                    .unwrap_or("{title} {link}");

                // テキスト整形
                let optimized_text = self.text_optimizer.optimize(
                    article,
                    template,
                    client.max_characters(),
                    self.config.announcement_text.as_deref()
                ).await.unwrap_or_else(|e| {
                    println!("Failed to optimize text: {}", e);
                    article.title.clone()
                });

                let content = PostContent {
                    text: optimized_text,
                    image_url: article.image_url.clone(),
                };

                if self.dry_run {
                    println!("[DRY RUN] Would post to {} ({}):", client.name(), client.account_name());
                    println!("[DRY RUN] Content: {}", content.text);
                    continue;
                }

                println!("Posting to SNS ({} - {}): {}", client.name(), client.account_name(), article.title);
                match client.post(&content).await {
                    Ok(result) => {
                        if result.success {
                            println!("Successfully posted! URI: {:?}", result.post_id);
                        } else {
                            println!("Failed to post to {}: {:?}", client.name(), result.error_message);
                        }
                    }
                    Err(e) => {
                        println!("Error while posting to {}: {:?}", client.name(), e);
                    }
                }
            }
        }

        // 4. 処理が終わった（または投稿対象の）記事を永続化
        if self.dry_run {
            println!("[DRY RUN] Skipping saving {} articles to datastore.", new_articles.len());
        } else {
            self.store.save_articles(&new_articles).await?;
        }

        Ok(new_articles)
    }
}

