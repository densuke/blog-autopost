//! `check` サブコマンド。
//!
//! RSSフィードを一度だけ確認し、新着記事を各SNSへ投稿する。

use blog_autopost_rs::config::Config;
use blog_autopost_rs::{article, runner, text};

use crate::cli::Cli;
use crate::commands::{build_sns_clients, feed_targets, filter_feed_targets, filter_sns_clients};

/// フィードを1回ずつ確認し、新着記事を投稿する。
///
/// `feed` に指定があればそのフィードだけを対象にする（Agy #365）。
pub async fn run(
    config_data: Config,
    cli: &Cli,
    dry_run: bool,
    sns: Option<String>,
    feed: Option<String>,
) -> anyhow::Result<()> {
    println!("Checking RSS feeds for new articles...");
    if dry_run {
        println!("*** DRY RUN MODE ENABLED ***");
    }

    // SnsClient のリストを生成し、--sns 指定があれば絞り込む
    let sns_clients = filter_sns_clients(build_sns_clients(&config_data), sns.as_deref());
    if sns_clients.is_empty() {
        println!("Warning: No valid SNS clients configured.");
    } else if cli.debug {
        let names: Vec<String> = sns_clients
            .iter()
            .map(|c| format!("{} ({})", c.name(), c.account_name()))
            .collect();
        println!("[DEBUG] 投稿対象SNS: {}", names.join(", "));
    }

    // 設定済みのフィードを対象にする（Python版の通常RSS監視モード相当）。
    // --feed 指定があれば絞り込む（Agy #365）
    let all_feeds = feed_targets(&config_data);
    if all_feeds.is_empty() {
        println!("Warning: No feed_url configured. Nothing to check.");
        return Ok(());
    }

    let feeds = filter_feed_targets(all_feeds, feed.as_deref());
    if feeds.is_empty() {
        // 指定名が設定に無い場合。黙って全件処理すると事故になるため明示的に止める
        println!(
            "Warning: --feed '{}' matched no configured feed. Nothing to check.",
            feed.as_deref().unwrap_or("")
        );
        return Ok(());
    }
    if cli.debug && feed.is_some() {
        let names: Vec<&str> = feeds.iter().map(|(_, n)| n.as_str()).collect();
        println!("[DEBUG] 対象フィード: {}", names.join(", "));
    }

    let fetcher = article::feed_fetcher::DefaultFeedFetcher::new();
    let store = article::store::JsonArticleStore::new("data/articles.json");
    let text_optimizer = text::optimizer::DefaultTextOptimizer::new();
    let image_extractor = article::image_extractor::OgpImageExtractor::new();
    std::fs::create_dir_all("data").ok();

    let runner = runner::Runner::new(
        fetcher,
        store,
        text_optimizer,
        image_extractor,
        sns_clients,
        config_data,
        dry_run,
        cli.limit,
        cli.debug,
        cli.sensitive,
    );

    let mut total = 0usize;
    for (feed_url, feed_name) in &feeds {
        println!("--- Feed: {} ({}) ---", feed_name, feed_url);
        match runner.run_once(feed_url, feed_name).await {
            Ok(articles) => {
                if articles.is_empty() {
                    println!("No new articles found.");
                } else {
                    println!("Processed {} new articles.", articles.len());
                    total += articles.len();
                }
            }
            Err(e) => {
                println!("Error checking feed '{}': {:?}", feed_name, e);
            }
        }
    }
    println!("Done. Total {} new articles processed.", total);

    Ok(())
}
