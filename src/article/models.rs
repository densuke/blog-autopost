use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Article {
    pub title: String,
    pub link: String,
    #[serde(deserialize_with = "deserialize_lenient_datetime")]
    pub published_parsed: DateTime<Utc>,
    pub image_url: Option<String>,
    pub feed_name: String,
}

/// `published_parsed` を寛容に解釈するデシリアライザ。
///
/// Python 版から移行したデータでは日付が空文字列 `""` で保存されている場合があり、
/// そのままでは `DateTime<Utc>` への変換に失敗する。空文字列や RFC3339 として
/// 解釈できない値は Unix エポックにフォールバックさせ、読み込みエラーを防ぐ。
fn deserialize_lenient_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(epoch());
    }
    match DateTime::parse_from_rfc3339(trimmed) {
        Ok(dt) => Ok(dt.with_timezone(&Utc)),
        Err(_) => Ok(epoch()),
    }
}

/// 日付不明を表すフォールバック値 (Unix エポック)。
fn epoch() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).expect("Unix epoch is a valid timestamp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_empty_published_parsed_falls_back_to_epoch() {
        let json = r#"{
            "title": "t",
            "link": "l",
            "published_parsed": "",
            "image_url": null,
            "feed_name": "main"
        }"#;
        let article: Article = serde_json::from_str(json).unwrap();
        assert_eq!(article.published_parsed, epoch());
    }

    #[test]
    fn test_deserialize_valid_rfc3339() {
        let json = r#"{
            "title": "t",
            "link": "l",
            "published_parsed": "2024-01-01T12:00:00Z",
            "image_url": null,
            "feed_name": "main"
        }"#;
        let article: Article = serde_json::from_str(json).unwrap();
        assert_eq!(
            article.published_parsed,
            DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc)
        );
    }

    #[test]
    fn test_deserialize_invalid_string_falls_back_to_epoch() {
        let json = r#"{
            "title": "t",
            "link": "l",
            "published_parsed": "not-a-date",
            "image_url": null,
            "feed_name": "main"
        }"#;
        let article: Article = serde_json::from_str(json).unwrap();
        assert_eq!(article.published_parsed, epoch());
    }
}
