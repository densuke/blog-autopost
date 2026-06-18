#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostContent {
    pub text: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostResult {
    pub success: bool,
    pub post_id: Option<String>,
    pub error_message: Option<String>,
}
