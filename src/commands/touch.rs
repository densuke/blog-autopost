//! `touch` サブコマンド。
//!
//! 現在のフィード記事をすべて「投稿済み(既読)」として記録する。

use blog_autopost_rs::article;
use blog_autopost_rs::config::Config;

use crate::cli::Cli;
use crate::commands::feed_targets;

/// 設定済みの全フィードを取得し、既読として保存する。
///
/// フィード単位でエラーを握って継続するため、1つのフィードの取得に
/// 失敗しても残りのフィードは処理される。
pub async fn run(config_data: &Config, cli: &Cli) -> anyhow::Result<()> {
    println!("Fetching current RSS feeds and marking all as read...");

    // 設定済みの全フィードを対象にする(Check と同様)。以前は先頭
    // フィードのみを touch しており、2つ目以降が未マークのまま残る
    // 不具合があったため、全フィードをループして処理する。
    let feeds = feed_targets(config_data);

    if feeds.is_empty() {
        println!("Warning: No feed_url configured. Cannot touch.");
        return Ok(());
    }

    let fetcher = article::feed_fetcher::DefaultFeedFetcher::new();
    let store = article::store::JsonArticleStore::new("data/articles.json");
    std::fs::create_dir_all("data").ok();

    use blog_autopost_rs::article::traits::ArticleStore;
    // フィード単位でエラーを握って継続する(Check と同様)。取得失敗が
    // 一過性(空応答やリダイレクト等で "no root element")の場合でも、
    // 1フィードの失敗で残りが未マークになる事態を避ける。
    let mut total = 0usize;
    let mut had_error = false;
    for (feed_url, feed_name) in &feeds {
        let latest_articles = match fetcher
            .fetch_articles_verbose(feed_url, feed_name, cli.verbose || cli.debug)
            .await
        {
            Ok(articles) => articles,
            Err(e) => {
                had_error = true;
                println!("Error touching feed '{}': {:?}", feed_name, e);
                continue;
            }
        };
        if let Err(e) = store.save_articles(&latest_articles).await {
            had_error = true;
            println!("Error saving feed '{}': {:?}", feed_name, e);
            continue;
        }
        println!(
            "Feed '{}': marked {} articles as read.",
            feed_name,
            latest_articles.len()
        );
        total += latest_articles.len();
    }
    println!("Successfully marked {} articles as read in total.", total);
    if had_error {
        println!("Warning: 一部フィードの処理に失敗しました。再実行すると未処理分を補えます。");
    }

    Ok(())
}
