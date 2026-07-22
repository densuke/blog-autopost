//! 設定からSNSクライアントを構築し、`--sns` 指定で絞り込む。

use blog_autopost_rs::config;
use blog_autopost_rs::sns::{
    bluesky::BlueskyClient, mastodon::MastodonClient, misskey::MisskeyClient, traits::SnsClient,
    x::XClient,
};

use crate::commands::sns_selector::SnsSelector;

/// 設定から SNS クライアントのリストを構築する。
///
/// Run / Check の両コマンドで共通して使用する。生成に失敗したアカウントは
/// スキップされ、未対応の設定が見つかった場合は警告を表示する。
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

#[cfg(test)]
mod tests {
    use super::*;
    use blog_autopost_rs::config::{Config, SnsConfig};
    use std::collections::HashMap;

    fn config_with(sns: Vec<SnsConfig>) -> Config {
        Config {
            announcement_text: None,
            blog: None,
            sns,
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: None,
            extra: HashMap::new(),
        }
    }

    fn mastodon(name: &str) -> SnsConfig {
        SnsConfig::Mastodon {
            name: name.to_string(),
            instance_url: "https://mstdn.example.com".to_string(),
            access_token: "t".to_string(),
        }
    }

    fn misskey(name: &str) -> SnsConfig {
        SnsConfig::Misskey {
            name: name.to_string(),
            instance_url: "https://misskey.example.com".to_string(),
            access_token: "t".to_string(),
            is_sensitive: None,
        }
    }

    fn bluesky(name: &str) -> SnsConfig {
        SnsConfig::Bluesky {
            name: name.to_string(),
            identifier: "id".to_string(),
            password: "pw".to_string(),
        }
    }

    fn x(name: &str) -> SnsConfig {
        SnsConfig::X {
            name: name.to_string(),
            consumer_key: "ck".to_string(),
            consumer_secret: "cs".to_string(),
            access_token: "at".to_string(),
            access_token_secret: "ats".to_string(),
        }
    }

    #[test]
    fn test_build_from_empty_config() {
        assert!(build_sns_clients(&config_with(vec![])).is_empty());
    }

    /// 対応する4種のSNSがすべて構築される。
    #[test]
    fn test_build_all_supported_kinds() {
        let clients = build_sns_clients(&config_with(vec![
            mastodon("mstdn-main"),
            misskey("misskey-main"),
            bluesky("bsky-main"),
            x("x-main"),
        ]));

        let kinds: Vec<&str> = clients.iter().map(|c| c.name()).collect();
        assert_eq!(kinds, vec!["mastodon", "misskey", "bluesky", "x"]);

        let accounts: Vec<&str> = clients.iter().map(|c| c.account_name()).collect();
        assert_eq!(
            accounts,
            vec!["mstdn-main", "misskey-main", "bsky-main", "x-main"]
        );
    }

    /// 同じ種別の複数アカウントもそれぞれ構築される。
    #[test]
    fn test_build_multiple_accounts_of_same_kind() {
        let clients = build_sns_clients(&config_with(vec![
            mastodon("mstdn-main"),
            mastodon("mstdn-sub"),
        ]));

        assert_eq!(clients.len(), 2);
        assert_eq!(clients[1].account_name(), "mstdn-sub");
    }

    /// 未対応の設定はスキップされ、他のアカウントの構築は継続される。
    #[test]
    fn test_build_skips_unsupported_kind() {
        let clients = build_sns_clients(&config_with(vec![
            SnsConfig::Unknown,
            mastodon("mstdn-main"),
        ]));

        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].name(), "mastodon");
    }

    /// Threads と Tumblr は移植対象外のためスキップされる。
    #[test]
    fn test_build_skips_threads_and_tumblr() {
        let clients = build_sns_clients(&config_with(vec![
            SnsConfig::Threads {
                name: "threads-main".to_string(),
                user_id: "u".to_string(),
                access_token: "t".to_string(),
            },
            SnsConfig::Tumblr {
                name: "tumblr-main".to_string(),
                consumer_key: "ck".to_string(),
                consumer_secret: "cs".to_string(),
                oauth_token: "ot".to_string(),
                oauth_secret: "os".to_string(),
                blog_identifier: "b".to_string(),
            },
        ]));

        assert!(clients.is_empty());
    }

    #[test]
    fn test_filter_none_returns_all() {
        let clients = build_sns_clients(&config_with(vec![mastodon("a"), misskey("b")]));

        assert_eq!(filter_sns_clients(clients, None).len(), 2);
    }

    #[test]
    fn test_filter_by_kind() {
        let clients = build_sns_clients(&config_with(vec![mastodon("a"), misskey("b")]));

        let filtered = filter_sns_clients(clients, Some("misskey"));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name(), "misskey");
    }

    #[test]
    fn test_filter_by_account_name() {
        let clients = build_sns_clients(&config_with(vec![
            mastodon("mstdn-main"),
            mastodon("mstdn-sub"),
        ]));

        let filtered = filter_sns_clients(clients, Some("mstdn-sub"));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].account_name(), "mstdn-sub");
    }

    #[test]
    fn test_filter_exclude() {
        let clients = build_sns_clients(&config_with(vec![mastodon("a"), misskey("b"), x("c")]));

        let filtered = filter_sns_clients(clients, Some("-x"));

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|c| c.name() != "x"));
    }

    #[test]
    fn test_filter_all_keyword() {
        let clients = build_sns_clients(&config_with(vec![mastodon("a"), misskey("b")]));

        assert_eq!(filter_sns_clients(clients, Some("all")).len(), 2);
    }

    #[test]
    fn test_filter_matching_nothing_returns_empty() {
        let clients = build_sns_clients(&config_with(vec![mastodon("a")]));

        assert!(filter_sns_clients(clients, Some("bluesky")).is_empty());
    }
}
