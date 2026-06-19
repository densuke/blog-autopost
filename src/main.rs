mod article;
mod config;
mod runner;
mod sns;
mod text;
mod web;

use std::fs;
use clap::{Parser, Subcommand};
use crate::config::parse_config;
use crate::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, traits::SnsClient, x::XClient,
};
use sns::models::PostContent;
use config::SnsConfig;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 設定ファイルのパス
    #[arg(short, long, default_value = "config.yml")]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// デーモンとしてスケジューラを起動し、定期実行する
    Run {
        /// ドライランモード（実際のSNSへの投稿とDB保存を行わない）
        #[arg(long)]
        dry_run: bool,
    },
    /// 任意のテキストを指定したSNSへ手動投稿する
    Post {
        /// 投稿するテキスト
        #[arg(short, long)]
        text: String,
        
        /// 投稿先のSNS (例: 'mastodon', 'misskey')
        #[arg(short, long)]
        sns: String,
        
        /// インスタンスURL (引数で上書きする場合)
        #[arg(long, env = "SNS_URL")]
        instance_url: Option<String>,
        
        /// アクセストークン (引数で上書きする場合)
        #[arg(long, env = "SNS_TOKEN")]
        token: Option<String>,
    },
    /// 現在のRSSフィードを取得し、すべて「既読（投稿済み）」として記録する
    Touch,
    /// Web UIを起動する
    Serve {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // config.ymlの読み込み
    let config_content = fs::read_to_string(&cli.config).unwrap_or_else(|_| "".to_string());
    let config_data = parse_config(&config_content).unwrap_or_else(|_| config::Config {
        announcement_text: None,
        blog: None,
        sns: vec![],
        templates: Default::default(),
    });

    match cli.command {
        Commands::Touch => {
            println!("Fetching current RSS feed and marking all as read...");
            let blog_conf = config_data.blog.clone().and_then(|mut blogs| if blogs.is_empty() { None } else { Some(blogs.remove(0)) });
            let feed_url = blog_conf.as_ref().map(|b| b.feed_url.clone()).unwrap_or_default();
            let feed_name = blog_conf.as_ref().map(|b| b.name.clone()).unwrap_or_else(|| "default".to_string());

            if feed_url.is_empty() {
                println!("Warning: No feed_url configured. Cannot touch.");
                return Ok(());
            }

            let fetcher = article::feed_fetcher::DefaultFeedFetcher::new();
            let store = article::store::JsonArticleStore::new("data/articles.json");
            std::fs::create_dir_all("data").ok();

            use crate::article::traits::{FeedFetcher, ArticleStore};
            let latest_articles = fetcher.fetch_articles(&feed_url, &feed_name).await?;
            store.save_articles(&latest_articles).await?;
            println!("Successfully marked {} articles as read.", latest_articles.len());
        }
        Commands::Run { dry_run } => {
            println!("Starting blog-autopost-rs scheduler...");
            if dry_run {
                println!("*** DRY RUN MODE ENABLED ***");
            }
            
            // SnsClient のリストを生成
            let mut sns_clients: Vec<Box<dyn SnsClient + Send + Sync>> = Vec::new();
            for sns_conf in &config_data.sns {
                match sns_conf {
                    config::SnsConfig::Mastodon { instance_url, access_token, name, .. } => {
                        if let Ok(client) = MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                            sns_clients.push(Box::new(client));
                        }
                    }
                    config::SnsConfig::Misskey { instance_url, access_token, name, .. } => {
                        if let Ok(client) = MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                            sns_clients.push(Box::new(client));
                        }
                    }
                    config::SnsConfig::Bluesky { identifier, password, name, .. } => {
                        if let Ok(client) = BlueskyClient::new(identifier.clone(), password.clone(), name.clone()) {
                            sns_clients.push(Box::new(client));
                        }
                    }
                    config::SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                        if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
                            sns_clients.push(Box::new(client));
                        }
                    }
                    _ => {
                        println!("Unknown or unsupported SNS configuration found.");
                    }
                }
            }

            if sns_clients.is_empty() {
                println!("Warning: No valid SNS clients configured.");
            }
            
            // ブログ設定を取得（複数ある場合は最初の一つ。今後は複数対応も可能）
            let blog_conf = config_data.blog.clone().and_then(|mut blogs| if blogs.is_empty() { None } else { Some(blogs.remove(0)) });
            let feed_url = blog_conf.as_ref().map(|b| b.feed_url.clone()).unwrap_or_default();
            let feed_name = blog_conf.as_ref().map(|b| b.name.clone()).unwrap_or_else(|| "default".to_string());

            if feed_url.is_empty() {
                println!("Warning: No feed_url configured. Runner will not fetch anything.");
            }

            let fetcher = article::feed_fetcher::DefaultFeedFetcher::new();
            let store = article::store::JsonArticleStore::new("data/articles.json");
            let text_optimizer = text::optimizer::DefaultTextOptimizer::new();
            let image_extractor = article::image_extractor::OgpImageExtractor::new();
            let url_shortener = text::shortener::IsGdUrlShortener::new();
            
            // dataディレクトリが無ければ作成する
            std::fs::create_dir_all("data").ok();

            let runner = std::sync::Arc::new(runner::Runner::new(
                fetcher,
                store,
                text_optimizer,
                image_extractor,
                url_shortener,
                sns_clients,
                config_data,
                dry_run,
            ));

            let sched = tokio_cron_scheduler::JobScheduler::new().await?;
            
            let runner_clone = std::sync::Arc::clone(&runner);
            sched.add(tokio_cron_scheduler::Job::new_async("0 * * * * *", move |uuid, _| {
                let r = std::sync::Arc::clone(&runner_clone);
                let f_url = feed_url.clone();
                let f_name = feed_name.clone();
                Box::pin(async move {
                    println!("Cron job triggered (UUID: {}) - Fetching feed...", uuid);
                    match r.run_once(&f_url, &f_name).await {
                        Ok(articles) => {
                            if articles.is_empty() {
                                println!("No new articles found.");
                            } else {
                                println!("Processed {} new articles.", articles.len());
                            }
                        }
                        Err(e) => {
                            println!("Error during run_once: {:?}", e);
                        }
                    }
                })
            })?).await?;

            sched.start().await?;
            
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)).await;
        }
        Commands::Post { text, sns, instance_url, token } => {
            if sns == "mastodon" {
                // config.ymlから探す
                let conf = config_data.sns.iter().find_map(|s| match s {
                    SnsConfig::Mastodon { instance_url, access_token, .. } => Some((instance_url.clone(), access_token.clone())),
                    _ => None,
                });
                
                let url = instance_url.or_else(|| conf.as_ref().map(|c| c.0.clone()))
                    .expect("instance_url must be provided via CLI or config.yml");
                let t = token.or_else(|| conf.as_ref().map(|c| c.1.clone()))
                    .expect("token must be provided via CLI or config.yml");

                let client = MastodonClient::new(url, t, "CLI_User".to_string())?;
                let content = PostContent { text, image_url: None };
                
                println!("Posting to Mastodon...");
                let result = client.post(&content).await?;
                
                if result.success {
                    println!("Successfully posted! URL: {:?}", result.post_id);
                } else {
                    println!("Failed to post: {:?}", result.error_message);
                }
            } else if sns == "misskey" {
                // config.ymlから探す
                let conf = config_data.sns.iter().find_map(|s| match s {
                    SnsConfig::Misskey { instance_url, access_token, .. } => Some((instance_url.clone(), access_token.clone())),
                    _ => None,
                });

                let url = instance_url.or_else(|| conf.as_ref().map(|c| c.0.clone()))
                    .expect("instance_url must be provided via CLI or config.yml");
                let t = token.or_else(|| conf.as_ref().map(|c| c.1.clone()))
                    .expect("token must be provided via CLI or config.yml");

                let client = sns::misskey::MisskeyClient::new(url, t, "CLI_User".to_string())?;
                let content = PostContent { text, image_url: None };
                
                println!("Posting to Misskey...");
                let result = client.post(&content).await?;
                
                if result.success {
                    println!("Successfully posted! Note ID: {:?}", result.post_id);
                } else {
                    println!("Failed to post: {:?}", result.error_message);
                }
            } else if sns == "bluesky" {
                // config.ymlから探す
                let conf = config_data.sns.iter().find_map(|s| match s {
                    SnsConfig::Bluesky { identifier, password, .. } => Some((identifier.clone(), password.clone())),
                    _ => None,
                });

                let id = instance_url.or_else(|| conf.as_ref().map(|c| c.0.clone()))
                    .expect("identifier must be provided (via --instance-url for now) or config.yml");
                let pw = token.or_else(|| conf.as_ref().map(|c| c.1.clone()))
                    .expect("password must be provided (via --token for now) or config.yml");

                let client = sns::bluesky::BlueskyClient::new(id, pw, "CLI_User".to_string())?;
                let content = PostContent { text, image_url: None };
                
                println!("Posting to Bluesky...");
                let result = client.post(&content).await?;
                
                if result.success {
                    println!("Successfully posted! URI: {:?}", result.post_id);
                } else {
                    println!("Failed to post: {:?}", result.error_message);
                }
            } else if sns == "x" {
                // config.ymlから探す
                let conf = config_data.sns.iter().find_map(|s| match s {
                    SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, .. } => {
                        Some((consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone()))
                    }
                    _ => None,
                });

                let (ck, cs, at, ats) = conf.expect("X (Twitter) configuration must be provided in config.yml");

                let client = sns::x::XClient::new(ck, cs, at, ats, "CLI_User".to_string())?;
                let content = PostContent { text, image_url: None };
                
                println!("Posting to X (Twitter)...");
                let result = client.post(&content).await?;
                
                if result.success {
                    println!("Successfully posted! Tweet ID: {:?}", result.post_id);
                } else {
                    println!("Failed to post: {:?}", result.error_message);
                }
            } else {
                println!("SNS '{}' is not supported yet.", sns);
            }
        }
        Commands::Serve { port } => {
            println!("Starting Web UI server on port {}...", port);
            web::start_server(config_data, port).await?;
        }
    }
    
    Ok(())
}
