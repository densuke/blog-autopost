//! 投稿前の文字数チェック。
//!
//! SNSごとに文字数の上限とURLの数え方が異なるため、送信前に超過を検出する。

use blog_autopost_rs::sns::traits::SnsClient;

/// 文字数の上限を超えたSNSの情報。
#[derive(Debug, PartialEq, Eq)]
pub struct LengthError {
    pub sns_name: String,
    pub account_name: String,
    pub max_length: usize,
    pub actual_length: usize,
}

impl std::fmt::Display for LengthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) - 制限: {}文字, 予定: {}文字",
            self.sns_name, self.account_name, self.max_length, self.actual_length
        )
    }
}

/// 投稿本文がSNSの文字数上限に収まるか調べ、実際の文字数を返す。
///
/// URLの数え方はSNSごとに異なる。X と Mastodon は t.co 相当の短縮を前提に
/// URLを一律23文字として数えるため、`url_char_weight` を通して計算する。
/// Bluesky はリンクをリンクカードとして添付し本文に含めないため、
/// リンクの分を加算しない。
///
/// # Arguments
///
/// * `client` - 対象のSNSクライアント
/// * `text` - 投稿本文
/// * `link` - 添付するリンクURL
///
/// # Returns
///
/// 本文として数えられる文字数。
pub fn calculate_length(client: &dyn SnsClient, text: &str, link: Option<&str>) -> usize {
    let mut length = text.chars().count();

    // Bluesky はリンクカードとして添付するため本文の文字数に含めない
    let is_link_card_sns = client.name() == "bluesky";

    if !is_link_card_sns && let Some(l) = link {
        // 本文とURLの間の半角スペース1文字分を加える
        length += 1 + client.url_char_weight(l);
    }

    length
}

/// 文字数の上限を超えている場合に `LengthError` を返す。
pub fn check_length(client: &dyn SnsClient, text: &str, link: Option<&str>) -> Option<LengthError> {
    let max_length = client.max_characters();
    let actual_length = calculate_length(client, text, link);

    if actual_length > max_length {
        Some(LengthError {
            sns_name: client.name().to_string(),
            account_name: client.account_name().to_string(),
            max_length,
            actual_length,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use blog_autopost_rs::sns::models::{PostContent, PostResult};

    /// 文字数計算だけを検証するためのスタブ。
    struct StubClient {
        name: String,
        account: String,
        max: usize,
        url_weight: Option<usize>,
    }

    impl StubClient {
        fn new(name: &str, max: usize, url_weight: Option<usize>) -> Self {
            Self {
                name: name.to_string(),
                account: format!("{}-account", name),
                max,
                url_weight,
            }
        }
    }

    #[async_trait]
    impl SnsClient for StubClient {
        fn name(&self) -> &str {
            &self.name
        }

        fn account_name(&self) -> &str {
            &self.account
        }

        async fn post(&self, _content: &PostContent) -> anyhow::Result<PostResult> {
            unreachable!("文字数チェックのテストでは投稿しない")
        }

        fn max_characters(&self) -> usize {
            self.max
        }

        fn url_char_weight(&self, url: &str) -> usize {
            self.url_weight.unwrap_or_else(|| url.chars().count())
        }
    }

    #[test]
    fn test_calculate_length_text_only() {
        let client = StubClient::new("misskey", 3000, None);

        assert_eq!(calculate_length(&client, "あいうえお", None), 5);
    }

    /// 日本語は文字数(バイト数ではない)で数える。
    #[test]
    fn test_calculate_length_counts_chars_not_bytes() {
        let client = StubClient::new("misskey", 3000, None);

        // 「あ」はUTF-8で3バイトだが1文字として数える
        assert_eq!(calculate_length(&client, "あ", None), 1);
    }

    /// URLの重みが固定のSNSでは、実際の長さに関わらずその値で数える。
    #[test]
    fn test_calculate_length_with_fixed_url_weight() {
        let client = StubClient::new("x", 280, Some(23));
        let long_url = "https://example.com/very/long/path/that/is/way/over/23/characters";

        // 本文5文字 + スペース1文字 + URL23文字
        assert_eq!(calculate_length(&client, "あいうえお", Some(long_url)), 29);
    }

    /// URLの重みが実長のSNSでは、URLの文字数がそのまま加算される。
    #[test]
    fn test_calculate_length_with_actual_url_length() {
        let client = StubClient::new("misskey", 3000, None);
        let url = "https://example.com"; // 19文字

        // 本文5文字 + スペース1文字 + URL19文字
        assert_eq!(calculate_length(&client, "あいうえお", Some(url)), 25);
    }

    /// Bluesky はリンクを本文に含めないため、リンク分を加算しない。
    #[test]
    fn test_calculate_length_bluesky_excludes_link() {
        let client = StubClient::new("bluesky", 300, None);

        assert_eq!(
            calculate_length(&client, "あいうえお", Some("https://example.com")),
            5
        );
    }

    #[test]
    fn test_check_length_within_limit() {
        let client = StubClient::new("misskey", 10, None);

        assert!(check_length(&client, "あいうえお", None).is_none());
    }

    /// 上限ちょうどは超過としない。
    #[test]
    fn test_check_length_exactly_at_limit() {
        let client = StubClient::new("misskey", 5, None);

        assert!(check_length(&client, "あいうえお", None).is_none());
    }

    #[test]
    fn test_check_length_over_limit() {
        let client = StubClient::new("x", 5, Some(23));

        let err = check_length(&client, "あいうえおか", None).expect("超過するはず");

        assert_eq!(err.sns_name, "x");
        assert_eq!(err.account_name, "x-account");
        assert_eq!(err.max_length, 5);
        assert_eq!(err.actual_length, 6);
    }

    /// リンクの加算により超過する場合も検出する。
    #[test]
    fn test_check_length_over_limit_due_to_link() {
        let client = StubClient::new("x", 25, Some(23));

        let err = check_length(&client, "あいうえお", Some("https://example.com"))
            .expect("リンク分を含めると超過するはず");

        assert_eq!(err.actual_length, 29);
    }

    #[test]
    fn test_length_error_display() {
        let err = LengthError {
            sns_name: "x".to_string(),
            account_name: "x-main".to_string(),
            max_length: 280,
            actual_length: 300,
        };

        assert_eq!(err.to_string(), "x (x-main) - 制限: 280文字, 予定: 300文字");
    }
}
