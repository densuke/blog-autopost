//! MCP エンドポイントの認証方針。
//!
//! Web UI と同じ `secret_key` を共用していると、キーが漏れたときに
//! 画面操作も投稿もまとめて奪われる。MCP 専用のキーを持たせて
//! 影響範囲を切り分けられるようにする。
//!
//! ただし既定を分離モードにすると、更新した瞬間に既存の MCP 接続が
//! すべて 401 になってしまう。`mcp` 節を書いたかどうかで
//! 「意図して設定した」かを見分け、書いていなければ従来どおり動かす。

use crate::config::{Config, McpConfig};
use crate::web::session::constant_time_eq;

/// MCP エンドポイントをどう認証するかの方針。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpAuthPolicy {
    /// MCP を無効にしてある。すべて拒否する。
    Disabled,
    /// `mcp` 節が無いため `web_auth.secret_key` を受け付ける(従来互換)。
    LegacySecretKey(String),
    /// MCP 専用キーのみを受け付ける。
    DedicatedKey(String),
    /// 専用キーと `web_auth.secret_key` の両方を受け付ける(移行期間用)。
    BothKeys {
        /// MCP 専用キー。
        mcp_key: String,
        /// Web UI と共用の秘密キー。
        web_key: String,
    },
    /// 認証に使えるキーが無い。設定不備として拒否する。
    NoKeyConfigured,
}

impl McpAuthPolicy {
    /// 設定から方針を決める。
    pub fn from_config(config: &Config) -> Self {
        let web_key = config
            .web_auth
            .as_ref()
            .and_then(|a| a.secret_key.clone())
            .filter(|k| !k.is_empty());

        /// `secret_key` を受け付ける従来互換の方針を返す。
        fn legacy(web_key: Option<String>) -> McpAuthPolicy {
            match web_key {
                Some(key) => McpAuthPolicy::LegacySecretKey(key),
                None => McpAuthPolicy::NoKeyConfigured,
            }
        }

        let Some(mcp) = config.mcp.as_ref() else {
            // 節が無い = 従来からの利用者。secret_key で通し続ける
            return legacy(web_key);
        };

        if mcp.enabled == Some(false) {
            return McpAuthPolicy::Disabled;
        }

        // 認証に関する項目を1つも書いていないなら、認証方針を指定する意図が
        // なかったとみなす。allowed_media_dirs や allowed_origins だけを
        // 設定したいときに認証が壊れてしまうのを避けるため
        if mcp.api_key.is_none() && mcp.allow_web_secret_key.is_none() {
            return legacy(web_key);
        }

        let mcp_key = mcp.api_key.clone().filter(|k| !k.is_empty());
        let allow_web = mcp.allow_web_secret_key == Some(true);

        match (mcp_key, web_key) {
            (Some(mcp_key), Some(web_key)) if allow_web => {
                McpAuthPolicy::BothKeys { mcp_key, web_key }
            }
            (Some(mcp_key), _) => McpAuthPolicy::DedicatedKey(mcp_key),
            // 専用キーを設定する意図はあったが値が無い。allow_web を
            // 明示したなら従来互換に倒し、そうでなければ設定不備として扱う
            (None, web_key) if allow_web => legacy(web_key),
            _ => McpAuthPolicy::NoKeyConfigured,
        }
    }

    /// 提示されたキーを受け付けるかどうかを返す。
    ///
    /// 比較は定時間で行う。一致した先頭バイト数が処理時間の差として
    /// 漏れると、キーを1バイトずつ推測されうる。
    pub fn accepts(&self, presented: &str) -> bool {
        match self {
            McpAuthPolicy::Disabled | McpAuthPolicy::NoKeyConfigured => false,
            McpAuthPolicy::LegacySecretKey(key) | McpAuthPolicy::DedicatedKey(key) => {
                constant_time_eq(presented, key)
            }
            McpAuthPolicy::BothKeys { mcp_key, web_key } => {
                // 短絡させないため両方を評価してから論理和を取る
                let a = constant_time_eq(presented, mcp_key);
                let b = constant_time_eq(presented, web_key);
                a || b
            }
        }
    }

    /// 起動時にログへ出す説明を返す。
    pub fn describe(&self) -> &'static str {
        match self {
            McpAuthPolicy::Disabled => "MCP endpoints are disabled by configuration",
            McpAuthPolicy::LegacySecretKey(_) => {
                "MCP auth: web_auth.secret_key (legacy). Consider setting mcp.api_key \
                 so that a leaked key cannot also be used for the Web UI"
            }
            McpAuthPolicy::DedicatedKey(_) => "MCP auth: mcp.api_key (dedicated)",
            McpAuthPolicy::BothKeys { .. } => {
                "MCP auth: mcp.api_key and web_auth.secret_key (migration mode)"
            }
            McpAuthPolicy::NoKeyConfigured => {
                "MCP auth: no key configured. All MCP requests will be rejected"
            }
        }
    }

    /// 専用キーと共用キーが同じ値かどうかを返す。
    ///
    /// 同じ値だと節を分けた意味がないため、起動時に注意を促す。
    pub fn keys_are_identical(config: &Config) -> bool {
        let web = config
            .web_auth
            .as_ref()
            .and_then(|a| a.secret_key.as_deref());
        let mcp = config.mcp.as_ref().and_then(|m| m.api_key.as_deref());
        match (web, mcp) {
            (Some(w), Some(m)) => !w.is_empty() && w == m,
            _ => false,
        }
    }
}

/// MCP のパスかどうかを判定する。
///
/// Streamable HTTP のエンドポイント `/api/mcp` (末尾のスラッシュなし) も
/// 含める。これを漏らすと新しいエンドポイントが通常の認証経路へ落ち、
/// Cookie セッションで MCP の tool を叩けるようになってしまう。
pub fn is_mcp_path(path: &str) -> bool {
    path == "/api/mcp" || path.starts_with("/api/mcp/")
}

/// リクエストヘッダから提示されたキーを取り出す。
///
/// `Authorization: Bearer` と `X-Api-Key` の両方を見る。
/// Cookie セッションは見ない。ブラウザに乗ったセッションで
/// MCP の tool (= 即時投稿) を叩ける経路を残さないため。
pub fn presented_key(headers: &axum::http::HeaderMap) -> Option<&str> {
    if let Some(token) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(token);
    }

    headers.get("X-Api-Key").and_then(|v| v.to_str().ok())
}

/// MCP の許可メディアディレクトリを設定から取り出す。
pub fn allowed_media_dirs(mcp: Option<&McpConfig>) -> Vec<std::path::PathBuf> {
    let configured = mcp.and_then(|m| m.allowed_media_dirs.as_deref());
    crate::web::media::resolve_allowed_dirs(configured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WebAuthConfig;

    /// 検証用の設定を作る。
    fn config_with(secret_key: Option<&str>, mcp: Option<McpConfig>) -> Config {
        Config {
            announcement_text: None,
            blog: None,
            sns: vec![],
            templates: std::collections::HashMap::new(),
            default_allowed_timings: None,
            allowed_timings_tolerance_minutes: None,
            allowed_timings: None,
            web_auth: Some(WebAuthConfig {
                username: "admin".to_string(),
                password: "hashed".to_string(),
                secret_key: secret_key.map(|s| s.to_string()),
                session_ttl_hours: None,
                cookie_secure: None,
                login_max_attempts: None,
                login_window_seconds: None,
            }),
            mcp,
            extra: std::collections::HashMap::new(),
        }
    }

    // --- 従来互換モード ---

    #[test]
    fn mcp節が無ければsecret_keyで通す() {
        let config = config_with(Some("web-secret"), None);
        let policy = McpAuthPolicy::from_config(&config);

        assert_eq!(
            policy,
            McpAuthPolicy::LegacySecretKey("web-secret".to_string())
        );
        assert!(policy.accepts("web-secret"));
        assert!(!policy.accepts("other"));
    }

    #[test]
    fn secret_keyもmcp節も無ければ拒否する() {
        let config = config_with(None, None);

        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::NoKeyConfigured
        );
        assert!(!McpAuthPolicy::from_config(&config).accepts(""));
    }

    #[test]
    fn 空のsecret_keyは未設定として扱う() {
        let config = config_with(Some(""), None);

        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::NoKeyConfigured
        );
    }

    // --- 分離モード ---

    #[test]
    fn 専用キーがあればそれだけを通す() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                api_key: Some("mcp-secret".to_string()),
                ..Default::default()
            }),
        );
        let policy = McpAuthPolicy::from_config(&config);

        assert_eq!(
            policy,
            McpAuthPolicy::DedicatedKey("mcp-secret".to_string())
        );
        assert!(policy.accepts("mcp-secret"));
        // 分離の目的は、漏れたキーで別の機能を触らせないこと
        assert!(!policy.accepts("web-secret"));
    }

    #[test]
    fn allow_web_secret_keyで両方通す() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                api_key: Some("mcp-secret".to_string()),
                allow_web_secret_key: Some(true),
                ..Default::default()
            }),
        );
        let policy = McpAuthPolicy::from_config(&config);

        assert!(policy.accepts("mcp-secret"));
        assert!(policy.accepts("web-secret"));
        assert!(!policy.accepts("nope"));
    }

    #[test]
    fn 専用キーが空なら未設定として扱う() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                api_key: Some("".to_string()),
                ..Default::default()
            }),
        );

        // 節を書いたのに使えるキーが無い状態。従来互換には倒さない
        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::NoKeyConfigured
        );
    }

    #[test]
    fn 節はあるが認証設定が無ければ従来互換にする() {
        let config = config_with(Some("web-secret"), Some(McpConfig::default()));

        // 認証項目を書いていないなら方針を指定する意図が無かったとみなす
        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::LegacySecretKey("web-secret".to_string())
        );
    }

    #[test]
    fn 認証以外の項目だけなら従来互換にする() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                allowed_media_dirs: Some(vec!["/srv/media".to_string()]),
                allowed_origins: Some(vec!["https://ui.example.com".to_string()]),
                ..Default::default()
            }),
        );

        // メディアやオリジンの設定だけで認証が壊れては困る
        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::LegacySecretKey("web-secret".to_string())
        );
    }

    #[test]
    fn enabled_trueだけでも従来互換にする() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                enabled: Some(true),
                ..Default::default()
            }),
        );

        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::LegacySecretKey("web-secret".to_string())
        );
    }

    #[test]
    fn allow_web_secret_key_falseで専用キー無しは拒否する() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                // 明示的に false を書いた = secret_key を使わせない意図
                allow_web_secret_key: Some(false),
                ..Default::default()
            }),
        );

        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::NoKeyConfigured
        );
    }

    #[test]
    fn 専用キー無しでもallow_webを明示すれば通す() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                allow_web_secret_key: Some(true),
                ..Default::default()
            }),
        );
        let policy = McpAuthPolicy::from_config(&config);

        assert_eq!(
            policy,
            McpAuthPolicy::LegacySecretKey("web-secret".to_string())
        );
        assert!(policy.accepts("web-secret"));
    }

    // --- 無効化 ---

    #[test]
    fn enabled_falseならすべて拒否する() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                enabled: Some(false),
                api_key: Some("mcp-secret".to_string()),
                ..Default::default()
            }),
        );
        let policy = McpAuthPolicy::from_config(&config);

        assert_eq!(policy, McpAuthPolicy::Disabled);
        assert!(!policy.accepts("mcp-secret"));
        assert!(!policy.accepts("web-secret"));
    }

    #[test]
    fn enabled_trueは通常どおり扱う() {
        let config = config_with(
            Some("web-secret"),
            Some(McpConfig {
                enabled: Some(true),
                api_key: Some("mcp-secret".to_string()),
                ..Default::default()
            }),
        );

        assert_eq!(
            McpAuthPolicy::from_config(&config),
            McpAuthPolicy::DedicatedKey("mcp-secret".to_string())
        );
    }

    // --- キーの重複 ---

    #[test]
    fn 同じキーを設定していれば検出する() {
        let config = config_with(
            Some("same"),
            Some(McpConfig {
                api_key: Some("same".to_string()),
                ..Default::default()
            }),
        );

        assert!(McpAuthPolicy::keys_are_identical(&config));
    }

    #[test]
    fn 別のキーなら重複としない() {
        let config = config_with(
            Some("web"),
            Some(McpConfig {
                api_key: Some("mcp".to_string()),
                ..Default::default()
            }),
        );

        assert!(!McpAuthPolicy::keys_are_identical(&config));
        assert!(!McpAuthPolicy::keys_are_identical(&config_with(
            Some("web"),
            None
        )));
    }

    // --- describe ---

    #[test]
    fn 方針ごとに説明を返す() {
        assert!(McpAuthPolicy::Disabled.describe().contains("disabled"));
        assert!(
            McpAuthPolicy::LegacySecretKey("k".to_string())
                .describe()
                .contains("legacy")
        );
        assert!(
            McpAuthPolicy::DedicatedKey("k".to_string())
                .describe()
                .contains("dedicated")
        );
        assert!(
            McpAuthPolicy::BothKeys {
                mcp_key: "a".to_string(),
                web_key: "b".to_string(),
            }
            .describe()
            .contains("migration")
        );
        assert!(
            McpAuthPolicy::NoKeyConfigured
                .describe()
                .contains("no key configured")
        );
    }

    // --- パス判定 ---

    #[test]
    fn mcpのパスを見分ける() {
        assert!(is_mcp_path("/api/mcp/sse"));
        assert!(is_mcp_path("/api/mcp/message"));
        // Streamable HTTP のエンドポイント。ここを漏らすと Cookie 認証が通ってしまう
        assert!(is_mcp_path("/api/mcp"));

        assert!(!is_mcp_path("/api/config"));
        assert!(!is_mcp_path("/api/mcpx"));
        assert!(!is_mcp_path("/login"));
    }

    // --- キーの取り出し ---

    fn headers_with(name: &str, value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        headers
    }

    #[test]
    fn bearerトークンを取り出せる() {
        let headers = headers_with("Authorization", "Bearer my-token");

        assert_eq!(presented_key(&headers), Some("my-token"));
    }

    #[test]
    fn x_api_keyを取り出せる() {
        let headers = headers_with("X-Api-Key", "my-key");

        assert_eq!(presented_key(&headers), Some("my-key"));
    }

    #[test]
    fn bearerを優先する() {
        let mut headers = headers_with("Authorization", "Bearer from-bearer");
        headers.insert("X-Api-Key", "from-api-key".parse().unwrap());

        assert_eq!(presented_key(&headers), Some("from-bearer"));
    }

    #[test]
    fn 該当ヘッダが無ければnoneを返す() {
        assert_eq!(presented_key(&axum::http::HeaderMap::new()), None);

        // Basic 認証などは受け付けない
        let headers = headers_with("Authorization", "Basic dXNlcjpwYXNz");
        assert_eq!(presented_key(&headers), None);
    }

    #[test]
    fn cookieはキーとして扱わない() {
        // ブラウザのセッションで MCP の tool を叩ける経路を残さない
        let headers = headers_with("Cookie", "session_id=abc");

        assert_eq!(presented_key(&headers), None);
    }

    // --- 許可メディアディレクトリ ---

    #[test]
    fn 許可ディレクトリは未設定なら既定値を使う() {
        assert_eq!(
            allowed_media_dirs(None),
            crate::web::media::resolve_allowed_dirs(None)
        );
    }

    #[test]
    fn 許可ディレクトリは設定があればそれを使う() {
        let mcp = McpConfig {
            allowed_media_dirs: Some(vec!["/srv/media".to_string()]),
            ..Default::default()
        };

        assert_eq!(
            allowed_media_dirs(Some(&mcp)),
            vec![std::path::PathBuf::from("/srv/media")]
        );
    }
}
