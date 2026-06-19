use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostContent {
    pub text: String,
    pub image_url: Option<String>,
    pub media_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostResult {
    pub success: bool,
    pub post_id: Option<String>,
    pub error_message: Option<String>,
}
