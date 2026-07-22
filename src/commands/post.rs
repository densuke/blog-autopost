//! `post` サブコマンド。
//!
//! 任意のテキストを指定したSNSへ手動投稿する。

use blog_autopost_rs::config::{self, Config};
use blog_autopost_rs::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, models::PostContent,
    traits::SnsClient, x::XClient,
};

use crate::cli::Cli;
use crate::commands::length_check::check_length;
use crate::commands::sns_selector::SnsSelector;

/// 投稿先の指定をまとめたもの。
///
/// CLIの引数が多いため、サブコマンドの値をそのまま受け渡す。
pub struct PostArgs {
    pub text: String,
    pub sns: Option<String>,
    pub instance_url: Option<String>,
    pub token: Option<String>,
    pub media: Option<Vec<String>>,
    pub link: Option<String>,
}

/// 指定されたSNSへテキストを投稿する。
///
/// 送信前に各SNSの文字数上限を確認し、1つでも超過している場合は
/// どのSNSへも投稿せずに中止する。
pub async fn run(args: PostArgs, config_data: &Config, cli: &Cli) -> anyhow::Result<()> {
    let PostArgs {
        text,
        sns,
        instance_url,
        token,
        media,
        link,
    } = args;

    let mut sns_clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> = Vec::new();

    // フィルタ条件の構築(check と同じ書式を使う)
    let selector = SnsSelector::parse(sns.as_deref());

    // 1. config.ymlの設定をパースしてフィルタリング
    for sns_conf in &config_data.sns {
        match sns_conf {
            config::SnsConfig::Mastodon {
                instance_url: conf_url,
                access_token: conf_token,
                name,
                ..
            } => {
                if selector.matches("mastodon", name) {
                    let url = instance_url.clone().unwrap_or_else(|| conf_url.clone());
                    let tok = token.clone().unwrap_or_else(|| conf_token.clone());
                    if let Ok(client) = MastodonClient::new(url, tok, name.clone()) {
                        sns_clients.push(std::sync::Arc::new(client));
                    }
                }
            }
            config::SnsConfig::Misskey {
                instance_url: conf_url,
                access_token: conf_token,
                name,
                ..
            } => {
                if selector.matches("misskey", name) {
                    let url = instance_url.clone().unwrap_or_else(|| conf_url.clone());
                    let tok = token.clone().unwrap_or_else(|| conf_token.clone());
                    if let Ok(client) = MisskeyClient::new(url, tok, name.clone()) {
                        sns_clients.push(std::sync::Arc::new(client));
                    }
                }
            }
            config::SnsConfig::Bluesky {
                identifier: conf_id,
                password: conf_pw,
                name,
                ..
            } => {
                if selector.matches("bluesky", name) {
                    let id = instance_url.clone().unwrap_or_else(|| conf_id.clone());
                    let pw = token.clone().unwrap_or_else(|| conf_pw.clone());
                    if let Ok(client) = BlueskyClient::new(id, pw, name.clone()) {
                        sns_clients.push(std::sync::Arc::new(client));
                    }
                }
            }
            config::SnsConfig::X {
                consumer_key,
                consumer_secret,
                access_token,
                access_token_secret,
                name,
            } => {
                if selector.matches("x", name)
                    && let Ok(client) = XClient::new(
                        consumer_key.clone(),
                        consumer_secret.clone(),
                        access_token.clone(),
                        access_token_secret.clone(),
                        name.clone(),
                    )
                {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            _ => {}
        }
    }

    // 2. config.ymlにマッチするものがなかった場合、CLI引数からの直接指定でフォールバック
    if sns_clients.is_empty()
        && let Some(ref sns_val) = sns
    {
        let first_sns = sns_val.split(',').next().unwrap_or("").trim();
        if first_sns == "mastodon" {
            let url = instance_url
                .clone()
                .expect("instance_url must be provided via CLI or config.yml");
            let tok = token
                .clone()
                .expect("token must be provided via CLI or config.yml");
            let client = MastodonClient::new(url, tok, "CLI_User".to_string())?;
            sns_clients.push(std::sync::Arc::new(client));
        } else if first_sns == "misskey" {
            let url = instance_url
                .clone()
                .expect("instance_url must be provided via CLI or config.yml");
            let tok = token
                .clone()
                .expect("token must be provided via CLI or config.yml");
            let client = MisskeyClient::new(url, tok, "CLI_User".to_string())?;
            sns_clients.push(std::sync::Arc::new(client));
        } else if first_sns == "bluesky" {
            let id = instance_url
                .clone()
                .expect("identifier must be provided via CLI (instance_url) or config.yml");
            let pw = token
                .clone()
                .expect("password must be provided via CLI (token) or config.yml");
            let client = BlueskyClient::new(id, pw, "CLI_User".to_string())?;
            sns_clients.push(std::sync::Arc::new(client));
        } else if first_sns == "x" {
            println!(
                "X (Twitter) requires consumer credentials in config.yml. Cannot post without configuration."
            );
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
        match check_length(client.as_ref(), &text, link.as_deref()) {
            Some(err) => length_errors.push(err),
            None => client_post_contents.push((
                client.clone(),
                PostContent {
                    text: text.clone(),
                    image_url: None,
                    media_paths: media.clone(),
                    link_url: link.clone(),
                    sensitive: cli.sensitive,
                },
            )),
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
        println!(
            "Posting to {} ({})...",
            client.name(),
            client.account_name()
        );
        match client.post(&content).await {
            Ok(result) => {
                if result.success {
                    println!(
                        "Successfully posted to {}! ID: {:?}",
                        client.name(),
                        result.post_id
                    );
                } else {
                    println!(
                        "Failed to post to {}: {:?}",
                        client.name(),
                        result.error_message
                    );
                }
            }
            Err(e) => {
                println!("Error posting to {}: {:?}", client.name(), e);
            }
        }
    }

    Ok(())
}
