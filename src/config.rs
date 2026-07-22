use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 投稿タイミングの定義。
///
/// 曜日などのキーと、その日に許可される時刻のリストの組を並べたもの。
/// 例: `("mon", ["09:00", "18:00"])`
pub type AllowedTimings = Vec<(String, Vec<String>)>;

/// SNSごとの投稿タイミング定義。キーはSNSの設定名。
pub type AllowedTimingsBySns = HashMap<String, AllowedTimings>;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    pub announcement_text: Option<String>,
    pub blog: Option<Vec<BlogConfig>>,
    #[serde(default)]
    pub sns: Vec<SnsConfig>,
    #[serde(default)]
    pub templates: HashMap<String, String>,
    pub default_allowed_timings: Option<AllowedTimings>,
    pub allowed_timings_tolerance_minutes: Option<i64>,
    pub allowed_timings: Option<AllowedTimingsBySns>,
    pub web_auth: Option<WebAuthConfig>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct WebAuthConfig {
    pub username: String,
    pub password: String,
    pub secret_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BlogConfig {
    pub name: String,
    pub feed_url: String,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type")]
pub enum SnsConfig {
    #[serde(rename = "mastodon")]
    Mastodon {
        name: String,
        instance_url: String,
        access_token: String,
    },
    #[serde(rename = "misskey")]
    Misskey {
        name: String,
        instance_url: String,
        access_token: String,
        is_sensitive: Option<bool>,
    },
    #[serde(rename = "bluesky")]
    Bluesky {
        name: String,
        identifier: String,
        password: String,
    },
    #[serde(rename = "x")]
    X {
        name: String,
        consumer_key: String,
        consumer_secret: String,
        access_token: String,
        access_token_secret: String,
    },
    #[serde(rename = "threads")]
    Threads {
        name: String,
        user_id: String,
        access_token: String,
    },
    #[serde(rename = "tumblr")]
    Tumblr {
        name: String,
        consumer_key: String,
        consumer_secret: String,
        oauth_token: String,
        oauth_secret: String,
        blog_identifier: String,
    },
    #[serde(other)]
    Unknown,
}

pub fn parse_config(yaml_content: &str) -> Result<Config, serde_yaml::Error> {
    serde_yaml::from_str(yaml_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let yaml = r#"
announcement_text: "ブログを更新しました！"
blog:
  - name: "main"
    feed_url: "https://example.com/blog/index.xml"
sns:
  - type: mastodon
    name: "mstdn-main"
    instance_url: "https://mstdn.example.com"
    access_token: "dummy"
default_allowed_timings:
  - ["*", ["09:00", "12:00"]]
allowed_timings_tolerance_minutes: 5
allowed_timings:
  mstdn-main:
    - ["Weekday", ["08:00", "17:00"]]
"#;
        let config = parse_config(yaml).expect("Failed to parse valid config");
        assert_eq!(
            config.announcement_text.as_deref(),
            Some("ブログを更新しました！")
        );
        assert_eq!(
            config.blog.unwrap()[0].feed_url,
            "https://example.com/blog/index.xml"
        );

        match &config.sns[0] {
            SnsConfig::Mastodon {
                instance_url,
                access_token,
                ..
            } => {
                assert_eq!(instance_url, "https://mstdn.example.com");
                assert_eq!(access_token, "dummy");
            }
            _ => panic!("Expected Mastodon config"),
        }

        assert_eq!(
            config.default_allowed_timings,
            Some(vec![(
                "*".to_string(),
                vec!["09:00".to_string(), "12:00".to_string()]
            )])
        );
        assert_eq!(config.allowed_timings_tolerance_minutes, Some(5));

        let allowed_timings = config.allowed_timings.unwrap();
        assert!(allowed_timings.contains_key("mstdn-main"));
        assert_eq!(
            allowed_timings.get("mstdn-main").unwrap(),
            &vec![(
                "Weekday".to_string(),
                vec!["08:00".to_string(), "17:00".to_string()]
            )]
        );
    }
}
