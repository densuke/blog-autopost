use crate::article::models::Article;
use crate::article::traits::{ArticleStore, FeedFetcher, ImageExtractor};
use std::sync::Arc;
use crate::config::Config;
use crate::sns::traits::SnsClient;
use crate::sns::models::PostContent;
use crate::text::traits::{TextOptimizer, UrlShortener};

pub struct Runner<F: FeedFetcher, S: ArticleStore, T: TextOptimizer, I: ImageExtractor, U: UrlShortener> {
    fetcher: F,
    store: S,
    text_optimizer: T,
    image_extractor: I,
    url_shortener: U,
    sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
    config: Config,
    dry_run: bool,
    limit: Option<usize>,
    debug: bool,
}

impl<F: FeedFetcher, S: ArticleStore, T: TextOptimizer, I: ImageExtractor, U: UrlShortener> Runner<F, S, T, I, U> {
    pub fn new(
        fetcher: F,
        store: S,
        text_optimizer: T,
        image_extractor: I,
        url_shortener: U,
        sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
        config: Config,
        dry_run: bool,
        limit: Option<usize>,
        debug: bool,
    ) -> Self {
        Self {
            fetcher,
            store,
            text_optimizer,
            image_extractor,
            url_shortener,
            sns_clients,
            config,
            dry_run,
            limit,
            debug,
        }
    }

    /// 1回分のフィードチェックと処理を実行する
    pub async fn run_once(&self, feed_url: &str, feed_name: &str) -> anyhow::Result<Vec<Article>> {
        // 1. 最新記事の取得
        if self.debug {
            println!("[DEBUG] Fetching RSS feed from: {}", feed_url);
        }
        let latest_articles = self.fetcher.fetch_articles(feed_url, feed_name).await?;
        if self.debug {
            println!("[DEBUG] Fetched {} articles from feed.", latest_articles.len());
            for (i, art) in latest_articles.iter().enumerate() {
                println!("[DEBUG]   [{}] Title: {}, Link: {}", i, art.title, art.link);
            }
        }

        // 2. 未保存の新着記事のみを抽出
        let mut new_articles = self.store.get_new_articles(latest_articles).await?;
        if self.debug {
            println!("[DEBUG] Found {} new (unposted) articles.", new_articles.len());
        }

        if let Some(limit) = self.limit {
            if new_articles.len() > limit {
                if self.debug {
                    println!("[DEBUG] Limiting new articles to {} (original: {}).", limit, new_articles.len());
                }
                new_articles.truncate(limit);
            }
        }

        if new_articles.is_empty() {
            return Ok(Vec::new());
        }

        // 3. SNSへの投稿ロジック
        for article in &mut new_articles {
            // 画像がなければ抽出しようと試みる
            if article.image_url.is_none() {
                if let Ok(Some(img_url)) = self.image_extractor.extract_image(&article.link).await {
                    article.image_url = Some(img_url);
                }
            }

            // URLの短縮
            let mut final_link = article.link.clone();
            if let Ok(short_url) = self.url_shortener.shorten(&article.link).await {
                if self.debug {
                    println!("[DEBUG] Shortened URL from '{}' to '{}'", article.link, short_url);
                }
                final_link = short_url;
            } else if self.debug {
                println!("[DEBUG] Failed to shorten URL for: {}", article.link);
            }

            // 差し替え用にクローンを作って書き換える
            let mut display_article = article.clone();
            display_article.link = final_link;

            for client in &self.sns_clients {
                // テンプレートの取得
                let template_key = client.name();
                let template = self.config.templates.get(template_key)
                    .or_else(|| self.config.templates.get("default"))
                    .map(|s| s.as_str())
                    .unwrap_or("{title} {link}");

                if self.debug {
                    println!("[DEBUG] Using template for {}: '{}'", client.name(), template);
                }

                // テキスト整形
                let optimized_text = self.text_optimizer.optimize(
                    &display_article,
                    template,
                    client.max_characters(),
                    self.config.announcement_text.as_deref()
                ).await.unwrap_or_else(|e| {
                    println!("Failed to optimize text: {}", e);
                    display_article.title.clone()
                });

                if self.debug {
                    println!("[DEBUG] Optimized text ({} chars, max {}): '{}'", optimized_text.chars().count(), client.max_characters(), optimized_text);
                }

                let content = PostContent {
                    text: optimized_text,
                    image_url: article.image_url.clone(),
                    media_paths: None,
                    link_url: None,
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

