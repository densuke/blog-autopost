use blog_autopost_rs::config::{self, Config};
use blog_autopost_rs::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, models::PostContent,
    traits::SnsClient, x::XClient,
};
use blog_autopost_rs::{article, runner, scheduled, text, web};

use crate::cli::{Cli, Commands};

mod schedule;

/// サブコマンドを受け取り、対応する処理を実行する。
///
/// グローバルオプション（`--limit` / `--debug` / `--verbose` / `--sensitive` /
/// `--config`）は `cli` から参照する。
pub async fn run_command(command: Commands, config_data: Config, cli: &Cli) -> anyhow::Result<()> {
    match command {
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

            use blog_autopost_rs::article::traits::ArticleStore;
            let latest_articles = fetcher
                .fetch_articles_verbose(&feed_url, &feed_name, cli.verbose || cli.debug)
                .await?;
            store.save_articles(&latest_articles).await?;
            println!("Successfully marked {} articles as read.", latest_articles.len());
        }
        Commands::Run { dry_run } => {
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

            let scheduled_store = std::sync::Arc::new(scheduled::JsonScheduledPostStore::new("data/scheduled_posts.json"));
            let executor = std::sync::Arc::new(scheduled::ScheduledPostExecutor::new(
                scheduled_store,
                sns_clients,
                dry_run,
            ));

            let sched = tokio_cron_scheduler::JobScheduler::new().await?;

            // 1. RSS フィードの定期監視ジョブ
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

            // 2. 予約投稿の定期実行ジョブ
            let executor_clone = std::sync::Arc::clone(&executor);
            sched.add(tokio_cron_scheduler::Job::new_async("0 * * * * *", move |uuid, _| {
                let exec = std::sync::Arc::clone(&executor_clone);
                Box::pin(async move {
                    println!("Cron job triggered (UUID: {}) - Checking scheduled posts...", uuid);
                    if let Err(e) = exec.execute_pending_posts().await {
                        println!("Error executing scheduled posts: {:?}", e);
                    }
                })
            })?).await?;

            sched.start().await?;

            tokio::time::sleep(std::time::Duration::from_secs(60 * 60 * 24)).await;
        }
        Commands::Check { dry_run, sns } => {
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

            // 設定済みの全フィードを対象にする（Python版の通常RSS監視モード相当）
            let feeds: Vec<(String, String)> = config_data
                .blog
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|b| (b.feed_url, b.name))
                .filter(|(url, _)| !url.is_empty())
                .collect();

            if feeds.is_empty() {
                println!("Warning: No feed_url configured. Nothing to check.");
                return Ok(());
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
        }
        Commands::Post { text, sns, instance_url, token, media, link } => {
            let mut sns_clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> = Vec::new();

            // フィルタ条件の構築
            let mut included = std::collections::HashSet::new();
            let mut excluded = std::collections::HashSet::new();
            let mut has_all = false;

            if let Some(sns_arg) = &sns {
                for part in sns_arg.split(',') {
                    let part = part.trim().to_lowercase();
                    if part.is_empty() {
                        continue;
                    }
                    if part.starts_with('-') {
                        excluded.insert(part[1..].to_string());
                    } else if part == "all" {
                        has_all = true;
                    } else {
                        included.insert(part);
                    }
                }
            }

            let is_implicit_all = sns.is_none() || (included.is_empty() && !has_all);

            // 1. config.ymlの設定をパースしてフィルタリング
            for sns_conf in &config_data.sns {
                match sns_conf {
                    config::SnsConfig::Mastodon { instance_url: conf_url, access_token: conf_token, name, .. } => {
                        let lower_name = name.to_lowercase();
                        let is_targeted = is_implicit_all || has_all || included.contains("mastodon") || included.contains(&lower_name);
                        let is_excluded = excluded.contains("mastodon") || excluded.contains(&lower_name);
                        if is_targeted && !is_excluded {
                            let url = instance_url.clone().unwrap_or_else(|| conf_url.clone());
                            let tok = token.clone().unwrap_or_else(|| conf_token.clone());
                            if let Ok(client) = MastodonClient::new(url, tok, name.clone()) {
                                sns_clients.push(std::sync::Arc::new(client));
                            }
                        }
                    }
                    config::SnsConfig::Misskey { instance_url: conf_url, access_token: conf_token, name, .. } => {
                        let lower_name = name.to_lowercase();
                        let is_targeted = is_implicit_all || has_all || included.contains("misskey") || included.contains(&lower_name);
                        let is_excluded = excluded.contains("misskey") || excluded.contains(&lower_name);
                        if is_targeted && !is_excluded {
                            let url = instance_url.clone().unwrap_or_else(|| conf_url.clone());
                            let tok = token.clone().unwrap_or_else(|| conf_token.clone());
                            if let Ok(client) = MisskeyClient::new(url, tok, name.clone()) {
                                sns_clients.push(std::sync::Arc::new(client));
                            }
                        }
                    }
                    config::SnsConfig::Bluesky { identifier: conf_id, password: conf_pw, name, .. } => {
                        let lower_name = name.to_lowercase();
                        let is_targeted = is_implicit_all || has_all || included.contains("bluesky") || included.contains(&lower_name);
                        let is_excluded = excluded.contains("bluesky") || excluded.contains(&lower_name);
                        if is_targeted && !is_excluded {
                            let id = instance_url.clone().unwrap_or_else(|| conf_id.clone());
                            let pw = token.clone().unwrap_or_else(|| conf_pw.clone());
                            if let Ok(client) = BlueskyClient::new(id, pw, name.clone()) {
                                sns_clients.push(std::sync::Arc::new(client));
                            }
                        }
                    }
                    config::SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                        let lower_name = name.to_lowercase();
                        let is_targeted = is_implicit_all || has_all || included.contains("x") || included.contains(&lower_name);
                        let is_excluded = excluded.contains("x") || excluded.contains(&lower_name);
                        if is_targeted && !is_excluded {
                            if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
                                sns_clients.push(std::sync::Arc::new(client));
                            }
                        }
                    }
                    _ => {}
                }
            }

            // 2. config.ymlにマッチするものがなかった場合、CLI引数からの直接指定でフォールバック
            if sns_clients.is_empty() {
                if let Some(ref sns_val) = sns {
                    let first_sns = sns_val.split(',').next().unwrap_or("").trim();
                    if first_sns == "mastodon" {
                        let url = instance_url.clone().expect("instance_url must be provided via CLI or config.yml");
                        let tok = token.clone().expect("token must be provided via CLI or config.yml");
                        let client = MastodonClient::new(url, tok, "CLI_User".to_string())?;
                        sns_clients.push(std::sync::Arc::new(client));
                    } else if first_sns == "misskey" {
                        let url = instance_url.clone().expect("instance_url must be provided via CLI or config.yml");
                        let tok = token.clone().expect("token must be provided via CLI or config.yml");
                        let client = MisskeyClient::new(url, tok, "CLI_User".to_string())?;
                        sns_clients.push(std::sync::Arc::new(client));
                    } else if first_sns == "bluesky" {
                        let id = instance_url.clone().expect("identifier must be provided via CLI (instance_url) or config.yml");
                        let pw = token.clone().expect("password must be provided via CLI (token) or config.yml");
                        let client = BlueskyClient::new(id, pw, "CLI_User".to_string())?;
                        sns_clients.push(std::sync::Arc::new(client));
                    } else if first_sns == "x" {
                        println!("X (Twitter) requires consumer credentials in config.yml. Cannot post without configuration.");
                    }
                }
            }

            if sns_clients.is_empty() {
                println!("Error: No valid SNS target configured or specified.");
                return Ok(());
            }

            // 3. 送信前の文字数チェック
            let mut client_post_contents = Vec::new();
            let mut length_errors = Vec::new();

            for client in &sns_clients {
                let max_len = client.max_characters();

                let final_text = text.clone();
                let final_link = link.clone();

                // Blueskyはリンクカードとして添付するため本文の文字数に含めない
                let is_link_card_sns = client.name() == "bluesky";

                let mut current_len = final_text.chars().count();
                if !is_link_card_sns {
                    if let Some(ref l) = final_link {
                        // URLの文字数はSNSごとの重みで計算する(X/Mastodonは一律23文字)
                        current_len += 1 + client.url_char_weight(l); // +1 for space
                    }
                }

                if current_len > max_len {
                    length_errors.push(format!(
                        "{} ({}) - 制限: {}文字, 予定: {}文字",
                        client.name(),
                        client.account_name(),
                        max_len,
                        current_len
                    ));
                } else {
                    client_post_contents.push((
                        client.clone(),
                        PostContent {
                            text: final_text,
                            image_url: None,
                            media_paths: media.clone(),
                            link_url: final_link,
                            sensitive: cli.sensitive,
                        }
                    ));
                }
            }

            if !length_errors.is_empty() {
                println!("Error: 送信テキストがSNSの文字数上限を超えています。送信を中止しました。");
                for err in length_errors {
                    println!("  - {}", err);
                }
                return Ok(());
            }

            // 4. 選択されたすべてのSNSへ投稿する
            for (client, content) in client_post_contents {
                println!("Posting to {} ({})...", client.name(), client.account_name());
                match client.post(&content).await {
                    Ok(result) => {
                        if result.success {
                            println!("Successfully posted to {}! ID: {:?}", client.name(), result.post_id);
                        } else {
                            println!("Failed to post to {}: {:?}", client.name(), result.error_message);
                        }
                    }
                    Err(e) => {
                        println!("Error posting to {}: {:?}", client.name(), e);
                    }
                }
            }
        }
        Commands::Serve { port } => {
            println!("Starting Web UI server on port {}...", port);
            web::start_server(config_data, cli.config.clone(), port).await?;
        }
        Commands::Schedule { action } => schedule::run(action, &config_data).await?,
    }

    Ok(())
}

/// 設定から SNS クライアントのリストを構築する。
///
/// Run / Check の両コマンドで共通して使用する。生成に失敗したアカウントは
/// スキップされ、未対応の設定が見つかった場合は警告を表示する。
fn build_sns_clients(
    config_data: &config::Config,
) -> Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> {
    let mut sns_clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &config_data.sns {
        match sns_conf {
            config::SnsConfig::Mastodon { instance_url, access_token, name, .. } => {
                if let Ok(client) = MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::Misskey { instance_url, access_token, name, .. } => {
                if let Ok(client) = MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone()) {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::Bluesky { identifier, password, name, .. } => {
                if let Ok(client) = BlueskyClient::new(identifier.clone(), password.clone(), name.clone()) {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::X { consumer_key, consumer_secret, access_token, access_token_secret, name } => {
                if let Ok(client) = XClient::new(consumer_key.clone(), consumer_secret.clone(), access_token.clone(), access_token_secret.clone(), name.clone()) {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            _ => {
                println!("Unknown or unsupported SNS configuration found.");
            }
        }
    }
    sns_clients
}

/// SNS クライアントのリストを --sns 指定で絞り込む。
///
/// `spec` はカンマ区切りで、SNS種別(例: 'mastodon')またはアカウント名
/// (例: 'mastodon-social')を指定する。先頭に '-' を付けると除外、'all' で
/// 全件対象。`None` または有効な指定が無い場合は全件をそのまま返す。
/// 判定は SnsClient の name()（種別）と account_name()（アカウント名）の
/// 両方に対して大文字小文字を無視して行う。
fn filter_sns_clients(
    clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>>,
    spec: Option<&str>,
) -> Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> {
    let mut included = std::collections::HashSet::new();
    let mut excluded = std::collections::HashSet::new();
    let mut has_all = false;

    if let Some(spec) = spec {
        for part in spec.split(',') {
            let part = part.trim().to_lowercase();
            if part.is_empty() {
                continue;
            }
            if let Some(name) = part.strip_prefix('-') {
                excluded.insert(name.to_string());
            } else if part == "all" {
                has_all = true;
            } else {
                included.insert(part);
            }
        }
    }

    let is_implicit_all = spec.is_none() || (included.is_empty() && !has_all);

    clients
        .into_iter()
        .filter(|c| {
            let kind = c.name().to_lowercase();
            let account = c.account_name().to_lowercase();
            let is_targeted = is_implicit_all
                || has_all
                || included.contains(&kind)
                || included.contains(&account);
            let is_excluded = excluded.contains(&kind) || excluded.contains(&account);
            is_targeted && !is_excluded
        })
        .collect()
}

pub fn list_sns(config: &config::Config) {
    println!("=== 登録されているSNSアカウント一覧 ===");
    println!("設定形式: 配列形式（複数アカウント対応）");
    println!("登録アカウント数: {}\n", config.sns.len());

    if config.sns.is_empty() {
        println!("SNSアカウントが設定されていません。");
        println!("config.ymlを確認してください。");
        return;
    }

    for (i, sns_conf) in config.sns.iter().enumerate() {
        let num = i + 1;
        match sns_conf {
            config::SnsConfig::Mastodon { name, instance_url, access_token } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: mastodon");
                println!("   インスタンス: {}", instance_url);
                let has_creds = !access_token.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::Misskey { name, instance_url, access_token, .. } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: misskey");
                println!("   インスタンス: {}", instance_url);
                let has_creds = !access_token.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::Bluesky { name, identifier, password } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: bluesky");
                let has_creds = !identifier.is_empty() && !password.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::X { name, consumer_key, consumer_secret, access_token, access_token_secret } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: x");
                let has_creds = !consumer_key.is_empty()
                    && !consumer_secret.is_empty()
                    && !access_token.is_empty()
                    && !access_token_secret.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::Threads { name, user_id, access_token } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: threads");
                let has_creds = !user_id.is_empty() && !access_token.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::Tumblr { name, consumer_key, consumer_secret, oauth_token, oauth_secret, blog_identifier } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: tumblr");
                println!("   ブログID: {}", blog_identifier);
                let has_creds = !consumer_key.is_empty()
                    && !consumer_secret.is_empty()
                    && !oauth_token.is_empty()
                    && !oauth_secret.is_empty();
                println!("   認証情報: {}", if has_creds { "設定済み" } else { "不完全" });
            }
            config::SnsConfig::Unknown => {
                println!("{}. Unknown", num);
                println!("   SNS種別: unknown");
            }
        }
        println!();
    }

    println!("注意: --sns オプションでは上記の名前またはSNS種別を指定できます。");
}

pub fn list_feeds(config: &config::Config) {
    println!("=== 登録されているフィード一覧 ===");

    let blogs = match &config.blog {
        Some(b) => b,
        None => {
            println!("フィードが設定されていません。");
            println!("config.ymlを確認してください。");
            return;
        }
    };

    if blogs.is_empty() {
        println!("フィードが設定されていません。");
        println!("config.ymlを確認してください。");
        return;
    }

    println!("登録フィード数: {}\n", blogs.len());

    for (i, blog) in blogs.iter().enumerate() {
        let num = i + 1;
        println!("{}. {}", num, blog.name);
        println!("   フィードURL: {}", blog.feed_url);
        println!();
    }

    println!("注意: --feed オプションでは上記の名前を指定できます。");
}

#[cfg(test)]
mod tests {
    use super::*;
    use blog_autopost_rs::sns::models::{PostContent, PostResult};
    use blog_autopost_rs::sns::traits::SnsClient;

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
            Ok(PostResult { success: true, post_id: None, error_message: None })
        }
        fn max_characters(&self) -> usize {
            500
        }
    }

    fn dummy(kind: &str, account: &str) -> std::sync::Arc<dyn SnsClient + Send + Sync> {
        std::sync::Arc::new(DummyClient { kind: kind.to_string(), account: account.to_string() })
    }

    fn names(clients: &[std::sync::Arc<dyn SnsClient + Send + Sync>]) -> Vec<String> {
        clients.iter().map(|c| c.account_name().to_string()).collect()
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
        assert_eq!(names(&result), vec!["bluesky", "mastodon-social", "misskey-io"]);
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
