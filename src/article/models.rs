use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Article {
    pub title: String,
    pub link: String,
    pub published_parsed: DateTime<Utc>,
    pub image_url: Option<String>,
    pub feed_name: String,
}
