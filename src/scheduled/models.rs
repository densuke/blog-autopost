use super::compat::deserialize_flexible_datetime;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ScheduledPost {
    pub id: String,
    pub content: String,
    /// Python 版が残した `2026-06-04 05:34:56.536170` 形式も読める（Agy #366）。
    /// 書き出しは従来どおり RFC3339 なので、書き戻された時点で正規化される。
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub scheduled_at: DateTime<Local>,
    #[serde(default)]
    pub media_files: Vec<String>,
    #[serde(default)]
    pub target_sns: Vec<String>,
    #[serde(default)]
    pub link_url: Option<String>,
    /// 添付メディアをセンシティブコンテンツとして扱うか（現状 Misskey のみ対応）
    #[serde(default)]
    pub sensitive: bool,
    /// "予約済み" (pending), "投稿済み" (posted), "失敗" (failed)。
    /// Python 版が残した "実行済み" / "投稿完了" / "スキップ" も読み込める。
    /// 比較する際は `compat::normalize_status` 系を経由すること（Agy #366）。
    pub status: String,
    pub error_message: Option<String>,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub created_at: DateTime<Local>,
    #[serde(deserialize_with = "deserialize_flexible_datetime")]
    pub updated_at: DateTime<Local>,
}

impl ScheduledPost {
    pub fn new(
        content: String,
        scheduled_at: DateTime<Local>,
        media_files: Vec<String>,
        target_sns: Vec<String>,
    ) -> Self {
        let now = Local::now();
        let id = format!("post-{}", now.timestamp_nanos_opt().unwrap_or(0));
        Self {
            id,
            content,
            scheduled_at,
            media_files,
            target_sns,
            link_url: None,
            sensitive: false,
            status: "予約済み".to_string(),
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 支店(GCP e2)の `scheduled_posts.jsonold` に実在したレコードそのまま（Agy #366）。
    /// Python 版が書いた形式で、日時はスペース区切り・TZ 無し、status は旧語彙。
    const LEGACY_PYTHON_RECORD: &str = r#"{
        "id": "1a5c217e-2d04-4445-8c51-c1b787e25294",
        "content": "テスト投稿",
        "scheduled_at": "2026-06-04 05:34:56.536170",
        "media_files": [],
        "target_sns": ["x", "bluesky", "mastodon-social"],
        "link_url": null,
        "status": "投稿完了",
        "error_message": null,
        "created_at": "2026-06-04 05:34:56.542199",
        "updated_at": "2026-06-04 05:34:56.542205"
    }"#;

    /// 現行 Rust 版が書く形式（RFC3339）。回帰確認用。
    const CURRENT_RUST_RECORD: &str = r#"{
        "id": "post-1784705654601537754",
        "content": "テスト投稿",
        "scheduled_at": "2026-06-13T14:48:44.335374+09:00",
        "media_files": [],
        "target_sns": ["x"],
        "link_url": null,
        "sensitive": false,
        "status": "予約済み",
        "error_message": null,
        "created_at": "2026-06-13T14:48:44.335374+09:00",
        "updated_at": "2026-06-13T14:48:44.335374+09:00"
    }"#;

    #[test]
    fn python版が残したレコードを読める() {
        // #366 の全滅原因。sensitive フィールドが無いことも含めて実データそのまま。
        let post: ScheduledPost = serde_json::from_str(LEGACY_PYTHON_RECORD)
            .expect("Python 版のレコードが読めなければ #366 が再発する");
        assert_eq!(post.id, "1a5c217e-2d04-4445-8c51-c1b787e25294");
        assert_eq!(post.status, "投稿完了");
        assert_eq!(
            post.scheduled_at
                .format("%Y-%m-%d %H:%M:%S%.6f")
                .to_string(),
            "2026-06-04 05:34:56.536170"
        );
        // created_at / updated_at も同じ形式で入っている
        assert_eq!(
            post.updated_at.format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
            "2026-06-04 05:34:56.542205"
        );
        // 旧データに無いフィールドは既定値で埋まる
        assert!(!post.sensitive);
    }

    #[test]
    fn 現行形式は従来どおり読める() {
        let post: ScheduledPost =
            serde_json::from_str(CURRENT_RUST_RECORD).expect("現行形式が読めなくなっている");
        assert_eq!(post.status, "予約済み");
        // `to_rfc3339()` はローカルタイムゾーンで表記されるため、JST 環境と UTC 環境(CI)で
        // 文字列が変わる。時刻そのものを突き合わせる。
        let expected =
            chrono::DateTime::parse_from_rfc3339("2026-06-13T14:48:44.335374+09:00").unwrap();
        assert_eq!(post.scheduled_at, expected);
    }

    #[test]
    fn 混在した配列をまとめて読める() {
        // 移行途中のファイルは両形式が混ざりうる（実データでも RFC3339 69 件 / スペース区切り 31 件）
        let json = format!("[{LEGACY_PYTHON_RECORD}, {CURRENT_RUST_RECORD}]");
        let posts: Vec<ScheduledPost> =
            serde_json::from_str(&json).expect("混在ファイルが読めなければ移行途中で全滅する");
        assert_eq!(posts.len(), 2);
    }

    #[test]
    fn 書き出しは常にrfc3339へ正規化される() {
        // 読み込みは緩く、書き出しは厳しく。書き戻した時点で形式が揃う。
        let post: ScheduledPost = serde_json::from_str(LEGACY_PYTHON_RECORD).unwrap();
        let out = serde_json::to_string(&post).unwrap();
        assert!(
            out.contains("2026-06-04T05:34:56.536170"),
            "RFC3339 で書き出されていない: {out}"
        );
        assert!(
            !out.contains("2026-06-04 05:34:56.536170"),
            "旧形式のまま書き出された: {out}"
        );
    }

    #[test]
    fn 壊れた日時は黙って既定値にならずエラーになる() {
        let broken = LEGACY_PYTHON_RECORD.replace("2026-06-04 05:34:56.536170", "not-a-date");
        let result: Result<ScheduledPost, _> = serde_json::from_str(&broken);
        assert!(
            result.is_err(),
            "不正な日時が通ってしまうと誤った予約時刻で投稿されうる"
        );
    }
}
