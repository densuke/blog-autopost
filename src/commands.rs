//! サブコマンドのディスパッチ。
//!
//! 個々のコマンドの実処理は `commands/` 配下のモジュールへ分割している。
//! ここはCLIの入力を各モジュールへ振り分ける役割のみを持つ。

use blog_autopost_rs::config::Config;
use blog_autopost_rs::web;

use crate::cli::{Cli, Commands};

mod check;
mod daemon;
mod length_check;
mod list;
mod post;
mod schedule;
mod sns_clients;
mod sns_selector;
mod touch;

pub use list::{list_feeds, list_sns};

use sns_clients::{build_sns_clients, filter_sns_clients};

///
/// グローバルオプション（`--limit` / `--debug` / `--verbose` / `--sensitive` /
/// `--config`）は `cli` から参照する。
/// 設定から監視対象フィードの `(feed_url, feed_name)` 一覧を取り出す。
///
/// `feed_url` が空のフィードは除外する。Check と Touch の双方で使用し、
/// どちらも全フィードを対象にすることを保証する。
fn feed_targets(config_data: &Config) -> Vec<(String, String)> {
    config_data
        .blog
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|b| (b.feed_url, b.name))
        .filter(|(url, _)| !url.is_empty())
        .collect()
}

/// サブコマンドを受け取り、対応するモジュールへ処理を振り分ける。
///
/// グローバルオプション（`--limit` / `--debug` / `--verbose` / `--sensitive` /
/// `--config`）は `cli` から参照する。
pub async fn run_command(command: Commands, config_data: Config, cli: &Cli) -> anyhow::Result<()> {
    match command {
        Commands::Touch => touch::run(&config_data, cli).await?,
        Commands::Run { dry_run } => daemon::run(config_data, cli, dry_run).await?,
        Commands::Check { dry_run, sns } => check::run(config_data, cli, dry_run, sns).await?,
        Commands::Post {
            text,
            sns,
            instance_url,
            token,
            media,
            link,
        } => {
            let args = post::PostArgs {
                text,
                sns,
                instance_url,
                token,
                media,
                link,
            };
            post::run(args, &config_data, cli).await?
        }
        Commands::Serve { port } => {
            println!("Starting Web UI server on port {}...", port);
            web::start_server(config_data, cli.config.clone(), port).await?;
        }
        Commands::Schedule { action } => schedule::run(action, &config_data).await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use blog_autopost_rs::config::parse_config;
    use blog_autopost_rs::sns::models::{PostContent, PostResult};
    use blog_autopost_rs::sns::traits::SnsClient;

    #[test]
    fn test_feed_targets_returns_all_feeds() {
        // 複数フィード。以前は Touch が先頭のみ処理していたが、全件返すこと。
        let yaml = r#"
blog:
  - name: main
    feed_url: https://example.com/main.xml
  - name: youtube
    feed_url: https://example.com/yt.xml
  - name: zenn
    feed_url: https://example.com/zenn.xml
"#;
        let config = parse_config(yaml).unwrap();
        let targets = feed_targets(&config);
        let names: Vec<&str> = targets.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, vec!["main", "youtube", "zenn"]);
    }

    #[test]
    fn test_feed_targets_skips_empty_url() {
        let yaml = r#"
blog:
  - name: main
    feed_url: https://example.com/main.xml
  - name: broken
    feed_url: ""
"#;
        let config = parse_config(yaml).unwrap();
        let targets = feed_targets(&config);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].1, "main");
    }

    /// テスト用のダミー SNS クライアント。name() と account_name() のみ意味を持つ。
    struct DummyClient {
        kind: String,
        account: String,
    }

    #[async_trait::async_trait]
    impl SnsClient for DummyClient {
        fn name(&self) -> &str {
            &self.kind
        }
        fn account_name(&self) -> &str {
            &self.account
        }
        async fn post(&self, _content: &PostContent) -> anyhow::Result<PostResult> {
            Ok(PostResult {
                success: true,
                post_id: None,
                error_message: None,
            })
        }
        fn max_characters(&self) -> usize {
            500
        }
    }

    fn dummy(kind: &str, account: &str) -> std::sync::Arc<dyn SnsClient + Send + Sync> {
        std::sync::Arc::new(DummyClient {
            kind: kind.to_string(),
            account: account.to_string(),
        })
    }

    fn names(clients: &[std::sync::Arc<dyn SnsClient + Send + Sync>]) -> Vec<String> {
        clients
            .iter()
            .map(|c| c.account_name().to_string())
            .collect()
    }

    fn sample() -> Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> {
        vec![
            dummy("x", "x"),
            dummy("bluesky", "bluesky"),
            dummy("mastodon", "mastodon-social"),
            dummy("misskey", "misskey-io"),
        ]
    }

    #[test]
    fn test_filter_none_returns_all() {
        let result = filter_sns_clients(sample(), None);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_filter_by_kind() {
        let result = filter_sns_clients(sample(), Some("misskey"));
        assert_eq!(names(&result), vec!["misskey-io"]);
    }

    #[test]
    fn test_filter_by_account_name() {
        let result = filter_sns_clients(sample(), Some("mastodon-social"));
        assert_eq!(names(&result), vec!["mastodon-social"]);
    }

    #[test]
    fn test_filter_multiple_included() {
        let result = filter_sns_clients(sample(), Some("x,bluesky"));
        assert_eq!(names(&result), vec!["x", "bluesky"]);
    }

    #[test]
    fn test_filter_exclude() {
        let result = filter_sns_clients(sample(), Some("-x"));
        assert_eq!(
            names(&result),
            vec!["bluesky", "mastodon-social", "misskey-io"]
        );
    }

    #[test]
    fn test_filter_all_keyword() {
        let result = filter_sns_clients(sample(), Some("all"));
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_filter_case_insensitive() {
        let result = filter_sns_clients(sample(), Some("MISSKEY"));
        assert_eq!(names(&result), vec!["misskey-io"]);
    }
}
