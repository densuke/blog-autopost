//! Web UI のログインセッションを扱う。
//!
//! セッションIDの生成、有効期限の判定、Cookie の組み立て、
//! および API キー比較を1箇所にまとめてある。
//! いずれもハンドラやミドルウェアから独立して検証できるようにしてある。

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

/// セッションの既定の有効期間(時間)。
pub const DEFAULT_SESSION_TTL_HOURS: u32 = 24;

/// ログイン済みセッションの1件分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// ログインしたユーザー名。
    pub username: String,
    /// セッションを発行した時刻。
    pub created_at: DateTime<Utc>,
    /// この時刻を過ぎたセッションは無効になる。
    pub expires_at: DateTime<Utc>,
}

impl Session {
    /// 現在時刻を起点に、指定の有効期間を持つセッションを作る。
    pub fn new(username: String, ttl_hours: u32) -> Self {
        Self::with_created_at(username, ttl_hours, Utc::now())
    }

    /// 発行時刻を明示してセッションを作る。
    ///
    /// 時刻を引数に取ることで、期限切れの状態をテストから直接作れる。
    pub fn with_created_at(username: String, ttl_hours: u32, created_at: DateTime<Utc>) -> Self {
        Session {
            username,
            created_at,
            expires_at: created_at + Duration::hours(ttl_hours as i64),
        }
    }

    /// 指定時刻の時点で期限切れかどうかを返す。
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at <= now
    }
}

/// 期限切れのセッションを取り除く。
///
/// 取り除いた件数を返す。単一ユーザー運用でエントリ数は少ないため、
/// バックグラウンドで回すのではなくログイン時にまとめて掃除する。
pub fn purge_expired(sessions: &mut HashMap<String, Session>, now: DateTime<Utc>) -> usize {
    let before = sessions.len();
    sessions.retain(|_, s| !s.is_expired(now));
    before - sessions.len()
}

/// CSPRNG から 256 ビットのセッションIDを生成する。
///
/// 発行時刻を含めないのは、作成時刻の漏洩と推測の足がかりを避けるため。
/// 乱数源が使えない場合はセッションを発行できないため `None` を返す。
pub fn generate_session_id() -> Option<String> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).ok()?;
    Some(buf.iter().map(|b| format!("{:02x}", b)).collect())
}

/// Cookie に `Secure` 属性を付ける方針。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CookieSecure {
    /// HTTPS 由来と判定できたときだけ付ける(既定)。
    #[default]
    Auto,
    /// 常に付ける。TLS 終端が確実な環境向け。
    Always,
    /// 付けない。明示的な避難口。
    Never,
}

impl CookieSecure {
    /// 設定値から方針を決める。
    ///
    /// 未設定や解釈できない値は `Auto` として扱う。素の HTTP で運用している
    /// 環境がログイン不能になるのを避けるため、既定を厳しくしない。
    pub fn from_config(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("auto") => CookieSecure::Auto,
            Some("always") => CookieSecure::Always,
            Some("never") => CookieSecure::Never,
            Some(other) => {
                println!(
                    "Unknown cookie_secure value '{}'. Falling back to 'auto'.",
                    other
                );
                CookieSecure::Auto
            }
        }
    }

    /// この方針のもとで `Secure` を付けるべきかを返す。
    pub fn should_set(&self, is_https: bool) -> bool {
        match self {
            CookieSecure::Auto => is_https,
            CookieSecure::Always => true,
            CookieSecure::Never => false,
        }
    }
}

/// リクエストが HTTPS 由来かどうかを判定する。
///
/// リバースプロキシ配下では `X-Forwarded-Proto` が実際の scheme を示す。
/// 直接 TLS を終端している場合は URI の scheme を見る。
pub fn is_https_request(headers: &axum::http::HeaderMap, uri: &axum::http::Uri) -> bool {
    if let Some(proto) = headers
        .get("X-Forwarded-Proto")
        .and_then(|v| v.to_str().ok())
    {
        // 複数段のプロキシではカンマ区切りで積まれるため、最初の値を見る
        let first = proto.split(',').next().unwrap_or("").trim();
        return first.eq_ignore_ascii_case("https");
    }

    uri.scheme_str() == Some("https")
}

/// セッション Cookie の値を組み立てる。
pub fn build_session_cookie(session_id: &str, ttl_hours: u32, secure: bool) -> String {
    let max_age = ttl_hours as i64 * 3600;
    let mut cookie = format!(
        "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session_id, max_age
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

/// セッションを失効させる Cookie の値を返す。
pub fn build_expired_cookie() -> String {
    "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT"
        .to_string()
}

/// Cookie ヘッダから `session_id` の値を取り出す。
pub fn extract_session_id(cookie_header: &str) -> Option<&str> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix("session_id=") {
            return Some(value);
        }
    }
    None
}

/// 2つの文字列を、内容による処理時間の差が出ない方法で比較する。
///
/// 長さの違いまでは隠せないが、先頭何文字が一致したかは漏れない。
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Session ---

    #[test]
    fn session_は指定した時間だけ有効になる() {
        let created = Utc::now();
        let session = Session::with_created_at("admin".to_string(), 24, created);

        assert_eq!(session.expires_at, created + Duration::hours(24));
        assert!(!session.is_expired(created));
        assert!(!session.is_expired(created + Duration::hours(23)));
    }

    #[test]
    fn session_は期限を過ぎると無効になる() {
        let created = Utc::now();
        let session = Session::with_created_at("admin".to_string(), 1, created);

        // 境界そのものも無効として扱う
        assert!(session.is_expired(created + Duration::hours(1)));
        assert!(session.is_expired(created + Duration::hours(2)));
    }

    #[test]
    fn session_newは現在時刻を起点にする() {
        let before = Utc::now();
        let session = Session::new("admin".to_string(), 24);
        let after = Utc::now();

        assert!(session.created_at >= before && session.created_at <= after);
        assert!(!session.is_expired(Utc::now()));
    }

    // --- purge_expired ---

    #[test]
    fn purge_expiredは期限切れだけを取り除く() {
        let now = Utc::now();
        let mut sessions = HashMap::new();
        sessions.insert(
            "live".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );
        sessions.insert(
            "dead".to_string(),
            Session::with_created_at("admin".to_string(), 1, now - Duration::hours(2)),
        );

        let removed = purge_expired(&mut sessions, now);

        assert_eq!(removed, 1);
        assert!(sessions.contains_key("live"));
        assert!(!sessions.contains_key("dead"));
    }

    #[test]
    fn purge_expiredは対象が無ければ何もしない() {
        let now = Utc::now();
        let mut sessions = HashMap::new();
        sessions.insert(
            "live".to_string(),
            Session::with_created_at("admin".to_string(), 24, now),
        );

        assert_eq!(purge_expired(&mut sessions, now), 0);
        assert_eq!(sessions.len(), 1);
    }

    // --- generate_session_id ---

    #[test]
    fn セッションidは256ビットの16進表現になる() {
        let id = generate_session_id().expect("乱数を取得できるはず");

        assert_eq!(id.len(), 64, "32バイトを16進で表すと64文字");
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn セッションidは呼ぶたびに異なる() {
        let ids: std::collections::HashSet<String> =
            (0..64).filter_map(|_| generate_session_id()).collect();

        assert_eq!(ids.len(), 64, "64回生成して重複が出ないこと");
    }

    #[test]
    fn セッションidはタイムスタンプを含まない() {
        // 旧実装は sess_<timestamp>_<hash> という形式で作成時刻が読めた
        let id = generate_session_id().expect("乱数を取得できるはず");

        assert!(!id.contains('_'));
        assert!(!id.starts_with("sess"));
    }

    // --- CookieSecure ---

    #[test]
    fn cookie_secureの既定はautoになる() {
        assert_eq!(CookieSecure::from_config(None), CookieSecure::Auto);
        assert_eq!(CookieSecure::from_config(Some("")), CookieSecure::Auto);
        assert_eq!(CookieSecure::from_config(Some("auto")), CookieSecure::Auto);
        assert_eq!(CookieSecure::default(), CookieSecure::Auto);
    }

    #[test]
    fn cookie_secureは大文字小文字と空白を吸収する() {
        assert_eq!(
            CookieSecure::from_config(Some(" Always ")),
            CookieSecure::Always
        );
        assert_eq!(
            CookieSecure::from_config(Some("NEVER")),
            CookieSecure::Never
        );
    }

    #[test]
    fn cookie_secureは未知の値をautoにする() {
        // 設定ミスでログイン不能にならないよう、厳しい側へは倒さない
        assert_eq!(CookieSecure::from_config(Some("yes")), CookieSecure::Auto);
    }

    #[test]
    fn autoはhttpsのときだけsecureを付ける() {
        assert!(CookieSecure::Auto.should_set(true));
        assert!(!CookieSecure::Auto.should_set(false));
    }

    #[test]
    fn alwaysとneverはリクエストに依らない() {
        assert!(CookieSecure::Always.should_set(false));
        assert!(CookieSecure::Always.should_set(true));
        assert!(!CookieSecure::Never.should_set(true));
        assert!(!CookieSecure::Never.should_set(false));
    }

    // --- is_https_request ---

    fn headers_with(name: &str, value: &str) -> axum::http::HeaderMap {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(),
            value.parse().unwrap(),
        );
        headers
    }

    #[test]
    fn x_forwarded_protoがhttpsならhttps扱いになる() {
        let headers = headers_with("X-Forwarded-Proto", "https");
        let uri: axum::http::Uri = "/login".parse().unwrap();

        assert!(is_https_request(&headers, &uri));
    }

    #[test]
    fn x_forwarded_protoがhttpならhttps扱いにしない() {
        let headers = headers_with("X-Forwarded-Proto", "http");
        let uri: axum::http::Uri = "/login".parse().unwrap();

        assert!(!is_https_request(&headers, &uri));
    }

    #[test]
    fn x_forwarded_protoは多段プロキシでも先頭を見る() {
        let headers = headers_with("X-Forwarded-Proto", "https, http");
        let uri: axum::http::Uri = "/login".parse().unwrap();

        assert!(is_https_request(&headers, &uri));
    }

    #[test]
    fn ヘッダが無ければuriのschemeを見る() {
        let headers = axum::http::HeaderMap::new();

        let https: axum::http::Uri = "https://autopost.example.com/login".parse().unwrap();
        assert!(is_https_request(&headers, &https));

        let http: axum::http::Uri = "http://autopost.example.com/login".parse().unwrap();
        assert!(!is_https_request(&headers, &http));

        // scheme を持たない相対 URI は HTTP 扱い
        let relative: axum::http::Uri = "/login".parse().unwrap();
        assert!(!is_https_request(&headers, &relative));
    }

    // --- Cookie 組み立て ---

    #[test]
    fn cookieはhttponlyとmax_ageを含む() {
        let cookie = build_session_cookie("abc123", 24, false);

        assert!(cookie.starts_with("session_id=abc123;"));
        assert!(cookie.contains("Path=/"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(
            cookie.contains("Max-Age=86400"),
            "24時間 = 86400秒: {}",
            cookie
        );
        assert!(!cookie.contains("Secure"));
    }

    #[test]
    fn cookieはsecure指定で属性が付く() {
        let cookie = build_session_cookie("abc123", 1, true);

        assert!(cookie.contains("Max-Age=3600"));
        assert!(cookie.contains("; Secure"));
    }

    #[test]
    fn 失効cookieは即時に期限切れになる() {
        let cookie = build_expired_cookie();

        assert!(cookie.starts_with("session_id=;"));
        assert!(cookie.contains("Max-Age=0"));
        assert!(cookie.contains("Expires=Thu, 01 Jan 1970"));
    }

    // --- extract_session_id ---

    #[test]
    fn cookieヘッダからセッションidを取り出せる() {
        assert_eq!(extract_session_id("session_id=abc"), Some("abc"));
        assert_eq!(
            extract_session_id("theme=dark; session_id=abc; other=1"),
            Some("abc")
        );
    }

    #[test]
    fn セッションidが無ければnoneを返す() {
        assert_eq!(extract_session_id("theme=dark"), None);
        assert_eq!(extract_session_id(""), None);
    }

    #[test]
    fn 値に等号を含むセッションidも失わない() {
        // 旧実装は '=' で分割して要素数2を要求していたため、
        // 値に '=' を含むと取り出せなかった
        assert_eq!(extract_session_id("session_id=ab=cd"), Some("ab=cd"));
    }

    #[test]
    fn 空のセッションidは空文字として返る() {
        assert_eq!(extract_session_id("session_id="), Some(""));
    }

    // --- constant_time_eq ---

    #[test]
    fn 定時間比較は同じ文字列を一致とみなす() {
        assert!(constant_time_eq("secret-token", "secret-token"));
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn 定時間比較は異なる文字列を不一致とみなす() {
        assert!(!constant_time_eq("secret-token", "secret-tokeM"));
        assert!(!constant_time_eq("secret", "secret-token"));
        assert!(!constant_time_eq("", "x"));
    }

    #[test]
    fn 定時間比較はマルチバイト文字も扱える() {
        assert!(constant_time_eq("あいう", "あいう"));
        assert!(!constant_time_eq("あいう", "あいえ"));
    }
}
