//! Python 版 blog-autopost が残したデータ形式を読むための互換層（Agy #366）。
//!
//! # 背景
//!
//! 2026-07-20 に支店(GCP e2)で予約投稿が全滅した。原因は Rust 版が Python 版の
//! 残したデータ形式を受け付けなかったこと。支店の実データを調査した結果、次の
//! 2 種類の非互換が確認されている（全 JSON 横断の実測値）。
//!
//! 1. **日時形式**: RFC3339 が 69 件に対し、Python の `datetime.isoformat()` 由来の
//!    スペース区切り・タイムゾーン無し（例 `2026-06-04 05:34:56.536170`）が 31 件。
//!    後者は `DateTime<Local>` の既定の deserializer では読めず、
//!    `Failed to parse scheduled posts JSON` で一覧もスロット計算も全滅する。
//! 2. **ステータス語彙**: Python 版の `実行済み`(62) / `投稿完了`(27) / `スキップ`(5) に対し、
//!    Rust 版は `予約済み` / `投稿済み` / `失敗` を前提としている。`status` は `String`
//!    なのでパースは通るが、条件分岐から漏れて「永久に掃除されないレコード」が生まれる。
//!
//! 現在は支店のデータが空になっており実害は止まっているが、旧データ（`*.jsonold`,
//! `scheduled_posts.db`, `*.bak*`）を復元すると即座に再発する。本モジュールはその
//! 再発を防ぐ防御的な措置である。
//!
//! # 方針
//!
//! - 読み取り時にのみ解釈を広げる。**既存データを書き換える migration は行わない**。
//! - シリアライズは従来どおり RFC3339。書き戻された時点で正規化される。
//! - 解釈できない値は黙って既定値にせず、明示的にエラーにする。

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Deserializer};

/// Python 版が使っていた「投稿が完了した」を表す語彙。
///
/// `実行済み` と `投稿完了` はいずれも Rust 版の `投稿済み` に対応する。
const LEGACY_POSTED: [&str; 2] = ["実行済み", "投稿完了"];

/// Rust 版の正規のステータス。
pub const STATUS_PENDING: &str = "予約済み";
pub const STATUS_POSTED: &str = "投稿済み";
pub const STATUS_FAILED: &str = "失敗";

/// Python 版のみに存在する「スキップ」。
///
/// Rust 版には対応する概念が無い。投稿されなかったが処理は終わっている終端状態であり、
/// `投稿済み` へ潰すと「投稿した」という誤った記録になるため **正規化しない**。
/// 掃除対象に含めるかどうかは呼び出し側が明示的に判断する（`terminal_statuses` を参照）。
pub const STATUS_SKIPPED: &str = "スキップ";

/// 旧語彙を Rust 版の語彙へ寄せる。
///
/// `スキップ` は対応概念が無いため、そのまま返す（[`STATUS_SKIPPED`] の説明を参照）。
/// 未知の値も改変せずそのまま返す。
pub fn normalize_status(status: &str) -> &str {
    if LEGACY_POSTED.contains(&status) {
        STATUS_POSTED
    } else {
        status
    }
}

/// 「投稿待ち」と見なすか。旧語彙に該当するものは無いが、対称性のため関数で包む。
pub fn is_pending(status: &str) -> bool {
    normalize_status(status) == STATUS_PENDING
}

/// 「投稿が完了した」と見なすか（旧語彙 `実行済み` / `投稿完了` を含む）。
pub fn is_posted(status: &str) -> bool {
    normalize_status(status) == STATUS_POSTED
}

/// 掃除（古いレコードの削除）の対象とする終端ステータス一覧。
///
/// `投稿済み` に加えて、旧語彙 `実行済み` / `投稿完了`、および `スキップ` を含める。
/// スキップは「もう処理が進まない終端状態」であり、放置すると旧データが永久に
/// 残り続けるため掃除対象とする（ステータス自体は書き換えない）。
pub fn terminal_statuses() -> Vec<String> {
    let mut v = vec![STATUS_POSTED.to_string(), STATUS_SKIPPED.to_string()];
    v.extend(LEGACY_POSTED.iter().map(|s| s.to_string()));
    v
}

/// RFC3339 と Python isoformat の双方を受け付けて `DateTime<Local>` にする。
///
/// 受け付ける形式:
/// 1. RFC3339（例 `2026-06-13T14:48:44.335374+09:00`）— 現行の正常系
/// 2. `%Y-%m-%d %H:%M:%S%.f`（例 `2026-06-04 05:34:56.536170`）— Python isoformat
/// 3. `%Y-%m-%d %H:%M:%S`（例 `2026-06-22 21:00:00`）— 秒未満なし
///
/// 2 と 3 はタイムゾーンを持たないため、**ローカルタイムゾーン**として解釈する。
/// Python 版はローカル時刻で書いていたため、この解釈が元の意図と一致する。
pub fn parse_flexible_datetime(s: &str) -> Result<DateTime<Local>, String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Local));
    }

    for fmt in ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, fmt) {
            return match Local.from_local_datetime(&naive) {
                chrono::LocalResult::Single(dt) => Ok(dt),
                // 夏時間の巻き戻しで 2 通りに解釈できる場合は早い方を採る。
                chrono::LocalResult::Ambiguous(dt, _) => Ok(dt),
                // 夏時間の飛びで存在しない時刻。推測せずエラーにする。
                chrono::LocalResult::None => Err(format!(
                    "ローカルタイムゾーンに存在しない日時です（夏時間の切り替わり）: {s}"
                )),
            };
        }
    }

    Err(format!(
        "日時として解釈できません（RFC3339 / 'YYYY-MM-DD HH:MM:SS[.ffffff]' のいずれでもない）: {s}"
    ))
}

/// serde 用。[`parse_flexible_datetime`] を deserializer として使う。
pub fn deserialize_flexible_datetime<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    parse_flexible_datetime(&s).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 日時パース ----

    #[test]
    fn rfc3339_は従来どおり読める() {
        // 支店の scheduled_posts.json.bak2-20260720 に実在した値
        let dt = parse_flexible_datetime("2026-06-13T14:48:44.335374+09:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-06-13T14:48:44.335374+09:00");
    }

    #[test]
    fn python_isoformat_の秒未満ありを読める() {
        // 支店の scheduled_posts.jsonold に実在した値
        let dt = parse_flexible_datetime("2026-06-04 05:34:56.536170").unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
            "2026-06-04 05:34:56.536170"
        );
    }

    #[test]
    fn python_isoformat_の秒未満なしを読める() {
        // 支店の scheduled_posts.db に実在した形
        let dt = parse_flexible_datetime("2026-06-22 21:00:00").unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-06-22 21:00:00"
        );
    }

    #[test]
    fn 解釈できない値は明示的にエラーになる() {
        // 黙って既定値へ倒さないことを担保する
        for bad in ["", "not-a-date", "2026/06/04 05:34:56", "20260604053456"] {
            assert!(
                parse_flexible_datetime(bad).is_err(),
                "エラーになるべき: {bad}"
            );
        }
    }

    // ---- ステータス正規化 ----

    #[test]
    fn 旧語彙の投稿完了と実行済みは投稿済みと同義になる() {
        assert_eq!(normalize_status("投稿完了"), STATUS_POSTED);
        assert_eq!(normalize_status("実行済み"), STATUS_POSTED);
        assert!(is_posted("投稿完了"));
        assert!(is_posted("実行済み"));
    }

    #[test]
    fn 現行語彙はそのまま維持される() {
        assert_eq!(normalize_status(STATUS_PENDING), STATUS_PENDING);
        assert_eq!(normalize_status(STATUS_POSTED), STATUS_POSTED);
        assert_eq!(normalize_status(STATUS_FAILED), STATUS_FAILED);
        assert!(is_pending(STATUS_PENDING));
        assert!(is_posted(STATUS_POSTED));
    }

    #[test]
    fn スキップは投稿済みへ潰さない() {
        // 「投稿されなかった」という事実を失わせないための担保
        assert_eq!(normalize_status(STATUS_SKIPPED), STATUS_SKIPPED);
        assert!(!is_posted(STATUS_SKIPPED));
        assert!(!is_pending(STATUS_SKIPPED));
    }

    #[test]
    fn 未知の語彙は改変しない() {
        assert_eq!(normalize_status("なにかの新しい状態"), "なにかの新しい状態");
    }

    #[test]
    fn 掃除対象には旧語彙とスキップが含まれる() {
        let t = terminal_statuses();
        for s in [STATUS_POSTED, STATUS_SKIPPED, "実行済み", "投稿完了"] {
            assert!(t.contains(&s.to_string()), "掃除対象に含まれるべき: {s}");
        }
        // 未完了のものは掃除しない
        for s in [STATUS_PENDING, STATUS_FAILED] {
            assert!(
                !t.contains(&s.to_string()),
                "掃除対象に含めてはならない: {s}"
            );
        }
    }
}
