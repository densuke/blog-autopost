use crate::scheduled::store::JsonScheduledPostStore;
use crate::sns::models::PostContent;
use crate::sns::traits::SnsClient;
use anyhow::Result;
use chrono::Local;
use std::sync::Arc;

pub struct ScheduledPostExecutor {
    store: Arc<JsonScheduledPostStore>,
    sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
    dry_run: bool,
}

impl ScheduledPostExecutor {
    pub fn new(
        store: Arc<JsonScheduledPostStore>,
        sns_clients: Vec<Arc<dyn SnsClient + Send + Sync>>,
        dry_run: bool,
    ) -> Self {
        Self {
            store,
            sns_clients,
            dry_run,
        }
    }

    pub async fn execute_pending_posts(&self) -> Result<()> {
        let now = Local::now();
        let posts = self.store.get_all_posts().await?;

        // 実行対象の投稿をフィルタリング
        // "予約済み" かつ scheduled_at <= now
        let pending_posts: Vec<_> = posts
            .into_iter()
            .filter(|p| p.status == "予約済み" && p.scheduled_at <= now)
            .collect();

        for mut post in pending_posts {
            println!(
                "Executing scheduled post: ID={}, ScheduledAt={}",
                post.id, post.scheduled_at
            );

            // target_sns の各 SNS クライアントへ送信
            let mut failed_sns = Vec::new();
            let mut success_sns = Vec::new();

            for target in &post.target_sns {
                let client = self.sns_clients.iter().find(|c| c.account_name() == target);
                match client {
                    Some(client) => {
                        let mut image_url = None;
                        let mut media_paths = Vec::new();
                        for file in &post.media_files {
                            if file.starts_with("http://") || file.starts_with("https://") {
                                if image_url.is_none() {
                                    image_url = Some(file.clone());
                                }
                            } else {
                                media_paths.push(file.clone());
                            }
                        }
                        let media_paths_opt = if media_paths.is_empty() {
                            None
                        } else {
                            Some(media_paths)
                        };

                        let content = PostContent {
                            text: post.content.clone(),
                            image_url,
                            media_paths: media_paths_opt,
                            link_url: post.link_url.clone(),
                            sensitive: post.sensitive,
                        };

                        if self.dry_run {
                            println!("[DRY RUN] Would post to {}: {:?}", target, content);
                            success_sns.push(target.clone());
                        } else {
                            match client.post(&content).await {
                                Ok(res) => {
                                    if res.success {
                                        success_sns.push(target.clone());
                                    } else {
                                        let err = res
                                            .error_message
                                            .unwrap_or_else(|| "Unknown error".to_string());
                                        println!("Failed to post to {}: {}", target, err);
                                        failed_sns.push((target.clone(), err));
                                    }
                                }
                                Err(e) => {
                                    println!("Error posting to {}: {:?}", target, e);
                                    failed_sns.push((target.clone(), format!("{:?}", e)));
                                }
                            }
                        }
                    }
                    None => {
                        let err = format!("SnsClient not found for target: {}", target);
                        println!("{}", err);
                        failed_sns.push((target.clone(), err));
                    }
                }
            }

            // ステータスの更新
            let now_updated = Local::now();
            post.updated_at = now_updated;
            if failed_sns.is_empty() {
                post.status = "投稿済み".to_string();
                post.error_message = None;
            } else {
                post.status = "失敗".to_string();
                let errors: Vec<String> = failed_sns
                    .into_iter()
                    .map(|(sns, err)| format!("{}: {}", sns, err))
                    .collect();
                post.error_message = Some(errors.join("; "));
            }

            let post_id = post.id.clone();
            self.store.update_post(&post_id, post).await?;
        }

        // クリーンアップ：24時間以上経過した完了投稿を削除
        let cleanup_cutoff = now - chrono::Duration::hours(24);
        match self
            .store
            .delete_posts_older_than(cleanup_cutoff, Some(vec!["投稿済み".to_string()]))
            .await
        {
            Ok(count) => {
                if count > 0 {
                    println!("Cleaned up {} old scheduled posts.", count);
                }
            }
            Err(e) => {
                println!("Error cleaning up old scheduled posts: {:?}", e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduled::models::ScheduledPost;
    use crate::scheduled::store::JsonScheduledPostStore;
    use crate::sns::models::PostResult;
    use async_trait::async_trait;
    use chrono::{Duration, Local};
    use tempfile::NamedTempFile;

    /// モックの応答パターン。
    #[derive(Clone, Copy)]
    enum MockBehavior {
        /// 投稿に成功する
        Success,
        /// 投稿は届いたが失敗した(success:false)
        Failure,
        /// 送信そのものが Err になる
        Error,
    }

    struct MockSnsClient {
        name: String,
        account_name: String,
        behavior: MockBehavior,
        /// 受け取った投稿内容を記録する
        received: std::sync::Mutex<Vec<PostContent>>,
    }

    impl MockSnsClient {
        fn new(account_name: &str, behavior: MockBehavior) -> Self {
            Self {
                name: "mock".to_string(),
                account_name: account_name.to_string(),
                behavior,
                received: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn success(account_name: &str) -> Self {
            Self::new(account_name, MockBehavior::Success)
        }
    }

    #[async_trait]
    impl SnsClient for MockSnsClient {
        fn name(&self) -> &str {
            &self.name
        }

        fn account_name(&self) -> &str {
            &self.account_name
        }

        async fn post(&self, content: &PostContent) -> Result<PostResult> {
            self.received.lock().unwrap().push(content.clone());
            match self.behavior {
                MockBehavior::Success => Ok(PostResult {
                    success: true,
                    post_id: Some("mock_post_id".to_string()),
                    error_message: None,
                }),
                MockBehavior::Failure => Ok(PostResult {
                    success: false,
                    post_id: None,
                    error_message: Some("投稿が拒否されました".to_string()),
                }),
                MockBehavior::Error => Err(anyhow::anyhow!("送信中にネットワークエラー")),
            }
        }

        fn max_characters(&self) -> usize {
            140
        }
    }

    /// 一時ファイル上のストアを作る。NamedTempFile は戻り値で保持すること。
    fn temp_store() -> (Arc<JsonScheduledPostStore>, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let store = Arc::new(JsonScheduledPostStore::new(temp_file.path()));
        (store, temp_file)
    }

    /// 実行対象となる過去日時の予約を1件作る。
    async fn seed_due_post(
        store: &JsonScheduledPostStore,
        content: &str,
        targets: Vec<String>,
    ) -> ScheduledPost {
        let post = ScheduledPost::new(
            content.to_string(),
            Local::now() - Duration::minutes(5),
            vec![],
            targets,
        );
        store.create_post(post).await.unwrap()
    }

    #[tokio::test]
    async fn test_executor_executes_pending_posts() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = Arc::new(JsonScheduledPostStore::new(temp_file.path()));

        // 過去の投稿（実行対象）
        let now = Local::now();
        let old_time = now - Duration::minutes(5);
        let post1 = ScheduledPost::new(
            "テスト投稿1".to_string(),
            old_time,
            vec![],
            vec!["mock-main".to_string()],
        );

        // 未来の投稿（対象外）
        let future_time = now + Duration::minutes(10);
        let post2 = ScheduledPost::new(
            "テスト投稿2".to_string(),
            future_time,
            vec![],
            vec!["mock-main".to_string()],
        );

        store.create_post(post1.clone()).await.unwrap();
        store.create_post(post2.clone()).await.unwrap();

        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> =
            vec![Arc::new(MockSnsClient::success("mock-main"))];

        let executor = ScheduledPostExecutor::new(store.clone(), clients, false);
        executor.execute_pending_posts().await.unwrap();

        // 1は投稿済みになっているはず
        let p1 = store.get_post_by_id(&post1.id).await.unwrap().unwrap();
        assert_eq!(p1.status, "投稿済み");

        // 2はまだ予約済みのままであるはず
        let p2 = store.get_post_by_id(&post2.id).await.unwrap().unwrap();
        assert_eq!(p2.status, "予約済み");
    }

    /// 投稿が失敗した場合はステータスが「失敗」になり、理由が記録される。
    #[tokio::test]
    async fn test_failed_post_is_marked_failed() {
        let (store, _tmp) = temp_store();
        let post = seed_due_post(&store, "失敗する投稿", vec!["mock-main".to_string()]).await;

        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![Arc::new(MockSnsClient::new(
            "mock-main",
            MockBehavior::Failure,
        ))];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        let updated = store.get_post_by_id(&post.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "失敗");
        let msg = updated.error_message.expect("失敗理由が記録されるはず");
        assert!(msg.contains("mock-main"), "実際の値: {}", msg);
        assert!(msg.contains("投稿が拒否されました"), "実際の値: {}", msg);
    }

    /// 送信自体がエラーになった場合も「失敗」として記録される。
    #[tokio::test]
    async fn test_errored_post_is_marked_failed() {
        let (store, _tmp) = temp_store();
        let post = seed_due_post(&store, "エラーになる投稿", vec!["mock-main".to_string()]).await;

        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![Arc::new(MockSnsClient::new(
            "mock-main",
            MockBehavior::Error,
        ))];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        let updated = store.get_post_by_id(&post.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "失敗");
        assert!(
            updated
                .error_message
                .unwrap()
                .contains("ネットワークエラー")
        );
    }

    /// 対象SNSのクライアントが見つからない場合も「失敗」になる。
    #[tokio::test]
    async fn test_missing_client_is_marked_failed() {
        let (store, _tmp) = temp_store();
        let post = seed_due_post(&store, "宛先不明", vec!["存在しないSNS".to_string()]).await;

        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> =
            vec![Arc::new(MockSnsClient::success("mock-main"))];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        let updated = store.get_post_by_id(&post.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "失敗");
        assert!(
            updated
                .error_message
                .unwrap()
                .contains("SnsClient not found")
        );
    }

    /// 一部のSNSだけ失敗した場合も全体としては「失敗」になる。
    #[tokio::test]
    async fn test_partial_failure_marks_whole_post_failed() {
        let (store, _tmp) = temp_store();
        let post = seed_due_post(
            &store,
            "片方だけ失敗",
            vec!["ok-account".to_string(), "ng-account".to_string()],
        )
        .await;

        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![
            Arc::new(MockSnsClient::success("ok-account")),
            Arc::new(MockSnsClient::new("ng-account", MockBehavior::Failure)),
        ];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        let updated = store.get_post_by_id(&post.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "失敗");
        let msg = updated.error_message.unwrap();
        assert!(msg.contains("ng-account"), "実際の値: {}", msg);
        assert!(
            !msg.contains("ok-account"),
            "成功した方は記録しない: {}",
            msg
        );
    }

    /// ドライランでは実際に送信せず、投稿済みとして扱う。
    #[tokio::test]
    async fn test_dry_run_does_not_post() {
        let (store, _tmp) = temp_store();
        let post = seed_due_post(&store, "ドライラン", vec!["mock-main".to_string()]).await;

        let client = Arc::new(MockSnsClient::new("mock-main", MockBehavior::Error));
        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![client.clone()];
        ScheduledPostExecutor::new(store.clone(), clients, true)
            .execute_pending_posts()
            .await
            .unwrap();

        // Error を返すモックだが、ドライランなので呼ばれず成功扱いになる
        assert!(
            client.received.lock().unwrap().is_empty(),
            "送信してはいけない"
        );
        let updated = store.get_post_by_id(&post.id).await.unwrap().unwrap();
        assert_eq!(updated.status, "投稿済み");
    }

    /// media_files のURLは image_url、ローカルパスは media_paths として渡される。
    #[tokio::test]
    async fn test_media_files_are_split_by_kind() {
        let (store, _tmp) = temp_store();
        let mut post = ScheduledPost::new(
            "メディア付き".to_string(),
            Local::now() - Duration::minutes(5),
            vec![
                "https://example.com/a.png".to_string(),
                "data/uploads/b.png".to_string(),
            ],
            vec!["mock-main".to_string()],
        );
        post.link_url = Some("https://example.com/article".to_string());
        store.create_post(post).await.unwrap();

        let client = Arc::new(MockSnsClient::success("mock-main"));
        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![client.clone()];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        let received = client.received.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(
            received[0].image_url.as_deref(),
            Some("https://example.com/a.png")
        );
        assert_eq!(
            received[0].media_paths.as_deref(),
            Some(["data/uploads/b.png".to_string()].as_slice())
        );
        assert_eq!(
            received[0].link_url.as_deref(),
            Some("https://example.com/article")
        );
    }

    /// 「投稿済み」の予約は再実行されない。
    #[tokio::test]
    async fn test_already_posted_is_skipped() {
        let (store, _tmp) = temp_store();
        let mut post = ScheduledPost::new(
            "既に投稿済み".to_string(),
            Local::now() - Duration::minutes(5),
            vec![],
            vec!["mock-main".to_string()],
        );
        post.status = "投稿済み".to_string();
        store.create_post(post).await.unwrap();

        let client = Arc::new(MockSnsClient::success("mock-main"));
        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![client.clone()];
        ScheduledPostExecutor::new(store.clone(), clients, false)
            .execute_pending_posts()
            .await
            .unwrap();

        assert!(client.received.lock().unwrap().is_empty());
    }
}
