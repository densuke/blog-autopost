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
