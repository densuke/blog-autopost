use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct Config {
    pub announcement_text: Option<String>,
    pub blog: Option<Vec<BlogConfig>>,
    #[serde(default)]
    pub sns: Vec<SnsConfig>,
    #[serde(default)]
    pub templates: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BlogConfig {
    pub name: String,
    pub feed_url: String,
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
    instance_url: "https://mstdn.jp"
    access_token: "dummy"
"#;
        let config = parse_config(yaml).expect("Failed to parse valid config");
        assert_eq!(config.announcement_text.as_deref(), Some("ブログを更新しました！"));
        assert_eq!(config.blog.unwrap()[0].feed_url, "https://example.com/blog/index.xml");
        
        match &config.sns[0] {
            SnsConfig::Mastodon { instance_url, access_token, .. } => {
                assert_eq!(instance_url, "https://mstdn.jp");
                assert_eq!(access_token, "dummy");
            }
            _ => panic!("Expected Mastodon config"),
        }
    }
}
