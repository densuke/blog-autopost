mod article;
mod config;
mod runner;
mod sns;
mod text;

use clap::{Parser, Subcommand};
use sns::models::PostContent;
use sns::traits::SnsClient;
use sns::mastodon::MastodonClient;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
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
        
        /// 投稿先のSNS (現状は 'mastodon')
        #[arg(short, long)]
        sns: String,
        
        /// Mastodon インスタンスURL (環境変数 MASTODON_URL でも可)
        #[arg(long, env = "MASTODON_URL")]
        instance_url: Option<String>,
        
        /// Mastodon アクセストークン (環境変数 MASTODON_TOKEN でも可)
        #[arg(long, env = "MASTODON_TOKEN")]
        token: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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
            
            // メインスレッドが終了しないように待機
            tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)).await;
        }
        Commands::Post { text, sns, instance_url, token } => {
            if sns == "mastodon" {
                let url = instance_url.expect("instance_url or MASTODON_URL must be provided");
                let t = token.expect("token or MASTODON_TOKEN must be provided");

                let client = MastodonClient::new(url, t, "CLI_User".to_string())?;
                let content = PostContent { text, image_url: None };
                
                println!("Posting to Mastodon...");
                let result = client.post(&content).await?;
                
                if result.success {
                    println!("Successfully posted! URL: {:?}", result.post_id);
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
