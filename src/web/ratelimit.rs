//! ログイン試行のレート制限。
//!
//! `POST /login` は認証不要で叩けるため、制限がないとパスワードを
//! 無制限に試せてしまう。窓内の失敗回数を鍵ごとに数え、閾値を超えたら
//! 一定時間だけ受け付けを止める。
//!
//! 単一プロセスのインメモリ実装で足りる用途なので外部クレートは使わない。
//! 時刻に `tokio::time::Instant` を使っているため、テストからは
//! `tokio::time::pause` と `advance` で窓の開閉を進められる。

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

/// 窓内に許す失敗回数の既定値。
pub const DEFAULT_MAX_ATTEMPTS: usize = 5;

/// 失敗回数を数える窓の長さ(秒)の既定値。
pub const DEFAULT_WINDOW_SECONDS: u64 = 300;

/// ログイン試行のレート制限器。窓内の試行回数を鍵ごとに数える。
#[derive(Debug)]
pub struct LoginRateLimiter {
    attempts: tokio::sync::Mutex<HashMap<String, Vec<Instant>>>,
    max_attempts: usize,
    window: Duration,
}

impl LoginRateLimiter {
    /// 上限と窓の長さを指定して作る。
    ///
    /// 上限に 0 を渡すと誰もログインできなくなるため、既定値へ倒す。
    pub fn new(max_attempts: usize, window_seconds: u64) -> Self {
        let max_attempts = if max_attempts == 0 {
            DEFAULT_MAX_ATTEMPTS
        } else {
            max_attempts
        };
        let window_seconds = if window_seconds == 0 {
            DEFAULT_WINDOW_SECONDS
        } else {
            window_seconds
        };

        LoginRateLimiter {
            attempts: tokio::sync::Mutex::new(HashMap::new()),
            max_attempts,
            window: Duration::from_secs(window_seconds),
        }
    }

    /// 設定値から作る。未指定の項目は既定値を使う。
    pub fn from_config(max_attempts: Option<usize>, window_seconds: Option<u64>) -> Self {
        Self::new(
            max_attempts.unwrap_or(DEFAULT_MAX_ATTEMPTS),
            window_seconds.unwrap_or(DEFAULT_WINDOW_SECONDS),
        )
    }

    /// この鍵がまだ試行を受け付けられるかを返す。
    ///
    /// 受け付けられない場合は、あと何秒待てばよいかを `Err` で返す。
    /// `Retry-After` ヘッダにそのまま載せられる値である。
    pub async fn check(&self, key: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut attempts = self.attempts.lock().await;

        let Some(times) = attempts.get_mut(key) else {
            return Ok(());
        };

        // 窓から出た分を落としてから数える
        times.retain(|t| now.duration_since(*t) < self.window);
        if times.is_empty() {
            attempts.remove(key);
            return Ok(());
        }

        if times.len() < self.max_attempts {
            return Ok(());
        }

        // 最も古い試行が窓から出るまで待たせる
        let oldest = times.iter().min().copied().unwrap_or(now);
        let elapsed = now.duration_since(oldest);
        let remaining = self.window.saturating_sub(elapsed);
        // 0 を返すと即再試行できてしまうため、最低1秒は待たせる
        Err(remaining.as_secs().max(1))
    }

    /// 失敗を1件記録する。
    pub async fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let mut attempts = self.attempts.lock().await;
        let times = attempts.entry(key.to_string()).or_default();
        times.retain(|t| now.duration_since(*t) < self.window);
        times.push(now);
    }

    /// 成功したのでこの鍵の記録を消す。
    pub async fn record_success(&self, key: &str) {
        let mut attempts = self.attempts.lock().await;
        attempts.remove(key);
    }

    /// 記録されている鍵の数を返す。テストと診断用。
    pub async fn tracked_keys(&self) -> usize {
        self.attempts.lock().await.len()
    }
}

/// リクエストの接続元アドレス。取得できない場合は `None` を持つ。
///
/// `ConnectInfo` を直接受けると、アドレスが得られない状況で
/// ハンドラごとリジェクトされてしまう。テストの `oneshot` には
/// 接続情報がないため、取得できないことを許す形で包んでいる。
#[derive(Debug, Clone, Copy)]
pub struct PeerAddr(pub Option<std::net::SocketAddr>);

impl<S> axum::extract::FromRequestParts<S> for PeerAddr
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let addr = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0);
        Ok(PeerAddr(addr))
    }
}

/// レート制限に使う鍵を決める。
///
/// 接続元アドレスが取れればそれを使う。取れない場合はユーザー名へ倒す。
/// アドレスが取れないときに固定の鍵を使うと、誰か1人の失敗で全員が
/// ロックされてしまうため、必ず要求ごとに分かれる値を選ぶ。
///
/// `X-Forwarded-For` は使わない。偽装すれば制限を回避できてしまう。
pub fn rate_limit_key(peer: Option<std::net::SocketAddr>, username: &str) -> String {
    match peer {
        Some(addr) => format!("ip:{}", addr.ip()),
        None => format!("user:{}", username),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 既定値の制限器を作る。
    fn limiter() -> LoginRateLimiter {
        LoginRateLimiter::new(3, 60)
    }

    #[tokio::test]
    async fn 記録が無ければ通す() {
        let l = limiter();

        assert!(l.check("ip:203.0.113.1").await.is_ok());
        assert_eq!(l.tracked_keys().await, 0);
    }

    #[tokio::test]
    async fn 上限未満なら通す() {
        let l = limiter();
        l.record_failure("k").await;
        l.record_failure("k").await;

        assert!(l.check("k").await.is_ok(), "2回目までは通る");
    }

    #[tokio::test]
    async fn 上限に達したら止める() {
        let l = limiter();
        for _ in 0..3 {
            l.record_failure("k").await;
        }

        let retry_after = l.check("k").await.expect_err("止まるはず");
        assert!(retry_after > 0, "待ち秒数を返すこと");
        assert!(retry_after <= 60, "窓の長さを超えないこと: {}", retry_after);
    }

    #[tokio::test]
    async fn 鍵ごとに独立して数える() {
        let l = limiter();
        for _ in 0..3 {
            l.record_failure("ip:203.0.113.1").await;
        }

        assert!(l.check("ip:203.0.113.1").await.is_err());
        assert!(
            l.check("ip:203.0.113.2").await.is_ok(),
            "別の鍵は影響を受けない"
        );
    }

    #[tokio::test]
    async fn 成功すると記録が消える() {
        let l = limiter();
        for _ in 0..3 {
            l.record_failure("k").await;
        }
        assert!(l.check("k").await.is_err());

        l.record_success("k").await;

        assert!(l.check("k").await.is_ok());
        assert_eq!(l.tracked_keys().await, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn 窓を過ぎれば再び通す() {
        let l = limiter();
        for _ in 0..3 {
            l.record_failure("k").await;
        }
        assert!(l.check("k").await.is_err());

        // 窓 (60秒) を越えるまで進める
        tokio::time::advance(Duration::from_secs(61)).await;

        assert!(l.check("k").await.is_ok(), "窓が明けたら通る");
        assert_eq!(l.tracked_keys().await, 0, "空になった鍵は消える");
    }

    #[tokio::test(start_paused = true)]
    async fn 窓の途中では止まったまま() {
        let l = limiter();
        for _ in 0..3 {
            l.record_failure("k").await;
        }

        tokio::time::advance(Duration::from_secs(30)).await;

        let retry_after = l.check("k").await.expect_err("まだ止まっている");
        // 残り 30 秒前後
        assert!(
            (25..=35).contains(&retry_after),
            "残り秒数が妥当でない: {}",
            retry_after
        );
    }

    #[tokio::test(start_paused = true)]
    async fn 古い試行は数から外れる() {
        let l = limiter();
        l.record_failure("k").await;
        l.record_failure("k").await;

        // 最初の2件を窓の外へ出す
        tokio::time::advance(Duration::from_secs(61)).await;
        l.record_failure("k").await;

        assert!(l.check("k").await.is_ok(), "窓内は1件だけなので通るはず");
    }

    // --- 設定値の扱い ---

    #[tokio::test]
    async fn 上限0は既定値へ倒す() {
        let l = LoginRateLimiter::new(0, 60);

        // 既定は 5 回なので 4 回目までは通る
        for _ in 0..4 {
            l.record_failure("k").await;
        }
        assert!(l.check("k").await.is_ok());

        l.record_failure("k").await;
        assert!(l.check("k").await.is_err(), "5回で止まる");
    }

    #[tokio::test(start_paused = true)]
    async fn 窓0秒は既定値へ倒す() {
        let l = LoginRateLimiter::new(1, 0);
        l.record_failure("k").await;

        // 既定の 300 秒なので、299 秒では明けない
        tokio::time::advance(Duration::from_secs(299)).await;
        assert!(l.check("k").await.is_err());

        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(l.check("k").await.is_ok());
    }

    #[tokio::test]
    async fn from_configは未指定で既定値を使う() {
        let l = LoginRateLimiter::from_config(None, None);

        for _ in 0..4 {
            l.record_failure("k").await;
        }
        assert!(l.check("k").await.is_ok(), "既定は5回");

        l.record_failure("k").await;
        assert!(l.check("k").await.is_err());
    }

    #[tokio::test]
    async fn from_configは指定値を使う() {
        let l = LoginRateLimiter::from_config(Some(1), Some(60));
        l.record_failure("k").await;

        assert!(l.check("k").await.is_err(), "1回で止まる");
    }

    // --- 鍵の決め方 ---

    #[test]
    fn 接続元があればipを鍵にする() {
        let addr: std::net::SocketAddr = "203.0.113.5:54321".parse().unwrap();

        // ポートは鍵に含めない。同じ相手からの再接続を別扱いにしないため
        assert_eq!(rate_limit_key(Some(addr), "admin"), "ip:203.0.113.5");
    }

    #[test]
    fn 接続元が無ければユーザー名を鍵にする() {
        // 固定の鍵にすると1人の失敗で全員が止まってしまう
        assert_eq!(rate_limit_key(None, "admin"), "user:admin");
        assert_ne!(rate_limit_key(None, "admin"), rate_limit_key(None, "other"));
    }

    #[test]
    fn ipv6でも鍵を作れる() {
        let addr: std::net::SocketAddr = "[2001:db8::1]:443".parse().unwrap();

        assert_eq!(rate_limit_key(Some(addr), "admin"), "ip:2001:db8::1");
    }
}
