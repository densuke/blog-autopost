//! `--list-sns` / `--list-feeds` の表示処理。

use blog_autopost_rs::config;

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
            config::SnsConfig::Mastodon {
                name,
                instance_url,
                access_token,
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: mastodon");
                println!("   インスタンス: {}", instance_url);
                let has_creds = !access_token.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
            }
            config::SnsConfig::Misskey {
                name,
                instance_url,
                access_token,
                ..
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: misskey");
                println!("   インスタンス: {}", instance_url);
                let has_creds = !access_token.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
            }
            config::SnsConfig::Bluesky {
                name,
                identifier,
                password,
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: bluesky");
                let has_creds = !identifier.is_empty() && !password.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
            }
            config::SnsConfig::X {
                name,
                consumer_key,
                consumer_secret,
                access_token,
                access_token_secret,
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: x");
                let has_creds = !consumer_key.is_empty()
                    && !consumer_secret.is_empty()
                    && !access_token.is_empty()
                    && !access_token_secret.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
            }
            config::SnsConfig::Threads {
                name,
                user_id,
                access_token,
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: threads");
                let has_creds = !user_id.is_empty() && !access_token.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
            }
            config::SnsConfig::Tumblr {
                name,
                consumer_key,
                consumer_secret,
                oauth_token,
                oauth_secret,
                blog_identifier,
            } => {
                println!("{}. {}", num, name);
                println!("   SNS種別: tumblr");
                println!("   ブログID: {}", blog_identifier);
                let has_creds = !consumer_key.is_empty()
                    && !consumer_secret.is_empty()
                    && !oauth_token.is_empty()
                    && !oauth_secret.is_empty();
                println!(
                    "   認証情報: {}",
                    if has_creds {
                        "設定済み"
                    } else {
                        "不完全"
                    }
                );
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
    use blog_autopost_rs::config::{BlogConfig, Config, SnsConfig};
    use std::collections::HashMap;

    /// 表示処理は標準出力へ書くだけで戻り値を持たないため、
    /// ここでは各分岐がパニックせずに完走することを検証する。
    fn config_with(sns: Vec<SnsConfig>, blog: Option<Vec<BlogConfig>>) -> Config {
        Config {
            announcement_text: None,
            blog,
            sns,
            templates: HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: None,
            extra: HashMap::new(),
        }
    }

    fn blog(name: &str, url: &str) -> BlogConfig {
        BlogConfig {
            name: name.to_string(),
            feed_url: url.to_string(),
            extra: HashMap::new(),
        }
    }

    // --- list_sns ---

    #[test]
    fn test_list_sns_with_no_accounts() {
        list_sns(&config_with(vec![], None));
    }

    /// 対応する全種別と、認証情報が空のケースを通す。
    #[test]
    fn test_list_sns_with_all_kinds() {
        let config = config_with(
            vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Misskey {
                    name: "misskey-main".to_string(),
                    instance_url: "https://misskey.example.com".to_string(),
                    access_token: "t".to_string(),
                    is_sensitive: Some(true),
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "id".to_string(),
                    password: "pw".to_string(),
                },
                SnsConfig::X {
                    name: "x-main".to_string(),
                    consumer_key: "ck".to_string(),
                    consumer_secret: "cs".to_string(),
                    access_token: "at".to_string(),
                    access_token_secret: "ats".to_string(),
                },
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
                SnsConfig::Unknown,
            ],
            None,
        );

        list_sns(&config);
    }

    /// 認証情報が空の場合は「不完全」として表示される分岐を通す。
    #[test]
    fn test_list_sns_with_empty_credentials() {
        let config = config_with(
            vec![
                SnsConfig::Mastodon {
                    name: "mstdn-empty".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: String::new(),
                },
                SnsConfig::Misskey {
                    name: "misskey-empty".to_string(),
                    instance_url: "https://misskey.example.com".to_string(),
                    access_token: String::new(),
                    is_sensitive: None,
                },
                SnsConfig::Bluesky {
                    name: "bsky-empty".to_string(),
                    identifier: String::new(),
                    password: String::new(),
                },
                SnsConfig::X {
                    name: "x-empty".to_string(),
                    consumer_key: String::new(),
                    consumer_secret: String::new(),
                    access_token: String::new(),
                    access_token_secret: String::new(),
                },
            ],
            None,
        );

        list_sns(&config);
    }

    // --- list_feeds ---

    /// blog セクション自体が無い場合。
    #[test]
    fn test_list_feeds_without_blog_section() {
        list_feeds(&config_with(vec![], None));
    }

    /// blog セクションはあるが空の場合。
    #[test]
    fn test_list_feeds_with_empty_blog_list() {
        list_feeds(&config_with(vec![], Some(vec![])));
    }

    #[test]
    fn test_list_feeds_with_multiple_feeds() {
        let config = config_with(
            vec![],
            Some(vec![
                blog("main", "https://example.com/index.xml"),
                blog("sub", "https://example.org/feed.atom"),
            ]),
        );

        list_feeds(&config);
    }
}
