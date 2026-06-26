use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct PostContent {
    pub text: String,
    pub image_url: Option<String>,
    pub media_paths: Option<Vec<String>>,
    pub link_url: Option<String>,
    /// 添付メディアをセンシティブコンテンツとして扱うか（現状 Misskey のみ対応）
    #[serde(default)]
    pub sensitive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostResult {
    pub success: bool,
    pub post_id: Option<String>,
    pub error_message: Option<String>,
}
