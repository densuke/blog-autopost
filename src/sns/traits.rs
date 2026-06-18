use async_trait::async_trait;
use super::models::{PostContent, PostResult};

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
}
