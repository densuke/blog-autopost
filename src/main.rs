mod article;
mod config;
mod runner;
mod sns;
mod text;

use std::fs;
use clap::{Parser, Subcommand};
use sns::models::PostContent;
use sns::traits::SnsClient;
use sns::mastodon::MastodonClient;
use config::{parse_config, SnsConfig};

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
    Run,
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
    });

    match cli.command {
        Commands::Run => {
            println!("Starting blog-autopost-rs scheduler...");
            let mut sched = tokio_cron_scheduler::JobScheduler::new().await?;
            sched.add(tokio_cron_scheduler::Job::new_async("0 * * * * *", |uuid, _| {
                Box::pin(async move {
                    println!("Cron job triggered (UUID: {})", uuid);
                    // TODO: runnerの実装を呼び出す
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
            } else {
                println!("SNS '{}' is not supported yet.", sns);
            }
        }
    }
    
    Ok(())
}
