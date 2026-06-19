use std::sync::Arc;
use anyhow::Result;
use chrono::Local;
use crate::scheduled::store::JsonScheduledPostStore;
use crate::sns::traits::SnsClient;
use crate::sns::models::PostContent;

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
            println!("Executing scheduled post: ID={}, ScheduledAt={}", post.id, post.scheduled_at);
            
            // target_sns の各 SNS クライアントへ送信
            let mut failed_sns = Vec::new();
            let mut success_sns = Vec::new();

            for target in &post.target_sns {
                let client = self.sns_clients.iter().find(|c| c.account_name() == target);
                match client {
                    Some(client) => {
                        let image_url = post.media_files.first().cloned();
                        let content = PostContent {
                            text: post.content.clone(),
                            image_url,
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
                                        let err = res.error_message.unwrap_or_else(|| "Unknown error".to_string());
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
                let errors: Vec<String> = failed_sns.into_iter().map(|(sns, err)| format!("{}: {}", sns, err)).collect();
                post.error_message = Some(errors.join("; "));
            }

            let post_id = post.id.clone();
            self.store.update_post(&post_id, post).await?;
        }

        // クリーンアップ：24時間以上経過した完了投稿を削除
        let cleanup_cutoff = now - chrono::Duration::hours(24);
        match self.store.delete_posts_older_than(cleanup_cutoff, Some(vec!["投稿済み".to_string()])).await {
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
    use crate::scheduled::store::JsonScheduledPostStore;
    use crate::scheduled::models::ScheduledPost;
    use crate::sns::models::PostResult;
    use async_trait::async_trait;
    use tempfile::NamedTempFile;
    use chrono::{Duration, Local};

    struct MockSnsClient {
        name: String,
        account_name: String,
    }

    #[async_trait]
    impl SnsClient for MockSnsClient {
        fn name(&self) -> &str {
            &self.name
        }

        fn account_name(&self) -> &str {
            &self.account_name
        }

        async fn post(&self, _content: &PostContent) -> Result<PostResult> {
            Ok(PostResult {
                success: true,
                post_id: Some("mock_post_id".to_string()),
                error_message: None,
            })
        }

        fn max_characters(&self) -> usize {
            140
        }
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

        let client = MockSnsClient {
            name: "mock".to_string(),
            account_name: "mock-main".to_string(),
        };
        let clients: Vec<Arc<dyn SnsClient + Send + Sync>> = vec![Arc::new(client)];

        let executor = ScheduledPostExecutor::new(store.clone(), clients, false);
        executor.execute_pending_posts().await.unwrap();

        // 1は投稿済みになっているはず
        let p1 = store.get_post_by_id(&post1.id).await.unwrap().unwrap();
        assert_eq!(p1.status, "投稿済み");

        // 2はまだ予約済みのままであるはず
        let p2 = store.get_post_by_id(&post2.id).await.unwrap().unwrap();
        assert_eq!(p2.status, "予約済み");
    }
}

