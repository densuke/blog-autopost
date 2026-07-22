//! `run` サブコマンド。
//!
//! スケジューラを起動し、RSS監視と予約投稿の実行を定期的に行い続ける。

use blog_autopost_rs::config::Config;
use blog_autopost_rs::{article, runner, scheduled, text};

use crate::cli::Cli;
use crate::commands::build_sns_clients;

/// デーモンとしてスケジューラを起動する。
pub async fn run(config_data: Config, cli: &Cli, dry_run: bool) -> anyhow::Result<()> {
    println!("Starting blog-autopost-rs scheduler...");
    if dry_run {
        println!("*** DRY RUN MODE ENABLED ***");
    }

    // SnsClient のリストを生成
    let sns_clients = build_sns_clients(&config_data);

    if sns_clients.is_empty() {
        println!("Warning: No valid SNS clients configured.");
    }

    // ブログ設定を取得（複数ある場合は最初の一つ。今後は複数対応も可能）
    let blog_conf = config_data.blog.clone().and_then(|mut blogs| {
        if blogs.is_empty() {
            None
        } else {
            Some(blogs.remove(0))
        }
    });
    let feed_url = blog_conf
        .as_ref()
        .map(|b| b.feed_url.clone())
        .unwrap_or_default();
    let feed_name = blog_conf
        .as_ref()
        .map(|b| b.name.clone())
        .unwrap_or_else(|| "default".to_string());

    if feed_url.is_empty() {
        println!("Warning: No feed_url configured. Runner will not fetch anything.");
    }

    let fetcher = article::feed_fetcher::DefaultFeedFetcher::new();
    let store = article::store::JsonArticleStore::new("data/articles.json");
    let text_optimizer = text::optimizer::DefaultTextOptimizer::new();
    let image_extractor = article::image_extractor::OgpImageExtractor::new();

    // dataディレクトリが無ければ作成する
    std::fs::create_dir_all("data").ok();

    let runner = std::sync::Arc::new(runner::Runner::new(
        fetcher,
        store,
        text_optimizer,
        image_extractor,
        sns_clients.clone(),
        config_data,
        dry_run,
        cli.limit,
        cli.debug,
        cli.sensitive,
    ));

    let scheduled_store = std::sync::Arc::new(scheduled::JsonScheduledPostStore::new(
        "data/scheduled_posts.json",
    ));
    let executor = std::sync::Arc::new(scheduled::ScheduledPostExecutor::new(
        scheduled_store,
        sns_clients,
        dry_run,
    ));

    let sched = tokio_cron_scheduler::JobScheduler::new().await?;

    // 1. RSS フィードの定期監視ジョブ
    let runner_clone = std::sync::Arc::clone(&runner);
    sched
        .add(tokio_cron_scheduler::Job::new_async(
            "0 * * * * *",
            move |uuid, _| {
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
            },
        )?)
        .await?;

    // 2. 予約投稿の定期実行ジョブ
    let executor_clone = std::sync::Arc::clone(&executor);
    sched
        .add(tokio_cron_scheduler::Job::new_async(
            "0 * * * * *",
            move |uuid, _| {
                let exec = std::sync::Arc::clone(&executor_clone);
                Box::pin(async move {
                    println!(
                        "Cron job triggered (UUID: {}) - Checking scheduled posts...",
                        uuid
                    );
                    if let Err(e) = exec.execute_pending_posts().await {
                        println!("Error executing scheduled posts: {:?}", e);
                    }
                })
            },
        )?)
        .await?;

    sched.start().await?;

    tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)).await;

    Ok(())
}
