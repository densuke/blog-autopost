use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub announcement_text: Option<String>,
    pub blog: BlogConfig,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BlogConfig {
    pub feed_url: String,
}

pub fn parse_config(yaml_content: &str) -> Result<Config, serde_yaml::Error> {
    // TDD: はじめは未実装で失敗させるか、KISSに則って一番シンプルな実装を書いてテストをパスさせる。
    // 今回はKISS優先で、一気に書いてしまう。
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
  feed_url: "https://example.com/feed"
"#;
        let config = parse_config(yaml).expect("Failed to parse valid config");
        assert_eq!(config.announcement_text.as_deref(), Some("ブログを更新しました！"));
        assert_eq!(config.blog.feed_url, "https://example.com/feed");
    }

    #[test]
    fn test_parse_config_without_announcement() {
        let yaml = r#"
blog:
  feed_url: "https://example.com/feed"
"#;
        let config = parse_config(yaml).expect("Failed to parse valid config");
        assert_eq!(config.announcement_text, None);
        assert_eq!(config.blog.feed_url, "https://example.com/feed");
    }
}
