use std::path::PathBuf;
use std::fs;
use tokio::sync::Mutex;
use anyhow::{Result, Context};
use crate::scheduled::models::ScheduledPost;
use chrono::{DateTime, Local, Duration};

pub struct JsonScheduledPostStore {
    file_path: PathBuf,
    lock: Mutex<()>,
}

impl JsonScheduledPostStore {
    pub fn new<P: Into<PathBuf>>(file_path: P) -> Self {
        Self {
            file_path: file_path.into(),
            lock: Mutex::new(()),
        }
    }

    // 内部用ヘルパー：ファイルを読み込む（ロックなし）
    fn read_all_unlocked(&self) -> Result<Vec<ScheduledPost>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.file_path)
            .context("Failed to read scheduled posts file")?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let posts: Vec<ScheduledPost> = serde_json::from_str(&content)
            .context("Failed to parse scheduled posts JSON")?;
        Ok(posts)
    }

    // 内部用ヘルパー：ファイルに書き込む（ロックなし）
    fn write_all_unlocked(&self, posts: &[ScheduledPost]) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let content = serde_json::to_string_pretty(posts)
            .context("Failed to serialize scheduled posts to JSON")?;
        fs::write(&self.file_path, content)
            .context("Failed to write scheduled posts file")?;
        Ok(())
    }

    pub async fn get_all_posts(&self) -> Result<Vec<ScheduledPost>> {
        let _guard = self.lock.lock().await;
        self.read_all_unlocked()
    }

    #[allow(dead_code)]
    pub async fn get_post_by_id(&self, id: &str) -> Result<Option<ScheduledPost>> {
        let _guard = self.lock.lock().await;
        let posts = self.read_all_unlocked()?;
        Ok(posts.into_iter().find(|p| p.id == id))
    }

    pub async fn create_post(&self, post: ScheduledPost) -> Result<ScheduledPost> {
        let _guard = self.lock.lock().await;
        let mut posts = self.read_all_unlocked()?;
        posts.push(post.clone());
        self.write_all_unlocked(&posts)?;
        Ok(post)
    }

    pub async fn update_post(&self, id: &str, updated: ScheduledPost) -> Result<Option<ScheduledPost>> {
        let _guard = self.lock.lock().await;
        let mut posts = self.read_all_unlocked()?;
        if let Some(pos) = posts.iter().position(|p| p.id == id) {
            posts[pos] = updated.clone();
            self.write_all_unlocked(&posts)?;
            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    #[allow(dead_code)]
    pub async fn delete_post(&self, id: &str) -> Result<bool> {
        let _guard = self.lock.lock().await;
        let mut posts = self.read_all_unlocked()?;
        let orig_len = posts.len();
        posts.retain(|p| p.id != id);
        if posts.len() != orig_len {
            self.write_all_unlocked(&posts)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn get_posts_by_sns_and_time(
        &self,
        sns_name: &str,
        time: DateTime<Local>,
        tolerance_minutes: i64,
    ) -> Result<Vec<ScheduledPost>> {
        let _guard = self.lock.lock().await;
        let posts = self.read_all_unlocked()?;
        
        let start_time = time - Duration::minutes(tolerance_minutes);
        let end_time = time + Duration::minutes(tolerance_minutes);
        
        let filtered = posts.into_iter()
            .filter(|p| {
                p.target_sns.contains(&sns_name.to_string()) &&
                p.scheduled_at >= start_time &&
                p.scheduled_at <= end_time &&
                p.status != "失敗"
            })
            .collect();
        Ok(filtered)
    }

    pub async fn delete_posts_older_than(
        &self,
        cutoff: DateTime<Local>,
        statuses: Option<Vec<String>>,
    ) -> Result<usize> {
        let _guard = self.lock.lock().await;
        let mut posts = self.read_all_unlocked()?;
        let orig_len = posts.len();
        
        posts.retain(|p| {
            let status_match = match &statuses {
                Some(s) => s.contains(&p.status),
                None => true,
            };
            if !status_match {
                return true;
            }
            
            let ref_time = p.updated_at;
            if ref_time <= cutoff {
                false
            } else {
                true
            }
        });
        
        let deleted_count = orig_len - posts.len();
        if deleted_count > 0 {
            self.write_all_unlocked(&posts)?;
        }
        Ok(deleted_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_store_operations() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonScheduledPostStore::new(temp_file.path());

        // 1. 最初は空であること
        let posts = store.get_all_posts().await.unwrap();
        assert!(posts.is_empty());

        // 2. 作成
        let time = Local::now();
        let post = ScheduledPost::new(
            "テスト投稿".to_string(),
            time,
            vec![],
            vec!["x-main".to_string()],
        );
        let created = store.create_post(post.clone()).await.unwrap();
        assert_eq!(created.content, "テスト投稿");

        // 3. 取得
        let fetched = store.get_post_by_id(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.content, "テスト投稿");

        // 4. 更新
        let mut to_update = fetched.clone();
        to_update.content = "更新された投稿".to_string();
        let updated = store.update_post(&created.id, to_update).await.unwrap().unwrap();
        assert_eq!(updated.content, "更新された投稿");

        // 5. SNSと時間による取得
        let by_time = store.get_posts_by_sns_and_time("x-main", time, 5).await.unwrap();
        assert_eq!(by_time.len(), 1);
        assert_eq!(by_time[0].content, "更新された投稿");

        // 別SNSでは取得できないこと
        let by_time_other = store.get_posts_by_sns_and_time("bluesky-main", time, 5).await.unwrap();
        assert!(by_time_other.is_empty());

        // 時間外では取得できないこと
        let by_time_out = store.get_posts_by_sns_and_time("x-main", time + Duration::minutes(10), 5).await.unwrap();
        assert!(by_time_out.is_empty());

        // 6. 削除
        let deleted = store.delete_post(&created.id).await.unwrap();
        assert!(deleted);
        
        let posts_after = store.get_all_posts().await.unwrap();
        assert!(posts_after.is_empty());

        // 7. 古い投稿の一括削除テスト
        // テスト用の投稿をいくつか作成
        let now = Local::now();
        let old_time = now - Duration::days(5);
        
        let mut post_old = ScheduledPost::new("古い投稿".to_string(), old_time, vec![], vec!["x-main".to_string()]);
        post_old.status = "投稿済み".to_string();
        post_old.updated_at = old_time;
        
        let mut post_new = ScheduledPost::new("新しい投稿".to_string(), now, vec![], vec!["x-main".to_string()]);
        post_new.status = "投稿済み".to_string();
        post_new.updated_at = now;
        
        store.create_post(post_old).await.unwrap();
        store.create_post(post_new).await.unwrap();
        
        // 3日前を閾値にして削除
        let cutoff = now - Duration::days(3);
        let deleted_count = store.delete_posts_older_than(cutoff, Some(vec!["投稿済み".to_string()])).await.unwrap();
        assert_eq!(deleted_count, 1);
        
        let remaining = store.get_all_posts().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].content, "新しい投稿");
    }
}
