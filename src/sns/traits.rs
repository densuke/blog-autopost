use super::models::{PostContent, PostResult};
use async_trait::async_trait;

/// SNS（X, Bluesky, Mastodon等）への投稿を抽象化するインターフェース
#[async_trait]
pub trait SnsClient {
    /// SNSの名前（識別子）を返す
    fn name(&self) -> &str;

    /// アカウントの表示名（マルチアカウント対応用）を返す
    fn account_name(&self) -> &str;

    /// 投稿を実行する
    async fn post(&self, content: &PostContent) -> anyhow::Result<PostResult>;

    /// このSNSの最大文字数制限を返す
    fn max_characters(&self) -> usize;

    /// 文字数計算におけるURL1個分の重み（文字数）を返す。
    ///
    /// X(t.co)やMastodonはリンクを実際の長さに関わらず一律23文字として
    /// カウントするため、それらは23を返すように override する。
    /// 既定では実際のURL長(文字数)を返す。
    fn url_char_weight(&self, url: &str) -> usize {
        url.chars().count()
    }
}
