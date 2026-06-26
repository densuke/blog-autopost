use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ScheduledPost {
    pub id: String,
    pub content: String,
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
    pub status: String, // "予約済み" (pending), "投稿済み" (posted), "失敗" (failed)
    pub error_message: Option<String>,
    pub created_at: DateTime<Local>,
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
