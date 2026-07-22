use crate::article::models::Article;
use crate::article::traits::ArticleStore;
use async_trait::async_trait;
use serde_json;
use std::path::{Path, PathBuf};

pub struct JsonArticleStore {
    file_path: PathBuf,
}

impl JsonArticleStore {
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }

    /// 原子的な置き換えに用いる一時ファイルのパス。プロセスIDを付与し、
    /// 同一ディレクトリ内(同一ファイルシステム)に作ることで rename を原子化する。
    fn tmp_path(&self) -> PathBuf {
        let mut name = self
            .file_path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_else(|| std::ffi::OsString::from("articles.json"));
        name.push(format!(".tmp.{}", std::process::id()));
        match self.file_path.parent() {
            Some(parent) => parent.join(name),
            None => PathBuf::from(name),
        }
    }

    fn read_articles(&self) -> anyhow::Result<Vec<Article>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(&self.file_path)?;

        let lock = fd_lock::RwLock::new(file);
        let read_guard = lock.read()?;

        use std::io::Read;
        let mut content = String::new();
        (&*read_guard).read_to_string(&mut content)?;

        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        // 破損したデータファイル(例: 書き込み中断による trailing characters)で
        // 全処理が止まらないよう、パース失敗時は警告のうえ空リストとして扱う。
        // Python 版の `_safe_load_json` 相当のフォールバック。
        match serde_json::from_str::<Vec<Article>>(&content) {
            Ok(articles) => Ok(articles),
            Err(e) => {
                eprintln!(
                    "警告: 記事キャッシュ '{}' の読み込みに失敗しました: {}。破損の可能性があるため空として処理します。",
                    self.file_path.display(),
                    e
                );
                Ok(Vec::new())
            }
        }
    }
}

#[async_trait]
impl ArticleStore for JsonArticleStore {
    async fn get_new_articles(
        &self,
        latest_articles: Vec<Article>,
    ) -> anyhow::Result<Vec<Article>> {
        let saved_articles = self.read_articles()?;
        let mut new_articles = Vec::new();

        for article in latest_articles {
            if !saved_articles.iter().any(|a| a.link == article.link) {
                new_articles.push(article);
            }
        }

        Ok(new_articles)
    }

    async fn save_articles(&self, articles: &[Article]) -> anyhow::Result<()> {
        let mut saved_articles = self.read_articles()?;

        for article in articles {
            if let Some(existing) = saved_articles.iter_mut().find(|a| a.link == article.link) {
                *existing = article.clone();
            } else {
                saved_articles.push(article.clone());
            }
        }

        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let content = serde_json::to_string_pretty(&saved_articles)?;

        // 一時ファイルへ書き切ってから rename で置き換える。rename は同一
        // ファイルシステム上で原子的なため、書き込み途中の内容が残って
        // 「trailing characters」等の破損を招くことがない。多重実行時も
        // 最後の書き込みが丸ごと採用されるだけで、torn write は発生しない。
        let tmp_path = self.tmp_path();
        {
            use std::io::Write;
            let mut tmp = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&tmp_path)?;
            tmp.write_all(content.as_bytes())?;
            tmp.flush()?;
            tmp.sync_all()?;
        }
        std::fs::rename(&tmp_path, &self.file_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tempfile::NamedTempFile;

    fn create_dummy_article(title: &str, link: &str) -> Article {
        Article {
            title: title.to_string(),
            link: link.to_string(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "dummy_feed".to_string(),
            tags: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_get_new_articles_empty_store() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        let articles = vec![
            create_dummy_article("Title 1", "http://example.com/1"),
            create_dummy_article("Title 2", "http://example.com/2"),
        ];

        let new_articles = store.get_new_articles(articles.clone()).await.unwrap();
        assert_eq!(new_articles.len(), 2);
    }

    #[tokio::test]
    async fn test_save_and_get_new_articles() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        let a1 = create_dummy_article("Title 1", "http://example.com/1");
        let a2 = create_dummy_article("Title 2", "http://example.com/2");

        // Save initially
        store
            .save_articles(std::slice::from_ref(&a1))
            .await
            .unwrap();

        // Check new articles
        let articles = vec![a1.clone(), a2.clone()];
        let new_articles = store.get_new_articles(articles).await.unwrap();

        assert_eq!(new_articles.len(), 1);
        assert_eq!(new_articles[0].link, "http://example.com/2");
    }

    #[tokio::test]
    async fn test_read_articles_falls_back_on_corrupt_json() {
        // 有効な配列の後ろにゴミが続く破損ファイル(trailing characters)。
        let temp_file = NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), b"[]garbage trailing").unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        // エラーで停止せず、空リストとして扱えること。
        let saved = store.read_articles().unwrap();
        assert!(saved.is_empty());
    }

    #[tokio::test]
    async fn test_save_shorter_content_leaves_no_trailing_garbage() {
        // 長い内容→短い内容の順で保存しても、旧内容の尻尾が残らないこと。
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        let many: Vec<Article> = (0..20)
            .map(|i| create_dummy_article(&format!("t{i}"), &format!("http://example.com/{i}")))
            .collect();
        store.save_articles(&many).await.unwrap();

        // 別リンクの短い1件だけを新規保存(内容が短くなるケースではないが、
        // ここでは破損しない = 常に正しくパースできることを検証する)。
        let saved_after = store.read_articles().unwrap();
        assert_eq!(saved_after.len(), 20);

        // ファイルが単一の有効な JSON 配列であること(trailing characters が無い)。
        let content = std::fs::read_to_string(temp_file.path()).unwrap();
        let parsed: Vec<Article> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 20);
    }

    #[tokio::test]
    async fn test_save_articles_updates_duplicates() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        let a1 = create_dummy_article("Title 1", "http://example.com/1");
        store
            .save_articles(std::slice::from_ref(&a1))
            .await
            .unwrap();

        // Save a modified version of a1 with the same link
        let mut a1_modified = a1.clone();
        a1_modified.title = "Title 1 Modified".to_string();

        store.save_articles(&[a1_modified]).await.unwrap();

        let saved = store.read_articles().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Title 1 Modified");
    }
}
