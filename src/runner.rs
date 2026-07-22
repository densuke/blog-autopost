use crate::article::models::Article;
use crate::article::traits::{ArticleStore, FeedFetcher, ImageExtractor};
use crate::config::Config;
use crate::sns::models::PostContent;
use crate::sns::traits::SnsClient;
use crate::text::traits::TextOptimizer;
use std::sync::Arc;

pub struct Runner<F: FeedFetcher, S: ArticleStore, T: TextOptimizer, I: ImageExtractor> {
    fetcher: F,
    store: S,
    text_optimizer: T,
    image_extractor: I,
    sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
    config: Config,
    dry_run: bool,
    limit: Option<usize>,
    debug: bool,
    sensitive: bool,
}

impl<F: FeedFetcher, S: ArticleStore, T: TextOptimizer, I: ImageExtractor> Runner<F, S, T, I> {
    /// Runner を構築する。
    ///
    /// 4つの依存(フィード取得・記事保存・テキスト最適化・画像抽出)と
    /// 実行時オプションを受け取るため引数が多いが、いずれも省略できない。
    /// ビルダー化は利用箇所が1つのみで割に合わないため見送っている。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fetcher: F,
        store: S,
        text_optimizer: T,
        image_extractor: I,
        sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
        config: Config,
        dry_run: bool,
        limit: Option<usize>,
        debug: bool,
        sensitive: bool,
    ) -> Self {
        Self {
            fetcher,
            store,
            text_optimizer,
            image_extractor,
            sns_clients,
            config,
            dry_run,
            limit,
            debug,
            sensitive,
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
            println!(
                "[DEBUG] Fetched {} articles from feed.",
                latest_articles.len()
            );
            for (i, art) in latest_articles.iter().enumerate() {
                println!("[DEBUG]   [{}] Title: {}, Link: {}", i, art.title, art.link);
            }
        }

        // 2. 未保存の新着記事のみを抽出
        let mut new_articles = self.store.get_new_articles(latest_articles).await?;
        if self.debug {
            println!(
                "[DEBUG] Found {} new (unposted) articles.",
                new_articles.len()
            );
        }

        if let Some(limit) = self.limit
            && new_articles.len() > limit
        {
            if self.debug {
                println!(
                    "[DEBUG] Limiting new articles to {} (original: {}).",
                    limit,
                    new_articles.len()
                );
            }
            new_articles.truncate(limit);
        }

        if new_articles.is_empty() {
            return Ok(Vec::new());
        }

        // 3. SNSへの投稿ロジック
        for article in &mut new_articles {
            // 画像がなければ抽出しようと試みる
            if article.image_url.is_none()
                && let Ok(Some(img_url)) = self.image_extractor.extract_image(&article.link).await
            {
                article.image_url = Some(img_url);
            }

            // 記事のリンクはそのまま使用する。
            // is.gd等による短縮は廃止した。X(t.co)やMastodonはURLを一律23文字に
            // 丸めて扱い、Bluesky/Misskeyも実URLで問題ないため短縮は不要であり、
            // 短縮サービスのエラー応答がURLを破壊する不具合を避ける狙いもある。
            let display_article = article.clone();

            // 付与するタグを決める。タイトルに既出のタグは重複を避けて除外し、
            // 多すぎないよう最大5個までとする。
            const MAX_TAGS: usize = 5;
            let title_tags: std::collections::HashSet<String> =
                crate::text::tags::extract_hashtags(&display_article.title)
                    .into_iter()
                    .collect();
            let post_tags: Vec<String> = display_article
                .tags
                .iter()
                .filter(|t| !title_tags.contains(*t))
                .take(MAX_TAGS)
                .cloned()
                .collect();

            for client in &self.sns_clients {
                // テンプレートの取得
                let template_key = client.name();
                let template = self
                    .config
                    .templates
                    .get(template_key)
                    .or_else(|| self.config.templates.get("default"))
                    .map(|s| s.as_str())
                    .unwrap_or("{title} {link}");

                if self.debug {
                    println!(
                        "[DEBUG] Using template for {}: '{}'",
                        client.name(),
                        template
                    );
                }

                // テキスト整形
                let link_weight = client.url_char_weight(&display_article.link);
                let optimized_text = self
                    .text_optimizer
                    .optimize(
                        &display_article,
                        template,
                        client.max_characters(),
                        self.config.announcement_text.as_deref(),
                        link_weight,
                        &post_tags,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        println!("Failed to optimize text: {}", e);
                        display_article.title.clone()
                    });

                if self.debug {
                    println!(
                        "[DEBUG] Optimized text ({} chars, max {}): '{}'",
                        optimized_text.chars().count(),
                        client.max_characters(),
                        optimized_text
                    );
                }

                let content = PostContent {
                    text: optimized_text,
                    image_url: article.image_url.clone(),
                    media_paths: None,
                    link_url: None,
                    sensitive: self.sensitive,
                };

                if self.dry_run {
                    println!(
                        "[DRY RUN] Would post to {} ({}):",
                        client.name(),
                        client.account_name()
                    );
                    println!("[DRY RUN] Content: {}", content.text);
                    continue;
                }

                println!(
                    "Posting to SNS ({} - {}): {}",
                    client.name(),
                    client.account_name(),
                    article.title
                );
                match client.post(&content).await {
                    Ok(result) => {
                        if result.success {
                            println!("Successfully posted! URI: {:?}", result.post_id);
                        } else {
                            println!(
                                "Failed to post to {}: {:?}",
                                client.name(),
                                result.error_message
                            );
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
            println!(
                "[DRY RUN] Skipping saving {} articles to datastore.",
                new_articles.len()
            );
        } else {
            self.store.save_articles(&new_articles).await?;
        }

        Ok(new_articles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::article::models::Article;
    use crate::article::traits::{ArticleStore, FeedFetcher, ImageExtractor};
    use crate::sns::models::{PostContent, PostResult};
    use crate::sns::traits::SnsClient;
    use crate::text::traits::TextOptimizer;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    struct MockFeedFetcher {
        articles: Vec<Article>,
    }

    #[async_trait]
    impl FeedFetcher for MockFeedFetcher {
        async fn fetch_articles(
            &self,
            _feed_url: &str,
            _feed_name: &str,
        ) -> anyhow::Result<Vec<Article>> {
            Ok(self.articles.clone())
        }
    }

    struct MockArticleStore {
        saved: Arc<Mutex<Vec<Article>>>,
    }

    #[async_trait]
    impl ArticleStore for MockArticleStore {
        async fn get_new_articles(
            &self,
            latest_articles: Vec<Article>,
        ) -> anyhow::Result<Vec<Article>> {
            Ok(latest_articles)
        }
        async fn save_articles(&self, articles: &[Article]) -> anyhow::Result<()> {
            let mut guard = self.saved.lock().unwrap();
            guard.extend_from_slice(articles);
            Ok(())
        }
    }

    struct MockImageExtractor;
    #[async_trait]
    impl ImageExtractor for MockImageExtractor {
        async fn extract_image(&self, _article_url: &str) -> anyhow::Result<Option<String>> {
            Ok(Some("http://example.com/extracted.jpg".to_string()))
        }
    }

    struct MockTextOptimizer;
    #[async_trait]
    impl TextOptimizer for MockTextOptimizer {
        async fn optimize(
            &self,
            article: &Article,
            template: &str,
            _max_length: usize,
            _announcement: Option<&str>,
            _link_weight: usize,
            _tags: &[String],
        ) -> anyhow::Result<String> {
            Ok(template
                .replace("{title}", &article.title)
                .replace("{link}", &article.link))
        }
    }

    struct MockSnsClient {
        name: String,
        posted: Arc<Mutex<Vec<PostContent>>>,
    }

    #[async_trait]
    impl SnsClient for MockSnsClient {
        fn name(&self) -> &str {
            &self.name
        }
        fn account_name(&self) -> &str {
            "mock-account"
        }
        async fn post(&self, content: &PostContent) -> anyhow::Result<PostResult> {
            let mut guard = self.posted.lock().unwrap();
            guard.push(content.clone());
            Ok(PostResult {
                success: true,
                post_id: Some("mock-post-123".to_string()),
                error_message: None,
            })
        }
        fn max_characters(&self) -> usize {
            280
        }
    }

    #[tokio::test]
    async fn test_runner_run_once_normal() {
        let saved_articles = Arc::new(Mutex::new(Vec::new()));
        let posted_contents = Arc::new(Mutex::new(Vec::new()));

        let fetcher = MockFeedFetcher {
            articles: vec![Article {
                title: "Test Article 1".to_string(),
                link: "http://example.com/1".to_string(),
                published_parsed: chrono::Utc::now(),
                image_url: None,
                feed_name: "test-feed".to_string(),
                tags: Vec::new(),
            }],
        };
        let store = MockArticleStore {
            saved: saved_articles.clone(),
        };
        let text_optimizer = MockTextOptimizer;
        let image_extractor = MockImageExtractor;

        let sns_client = Arc::new(MockSnsClient {
            name: "mastodon".to_string(),
            posted: posted_contents.clone(),
        });

        let mut config = Config {
            announcement_text: None,
            blog: None,
            sns: vec![],
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: None,
            extra: HashMap::new(),
        };
        config
            .templates
            .insert("default".to_string(), "{title} {link}".to_string());

        let runner = Runner::new(
            fetcher,
            store,
            text_optimizer,
            image_extractor,
            vec![sns_client],
            config,
            false, // dry_run
            None,  // limit
            true,  // debug
            false, // sensitive
        );

        let result = runner
            .run_once("https://blog.example.com/feed.xml", "test-feed")
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Test Article 1");
        assert_eq!(
            result[0].image_url,
            Some("http://example.com/extracted.jpg".to_string())
        );

        // 保存された件数
        let saved = saved_articles.lock().unwrap();
        assert_eq!(saved.len(), 1);

        // 投稿された件数と内容
        let posted = posted_contents.lock().unwrap();
        assert_eq!(posted.len(), 1);
        // 短縮を廃止したため、実URLがそのまま投稿される
        assert_eq!(posted[0].text, "Test Article 1 http://example.com/1");
    }

    #[tokio::test]
    async fn test_runner_run_once_limit() {
        let saved_articles = Arc::new(Mutex::new(Vec::new()));
        let posted_contents = Arc::new(Mutex::new(Vec::new()));

        let fetcher = MockFeedFetcher {
            articles: vec![
                Article {
                    title: "Test 1".to_string(),
                    link: "http://example.com/1".to_string(),
                    published_parsed: chrono::Utc::now(),
                    image_url: None,
                    feed_name: "test-feed".to_string(),
                    tags: Vec::new(),
                },
                Article {
                    title: "Test 2".to_string(),
                    link: "http://example.com/2".to_string(),
                    published_parsed: chrono::Utc::now(),
                    image_url: None,
                    feed_name: "test-feed".to_string(),
                    tags: Vec::new(),
                },
            ],
        };
        let store = MockArticleStore {
            saved: saved_articles.clone(),
        };
        let text_optimizer = MockTextOptimizer;
        let image_extractor = MockImageExtractor;

        let sns_client = Arc::new(MockSnsClient {
            name: "mastodon".to_string(),
            posted: posted_contents.clone(),
        });

        let runner = Runner::new(
            fetcher,
            store,
            text_optimizer,
            image_extractor,
            vec![sns_client],
            Config {
                announcement_text: None,
                blog: None,
                sns: vec![],
                templates: HashMap::new(),
                default_allowed_timings: None,
                allowed_timings_tolerance_minutes: None,
                allowed_timings: None,
                web_auth: None,
                extra: HashMap::new(),
            },
            false,   // dry_run
            Some(1), // limit = 1
            false,   // debug
            false,   // sensitive
        );

        let result = runner
            .run_once("https://blog.example.com/feed.xml", "test-feed")
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].title, "Test 1");
    }
}
