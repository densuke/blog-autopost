//! 稼働バージョンと、より新しい版が公開されているかの確認（Agy #408）。
//!
//! # 背景
//!
//! 2026-07-27 に支店(GCP e2)のバイナリが 0.1.0(07-10 版)のまま3世代遅れていたことが
//! 判明した。Release には v0.1.3(07-22) が出ていたが、稼働側から見る手段が
//! `--version` しか無く、誰も気づかないまま放置されていた。
//!
//! # 方針
//!
//! - チェックは **1日1回で十分**（会長指示 2026-07-27）。GitHub API は未認証だと
//!   60 req/h の制限があり、頻繁に叩く理由も無い。
//! - **本モジュールは検知と表示のみを担う**。実際の更新は Agy #409（深夜帯の自動差し替え）が行う。
//! - **API 取得の失敗で画面を壊さない**。取得できなければ「稼働バージョンのみ表示」へ退避する。
//! - 判定は文字列一致ではなく **数値としてのバージョン比較**。`v0.1.10` > `v0.1.9` を正しく扱う。

use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// このバイナリのバージョン（コンパイル時に埋め込まれる）。
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// GitHub Releases の最新タグを取得する先。
const LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/densuke/blog-autopost/releases/latest";

/// Web UI へ返す更新状況。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VersionStatus {
    /// 稼働中のバージョン（例 `0.1.4`）。取得に失敗しても必ず入る。
    pub current: String,
    /// 公開されている最新版（例 `0.1.5`）。未確認・取得失敗なら `None`。
    pub latest: Option<String>,
    /// より新しい版が公開されているか。未確認なら `false`。
    pub update_available: bool,
    /// 最後に確認できた時刻（RFC3339）。一度も成功していなければ `None`。
    pub checked_at: Option<String>,
}

impl VersionStatus {
    /// まだ一度も確認していない状態。稼働バージョンだけ分かる。
    pub fn unchecked() -> Self {
        Self {
            current: current_version().to_string(),
            latest: None,
            update_available: false,
            checked_at: None,
        }
    }
}

/// 確認結果の共有キャッシュ。
///
/// 1日1回の更新なので `RwLock` で十分（読みが圧倒的に多い）。
pub type SharedVersionStatus = Arc<RwLock<VersionStatus>>;

pub fn new_shared_status() -> SharedVersionStatus {
    Arc::new(RwLock::new(VersionStatus::unchecked()))
}

/// `v0.1.4` / `0.1.4` のような文字列を数値の並びへ分解する。
///
/// 比較のためだけの簡易パーサ。プレリリース識別子(`-rc1` 等)は無視して数値部分だけを見る。
fn parse_version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

/// `latest` が `current` より新しいか。
///
/// 文字列比較だと `0.1.10` < `0.1.9` と誤判定するため、要素ごとに数値で比較する。
pub fn is_newer(latest: &str, current: &str) -> bool {
    let l = parse_version_parts(latest);
    let c = parse_version_parts(current);
    let len = l.len().max(c.len());
    for i in 0..len {
        let lv = l.get(i).copied().unwrap_or(0);
        let cv = c.get(i).copied().unwrap_or(0);
        if lv != cv {
            return lv > cv;
        }
    }
    false
}

/// GitHub Releases API のレスポンスから必要な項目だけ取り出す。
#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// 最新版を問い合わせてキャッシュを更新する。
///
/// 失敗しても `Err` を返すだけでキャッシュは壊さない（前回の結果が残る）。呼び出し側は
/// ログに出す程度に留め、処理を継続すること。
pub async fn refresh(status: &SharedVersionStatus) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(format!("blog-autopost-rs/{}", current_version()))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let release: GithubRelease = client
        .get(LATEST_RELEASE_API)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let latest = release.tag_name.trim_start_matches('v').to_string();
    let current = current_version().to_string();
    let update_available = is_newer(&latest, &current);

    let mut guard = status.write().await;
    *guard = VersionStatus {
        current,
        latest: Some(latest),
        update_available,
        checked_at: Some(chrono::Local::now().to_rfc3339()),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 稼働バージョンはcargoから埋め込まれる() {
        // 空でなく、数字で始まること（Cargo.toml の version が入る）
        let v = current_version();
        assert!(!v.is_empty());
        assert!(
            v.chars().next().unwrap().is_ascii_digit(),
            "想定外の形式: {v}"
        );
    }

    #[test]
    fn 数値としてバージョンを比較する() {
        // 文字列比較だと "0.1.10" < "0.1.9" になってしまう。ここが本質。
        assert!(is_newer("0.1.10", "0.1.9"), "0.1.10 は 0.1.9 より新しい");
        assert!(!is_newer("0.1.9", "0.1.10"));

        assert!(is_newer("0.2.0", "0.1.4"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.4", "0.1.4"), "同一版は更新ではない");
        assert!(!is_newer("0.1.3", "0.1.4"), "古い版は更新ではない");
    }

    #[test]
    fn vプレフィックスの有無を吸収する() {
        assert!(is_newer("v0.1.5", "0.1.4"));
        assert!(is_newer("0.1.5", "v0.1.4"));
        assert!(!is_newer("v0.1.4", "v0.1.4"));
    }

    #[test]
    fn 桁数が違っても比較できる() {
        assert!(is_newer("0.2", "0.1.9"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(is_newer("1", "0.9.9"));
    }

    #[test]
    fn プレリリース識別子は数値部分だけ見る() {
        // タグ付きリリースが対象なので通常は現れないが、混入しても壊れないこと
        assert!(is_newer("0.1.5-rc1", "0.1.4"));
        assert!(!is_newer("0.1.4-rc1", "0.1.4"));
    }

    #[test]
    fn 壊れた文字列でもpanicしない() {
        // API が想定外の値を返しても画面を壊さないことの担保
        assert!(!is_newer("", "0.1.4"));
        assert!(!is_newer("not-a-version", "0.1.4"));
        assert!(is_newer("0.1.5", "こわれた"));
    }

    #[tokio::test]
    async fn 未確認状態でも稼働バージョンは分かる() {
        // API を一度も叩けていなくても画面が壊れないこと
        let s = new_shared_status();
        let v = s.read().await.clone();
        assert_eq!(v.current, current_version());
        assert_eq!(v.latest, None);
        assert!(!v.update_available);
        assert_eq!(v.checked_at, None);
    }

    #[tokio::test]
    async fn 未確認状態はjsonへ直列化できる() {
        let s = new_shared_status();
        let v = s.read().await.clone();
        let json = serde_json::to_string(&v).unwrap();
        assert!(json.contains("\"current\""));
        assert!(json.contains("\"update_available\":false"));
    }
}
