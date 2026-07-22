//! 設定からSNSクライアントを構築し、`--sns` 指定で絞り込む。

use blog_autopost_rs::config;
use blog_autopost_rs::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, traits::SnsClient,
    x::XClient,
};

use crate::commands::sns_selector::SnsSelector;

pub fn build_sns_clients(
    config_data: &config::Config,
) -> Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> {
    let mut sns_clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> = Vec::new();
    for sns_conf in &config_data.sns {
        match sns_conf {
            config::SnsConfig::Mastodon {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MastodonClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::Misskey {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) =
                    MisskeyClient::new(instance_url.clone(), access_token.clone(), name.clone())
                {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::Bluesky {
                identifier,
                password,
                name,
                ..
            } => {
                if let Ok(client) =
                    BlueskyClient::new(identifier.clone(), password.clone(), name.clone())
                {
                    sns_clients.push(std::sync::Arc::new(client));
                }
            }
            config::SnsConfig::X {
                consumer_key,
                consumer_secret,
                access_token,
                access_token_secret,
                name,
            } => {
                if let Ok(client) = XClient::new(
                    consumer_key.clone(),
                    consumer_secret.clone(),
                    access_token.clone(),
                    access_token_secret.clone(),
                    name.clone(),
                ) {
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

/// SNS クライアントのリストを `--sns` 指定で絞り込む。
///
/// `spec` はカンマ区切りで、SNS種別(例: 'mastodon')またはアカウント名
/// (例: 'mastodon-social')を指定する。先頭に '-' を付けると除外、'all' で
/// 全件対象。`None` または有効な指定が無い場合は全件をそのまま返す。
/// 判定の詳細は [`SnsSelector`] を参照。
pub fn filter_sns_clients(
    clients: Vec<std::sync::Arc<dyn SnsClient + Send + Sync>>,
    spec: Option<&str>,
) -> Vec<std::sync::Arc<dyn SnsClient + Send + Sync>> {
    let selector = SnsSelector::parse(spec);

    clients
        .into_iter()
        .filter(|c| selector.matches(c.name(), c.account_name()))
        .collect()
}
