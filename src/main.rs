use std::fs;
use clap::{Parser, Subcommand};
use blog_autopost_rs::config::{self, parse_config};
use blog_autopost_rs::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, traits::SnsClient, x::XClient,
    models::PostContent,
};
use blog_autopost_rs::{article, runner, scheduled, text, web, timing};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 設定ファイルのパス
    #[arg(short, long, default_value = "config.yml")]
    config: String,

    /// 新着記事のチェック時に処理する記事数を制限
    #[arg(short, long)]
    limit: Option<usize>,

    /// 詳細なデバッグログを表示
    #[arg(long)]
    debug: bool,

    /// フィード取得などの詳細な診断情報を表示します
    #[arg(short, long)]
    verbose: bool,

    /// 登録されているSNSアカウントの一覧を表示します
    #[arg(long)]
    list_sns: bool,

    /// 登録されているフィードの一覧を表示します
    #[arg(long)]
    list_feeds: bool,

    #[command(subcommand)]
    command: Option<Commands>,
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
        
        /// 投稿先のSNS (例: 'mastodon', 'misskey')。省略または'all'指定時は全SNSが対象
        #[arg(short, long)]
        sns: Option<String>,
        
        /// インスタンスURL (引数で上書きする場合)
        #[arg(long, env = "SNS_URL")]
        instance_url: Option<String>,
        
        /// アクセストークン (引数で上書きする場合)
        #[arg(long, env = "SNS_TOKEN")]
        token: Option<String>,

        /// 添付するローカルの画像ファイルパス（複数指定可）
        #[arg(short, long)]
        media: Option<Vec<String>>,

        /// 添付するリンクURL
        #[arg(short, long)]
        link: Option<String>,
    },
    /// 現在のRSSフィードを取得し、すべて「既読（投稿済み）」として記録する
    Touch,
    /// Web UIを起動する
    Serve {
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
    /// 予約投稿を管理する（一覧、追加、削除、変更）
    Schedule {
        #[command(subcommand)]
        action: ScheduleAction,
    },
}

#[derive(Subcommand, Debug, Clone)]
enum ScheduleAction {
    /// 予約一覧を表示する
    List {
        /// 特定のステータスでフィルタリングする（例: '予約済み', '投稿済み', '失敗'）
        #[arg(short, long)]
        status: Option<String>,
    },
    /// 新しい予約投稿を追加する
    Add {
        /// 投稿するテキスト
        #[arg(short, long)]
        text: String,

        /// 投稿予定時刻 (RFC3339形式。例: '2026-06-20T15:00:00+09:00' もしくは 'YYYY-MM-DD HH:MM')
        #[arg(short, long)]
        at: Option<String>,

        /// 自動で空いている次の投稿枠を検索して設定する
        #[arg(long)]
        auto_slot: bool,

        /// 投稿先のSNS (例: 'mastodon', 'misskey')。カンマ区切りで複数指定可。省略時は全SNS
        #[arg(short, long)]
        sns: Option<String>,

        /// 添付するローカルの画像ファイルパス（複数指定可）
        #[arg(short, long)]
        media: Option<Vec<String>>,

        /// 添付するリンクURL
        #[arg(short, long)]
        link: Option<String>,
    },
    /// 予約投稿を削除する
    Delete {
        /// 削除する予約投稿 of ID
        id: String,
    },
    /// 予約投稿を変更する
    Update {
        /// 変更する予約投稿のID
        id: String,

        /// 変更後のテキスト
        #[arg(short, long)]
        text: Option<String>,

        /// 変更後の投稿予定時刻 (RFC3339形式)
        #[arg(short, long)]
        at: Option<String>,

        /// 変更後のSNS（カンマ区切り。例: 'mastodon,bluesky'）
        #[arg(short, long)]
        sns: Option<String>,

        /// 変更後のステータス（'予約済み', '投稿済み', '失敗'）
        #[arg(long)]
        status: Option<String>,

        /// 変更後のリンクURL
        #[arg(short, long)]
        link: Option<String>,
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
        default_allowed_timings: None,
        allowed_timings_tolerance_minutes: None,
        allowed_timings: None,
        web_auth: None,
        extra: Default::default(),
    });

    if cli.list_sns {
        list_sns(&config_data);
        return Ok(());
    }

    if cli.list_feeds {
        list_feeds(&config_data);
        return Ok(());
    }

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            return Ok(());
        }
    };

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

            use crate::article::traits::ArticleStore;
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
                sns_clients.clone(),
                config_data,
                dry_run,
                cli.limit,
                cli.debug,
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

            // 3. 送信前チェックとURL短縮の適用
            use crate::text::traits::UrlShortener;
            let shortener = crate::text::shortener::IsGdUrlShortener::new();

            let mut client_post_contents = Vec::new();
            let mut length_errors = Vec::new();

            for client in &sns_clients {
                let max_len = client.max_characters();
                
                let final_text = text.clone();
                let mut final_link = link.clone();

                let is_link_card_sns = client.name() == "bluesky";
                
                let mut current_len = final_text.chars().count();
                if !is_link_card_sns {
                    if let Some(ref l) = final_link {
                        current_len += 1 + l.chars().count(); // +1 for space
                    }
                }

                if current_len > max_len {
                    if let Some(ref l) = final_link {
                        println!("URL is too long for {} (limit: {} chars). Trying to shorten...", client.name(), max_len);
                        match shortener.shorten(l).await {
                            Ok(short_url) => {
                                final_link = Some(short_url.clone());
                                current_len = final_text.chars().count();
                                if !is_link_card_sns {
                                    current_len += 1 + short_url.chars().count();
                                }
                            }
                            Err(e) => {
                                println!("Warning: Failed to shorten URL for {}: {:?}", client.name(), e);
                            }
                        }
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
        Commands::Schedule { action } => {
            let scheduled_store = scheduled::JsonScheduledPostStore::new("data/scheduled_posts.json");
            std::fs::create_dir_all("data").ok();

            match action {
                ScheduleAction::List { status } => {
                    let posts = scheduled_store.get_all_posts().await?;
                    let mut filtered_posts = posts;
                    if let Some(status_filter) = status {
                        filtered_posts.retain(|p| p.status == status_filter);
                    }

                    // 予定時間順にソート
                    filtered_posts.sort_by_key(|p| p.scheduled_at);

                    println!("{:<25} {:<20} {:<20} {:<10} {}", "ID", "Scheduled At", "SNS", "Status", "Content Preview");
                    println!("{}", "-".repeat(100));
                    for post in filtered_posts {
                        let content_preview = if post.content.chars().count() > 30 {
                            format!("{}...", post.content.chars().take(27).collect::<String>())
                        } else {
                            post.content.clone()
                        };
                        let sns_str = post.target_sns.join(",");
                        println!(
                            "{:<25} {:<20} {:<20} {:<10} {}",
                            post.id,
                            post.scheduled_at.format("%Y-%m-%d %H:%M:%S"),
                            sns_str,
                            post.status,
                            content_preview
                        );
                    }
                }
                ScheduleAction::Add { text, at, auto_slot, sns, media, link } => {
                    // SNS ターゲットの決定
                    let mut target_sns = Vec::new();
                    if let Some(sns_arg) = sns {
                        for part in sns_arg.split(',') {
                            let part = part.trim();
                            if !part.is_empty() {
                                target_sns.push(part.to_string());
                            }
                        }
                    } else {
                        // 省略時は config.yml 内 of 全 SNS を取得
                        for sns_conf in &config_data.sns {
                            let name = match sns_conf {
                                config::SnsConfig::Mastodon { name, .. } => name,
                                config::SnsConfig::Misskey { name, .. } => name,
                                config::SnsConfig::Bluesky { name, .. } => name,
                                config::SnsConfig::X { name, .. } => name,
                                config::SnsConfig::Threads { name, .. } => name,
                                config::SnsConfig::Tumblr { name, .. } => name,
                                _ => continue,
                            };
                            target_sns.push(name.clone());
                        }
                    }

                    if target_sns.is_empty() {
                        println!("Error: No target SNS configured or specified.");
                        return Ok(());
                    }

                    // 画像ファイルのコピー（退避）処理
                    let mut processed_media = Vec::new();
                    if let Some(ref media_list) = media {
                        if let Err(e) = std::fs::create_dir_all("data/uploads") {
                            println!("Error: Failed to create uploads directory: {:?}", e);
                            return Ok(());
                        }
                        for file_path in media_list {
                            let path = std::path::Path::new(file_path);
                            if !path.exists() {
                                println!("Error: Media file not found: {}", file_path);
                                return Ok(());
                            }
                            
                            let file_name = path.file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("image.png");
                                
                            let sanitized_name: String = file_name
                                .chars()
                                .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
                                .collect();
                                
                            let timestamp = chrono::Utc::now().timestamp_micros();
                            let unique_name = format!("{}_{}", timestamp, sanitized_name);
                            let save_path = format!("data/uploads/{}", unique_name);
                            
                            if let Err(e) = std::fs::copy(file_path, &save_path) {
                                println!("Error: Failed to copy media file {} to {}: {:?}", file_path, save_path, e);
                                return Ok(());
                            }
                            
                            processed_media.push(save_path);
                        }
                    }

                    // 時刻の決定
                    use chrono::TimeZone;
                    let scheduled_time = if auto_slot {
                        let timing_manager = timing::TimingManager::new(&config_data);
                        let finder = timing::SlotFinder::new(&timing_manager, &scheduled_store, 5);
                        
                        let mut created_posts = Vec::new();
                        for sns_name in &target_sns {
                            match finder.find_next_available_slot(sns_name, None, 7).await {
                                Ok(Some(dt)) => {
                                    let mut post = scheduled::ScheduledPost::new(
                                        text.clone(),
                                        dt,
                                        processed_media.clone(),
                                        vec![sns_name.clone()],
                                    );
                                    post.link_url = link.clone();
                                    let created = scheduled_store.create_post(post).await?;
                                    created_posts.push(created);
                                }
                                Ok(None) => {
                                    println!("Warning: No available slot found for SNS: {}", sns_name);
                                }
                                Err(e) => {
                                    println!("Error calculating slot for SNS {}: {:?}", sns_name, e);
                                }
                            }
                        }

                        if created_posts.is_empty() {
                            println!("Error: Failed to schedule post on any SNS via auto-slot.");
                            return Ok(());
                        }

                        println!("Successfully scheduled {} posts via auto-slot:", created_posts.len());
                        for p in created_posts {
                            println!("  - ID: {} | Time: {} | SNS: {:?}", p.id, p.scheduled_at.format("%Y-%m-%d %H:%M:%S"), p.target_sns);
                        }
                        return Ok(());
                    } else if let Some(at_str) = at {
                        let parsed_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&at_str) {
                            dt.with_timezone(&chrono::Local)
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M:%S") {
                            chrono::Local.from_local_datetime(&dt).unwrap()
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M") {
                            chrono::Local.from_local_datetime(&dt).unwrap()
                        } else {
                            println!("Error: Invalid datetime format. Use RFC3339 (e.g., 2026-06-20T15:00:00+09:00) or 'YYYY-MM-DD HH:MM:SS'");
                            return Ok(());
                        };
                        parsed_time
                    } else {
                        println!("Error: Either --at or --auto-slot must be specified.");
                        return Ok(());
                    };

                    let mut post = scheduled::ScheduledPost::new(
                        text,
                        scheduled_time,
                        processed_media,
                        target_sns,
                    );
                    post.link_url = link;

                    let created = scheduled_store.create_post(post).await?;
                    println!("Successfully scheduled post:");
                    println!("  ID: {}", created.id);
                    println!("  Time: {}", created.scheduled_at.format("%Y-%m-%d %H:%M:%S"));
                    println!("  SNS: {:?}", created.target_sns);
                }
                ScheduleAction::Delete { id } => {
                    let success = scheduled_store.delete_post(&id).await?;
                    if success {
                        println!("Successfully deleted scheduled post: {}", id);
                    } else {
                        println!("Error: Scheduled post not found: {}", id);
                    }
                }
                ScheduleAction::Update { id, text, at, sns, status, link } => {
                    let opt_post = scheduled_store.get_post_by_id(&id).await?;
                    let Some(mut post) = opt_post else {
                        println!("Error: Scheduled post not found: {}", id);
                        return Ok(());
                    };

                    if let Some(t) = text {
                        post.content = t;
                    }
                    use chrono::TimeZone;
                    if let Some(at_str) = at {
                        let parsed_time = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&at_str) {
                            dt.with_timezone(&chrono::Local)
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M:%S") {
                            chrono::Local.from_local_datetime(&dt).unwrap()
                        } else if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&at_str, "%Y-%m-%d %H:%M") {
                            chrono::Local.from_local_datetime(&dt).unwrap()
                        } else {
                            println!("Error: Invalid datetime format. Use RFC3339 or 'YYYY-MM-DD HH:MM:SS'");
                            return Ok(());
                        };
                        post.scheduled_at = parsed_time;
                    }
                    if let Some(sns_arg) = sns {
                        let mut target_sns = Vec::new();
                        for part in sns_arg.split(',') {
                            let part = part.trim();
                            if !part.is_empty() {
                                target_sns.push(part.to_string());
                            }
                        }
                        post.target_sns = target_sns;
                    }
                    if let Some(s) = status {
                        post.status = s;
                    }
                    if let Some(l) = link {
                        post.link_url = Some(l);
                    }

                    post.updated_at = chrono::Local::now();

                    let updated = scheduled_store.update_post(&id, post).await?;
                    if let Some(p) = updated {
                        println!("Successfully updated scheduled post: {}", p.id);
                        println!("  Time: {}", p.scheduled_at.format("%Y-%m-%d %H:%M:%S"));
                        println!("  SNS: {:?}", p.target_sns);
                        println!("  Status: {}", p.status);
                    } else {
                        println!("Error: Failed to update scheduled post: {}", id);
                    }
                }
            }
        }
    }
    
    Ok(())
}

fn list_sns(config: &config::Config) {
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

fn list_feeds(config: &config::Config) {
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
