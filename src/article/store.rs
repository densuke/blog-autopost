use crate::article::models::Article;
use crate::article::traits::ArticleStore;
use async_trait::async_trait;
use serde_json;
use std::fs;
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

    fn read_articles(&self) -> anyhow::Result<Vec<Article>> {
        if !self.file_path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&self.file_path)?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let articles: Vec<Article> = serde_json::from_str(&content)?;
        Ok(articles)
    }
}

#[async_trait]
impl ArticleStore for JsonArticleStore {
    async fn get_new_articles(&self, latest_articles: Vec<Article>) -> anyhow::Result<Vec<Article>> {
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
        
        let content = serde_json::to_string_pretty(&saved_articles)?;
        fs::write(&self.file_path, content)?;
        
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
        store.save_articles(&[a1.clone()]).await.unwrap();

        // Check new articles
        let articles = vec![a1.clone(), a2.clone()];
        let new_articles = store.get_new_articles(articles).await.unwrap();
        
        assert_eq!(new_articles.len(), 1);
        assert_eq!(new_articles[0].link, "http://example.com/2");
    }

    #[tokio::test]
    async fn test_save_articles_updates_duplicates() {
        let temp_file = NamedTempFile::new().unwrap();
        let store = JsonArticleStore::new(temp_file.path());

        let a1 = create_dummy_article("Title 1", "http://example.com/1");
        store.save_articles(&[a1.clone()]).await.unwrap();

        // Save a modified version of a1 with the same link
        let mut a1_modified = a1.clone();
        a1_modified.title = "Title 1 Modified".to_string();
        
        store.save_articles(&[a1_modified]).await.unwrap();

        let saved = store.read_articles().unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Title 1 Modified");
    }
}
