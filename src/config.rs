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
    /// MCP サーバ機能の設定。節そのものを省略できる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// MCP サーバ機能の設定。
///
/// `WebAuthConfig` とは別の節にしてある。ログイン時の bcrypt 移行が
/// 設定ファイルを丸ごと書き戻すため、同じ構造体に置くと Web ログインの
/// 副作用で MCP の設定が書き換わる経路ができてしまう。
///
/// 追加するフィールドには必ず `skip_serializing_if` を付けること。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct McpConfig {
    /// MCP エンドポイントを有効にするか。未指定時は有効。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// MCP 専用の API キー。
    ///
    /// これを設定すると `web_auth.secret_key` では MCP を認証できなくなり、
    /// キーが漏れたときの影響範囲を MCP だけに限定できる。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// `web_auth.secret_key` でも MCP を認証できるようにするか。
    ///
    /// 専用キーへ移行する期間だけ両方を通したい場合に使う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_web_secret_key: Option<bool>,
    /// MCP の tool がメディアとして参照できるディレクトリ。
    ///
    /// 未指定時は `data/uploads` と `data` のみを許可する。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_media_dirs: Option<Vec<String>>,
}

/// Web UI の認証設定。
///
/// 追加するフィールドには必ず `skip_serializing_if` を付けること。
/// ログイン時の bcrypt 移行で設定ファイルを丸ごと書き戻すため、
/// これがないと未設定の項目が `null` として既存の config.yml に現れてしまう。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct WebAuthConfig {
    pub username: String,
    pub password: String,
    pub secret_key: Option<String>,
    /// セッションの有効期間(時間)。未指定時は 24 時間。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ttl_hours: Option<u32>,
    /// セッション Cookie に `Secure` を付ける方針。`auto` / `always` / `never`。
    ///
    /// 未指定時は `auto` で、HTTPS 由来と判定できたときだけ付ける。
    /// 素の HTTP で運用している環境がログイン不能にならないようにするため。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie_secure: Option<String>,
    /// 窓内に許すログイン失敗の回数。未指定時は 5。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_max_attempts: Option<usize>,
    /// ログイン失敗を数える窓の長さ(秒)。未指定時は 300。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login_window_seconds: Option<u64>,
}

impl WebAuthConfig {
    /// 実際に使うセッションの有効期間(時間)を返す。
    ///
    /// 未設定と 0 は既定値として扱う。0 を許すと発行直後に切れてしまう。
    pub fn effective_session_ttl_hours(&self) -> u32 {
        match self.session_ttl_hours {
            Some(h) if h > 0 => h,
            _ => crate::web::session::DEFAULT_SESSION_TTL_HOURS,
        }
    }
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

    // --- mcp 節 ---

    #[test]
    fn mcp節が無い設定も読める() {
        // 既存の利用者の config.yml を壊さないことの確認
        let yaml = r#"
web_auth:
  username: "admin"
  password: "changeme"
  secret_key: "web-secret"
"#;
        let config = parse_config(yaml).expect("mcp節が無くても読めるはず");

        assert!(config.mcp.is_none());
    }

    #[test]
    fn mcp節を読み取れる() {
        let yaml = r#"
mcp:
  enabled: true
  api_key: "mcp-secret"
  allow_web_secret_key: false
  allowed_media_dirs:
    - "data/uploads"
    - "/srv/media"
"#;
        let config = parse_config(yaml).expect("mcp節を解釈できるはず");
        let mcp = config.mcp.expect("mcp節があるはず");

        assert_eq!(mcp.enabled, Some(true));
        assert_eq!(mcp.api_key.as_deref(), Some("mcp-secret"));
        assert_eq!(mcp.allow_web_secret_key, Some(false));
        assert_eq!(
            mcp.allowed_media_dirs,
            Some(vec!["data/uploads".to_string(), "/srv/media".to_string()])
        );
    }

    #[test]
    fn mcp節は一部だけの指定も読める() {
        let yaml = r#"
mcp:
  api_key: "mcp-secret"
"#;
        let mcp = parse_config(yaml).unwrap().mcp.expect("mcp節があるはず");

        assert_eq!(mcp.api_key.as_deref(), Some("mcp-secret"));
        assert_eq!(mcp.enabled, None);
        assert_eq!(mcp.allow_web_secret_key, None);
        assert_eq!(mcp.allowed_media_dirs, None);
    }

    #[test]
    fn 未設定の項目は書き戻しに現れない() {
        // ログイン時の bcrypt 移行で設定ファイルを丸ごと書き戻すため、
        // skip_serializing_if が無いと未設定の項目が null として現れる
        let yaml = r#"
web_auth:
  username: "admin"
  password: "changeme"
  secret_key: "web-secret"
"#;
        let config = parse_config(yaml).unwrap();
        let written = serde_yaml::to_string(&config).expect("シリアライズできるはず");

        assert!(!written.contains("mcp"), "実際の内容: {}", written);
        assert!(
            !written.contains("session_ttl_hours"),
            "実際の内容: {}",
            written
        );
        assert!(
            !written.contains("cookie_secure"),
            "実際の内容: {}",
            written
        );
        assert!(
            !written.contains("login_max_attempts"),
            "実際の内容: {}",
            written
        );
    }

    #[test]
    fn 配布テンプレートを読み込める() {
        // テンプレートの記述が実際の構造体と食い違っていないことを確認する。
        // カレントディレクトリ依存なので、読めない場合は検証を飛ばす
        let Ok(template) = std::fs::read_to_string("config.yml.template") else {
            return;
        };

        let config = parse_config(&template).expect("テンプレートを解釈できるはず");

        let auth = config.web_auth.expect("web_auth の例があるはず");
        assert_eq!(auth.username, "admin");

        // 未知のキーは extra に吸われてエラーにならないため、
        // 例として載せている SNS が種別として解釈できたことを確かめる
        assert!(!config.sns.is_empty());
        assert!(
            !config.sns.contains(&SnsConfig::Unknown),
            "テンプレートに解釈できない種別が含まれている"
        );
    }

    #[test]
    fn 設定した項目は書き戻しに残る() {
        let yaml = r#"
web_auth:
  username: "admin"
  password: "changeme"
  secret_key: "web-secret"
  session_ttl_hours: 8
mcp:
  api_key: "mcp-secret"
"#;
        let config = parse_config(yaml).unwrap();
        let written = serde_yaml::to_string(&config).expect("シリアライズできるはず");

        assert!(written.contains("mcp"), "実際の内容: {}", written);
        assert!(written.contains("mcp-secret"), "実際の内容: {}", written);
        assert!(
            written.contains("session_ttl_hours"),
            "実際の内容: {}",
            written
        );

        // 書き戻したものを読み直しても同じ設定になる
        let round_tripped = parse_config(&written).expect("読み直せるはず");
        assert_eq!(round_tripped.mcp, config.mcp);
        assert_eq!(round_tripped.web_auth, config.web_auth);
    }

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
