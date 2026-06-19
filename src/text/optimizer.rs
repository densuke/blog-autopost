use crate::article::models::Article;
use super::traits::TextOptimizer;
use async_trait::async_trait;

pub struct DefaultTextOptimizer;

impl DefaultTextOptimizer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TextOptimizer for DefaultTextOptimizer {
    async fn optimize(
        &self,
        article: &Article,
        template: &str,
        max_length: usize,
        announcement: Option<&str>,
    ) -> anyhow::Result<String> {
        // announcement と link は省略できないため、その長さを先に計算する
        let link_len = article.link.chars().count();
        let announcement_str = announcement.map(|s| format!("{}\n\n", s)).unwrap_or_default();
        let announcement_len = announcement_str.chars().count();

        // テンプレート内の固定文字（"{title}", "{link}" 以外の部分）の長さを計算
        let template_fixed_part = template.replace("{title}", "").replace("{link}", "");
        let template_fixed_len = template_fixed_part.chars().count();

        // タイトルに使える残りの文字数
        let reserved_len = link_len + announcement_len + template_fixed_len;
        let mut title = article.title.clone();

        if reserved_len + title.chars().count() > max_length {
            let available_title_len = max_length.saturating_sub(reserved_len + 3); // 3 for "..."
            
            if available_title_len > 0 {
                title = format!("{}...", title.chars().take(available_title_len).collect::<String>());
            } else {
                title = "".to_string(); // 極端なケース
            }
        }

        // 最終的なテキストを組み立てる
        let body = template
            .replace("{title}", &title)
            .replace("{link}", &article.link);

        Ok(format!("{}{}", announcement_str, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_optimize_short_text() {
        let optimizer = DefaultTextOptimizer::new();
        let article = Article {
            title: "短いタイトル".into(),
            link: "http://example.com/1".into(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test".into(),
        };

        let result = optimizer.optimize(&article, "{title} {link}", 100, Some("更新しました！")).await.unwrap();
        assert_eq!(result, "更新しました！\n\n短いタイトル http://example.com/1");
    }

    #[tokio::test]
    async fn test_optimize_long_text() {
        let optimizer = DefaultTextOptimizer::new();
        let article = Article {
            title: "あいうえおかきくけこさしすせそたちつてと".into(), // 20 chars
            link: "http://example.com/1".into(), // 20 chars
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test".into(),
        };

        // max=50 とする。
        // update_str = "更新\n\n" (4 chars)
        // link = 20 chars
        // space = 1 chars
        // reserved = 25 chars.
        // title allowed = 50 - 25 - 3("...") = 22.  titleは20なのでそのまま入る。
        let result = optimizer.optimize(&article, "{title} {link}", 50, Some("更新")).await.unwrap();
        assert_eq!(result, "更新\n\nあいうえおかきくけこさしすせそたちつてと http://example.com/1");

        // max=35 とする。
        // title allowed = 35 - 25 - 3 = 7
        let result2 = optimizer.optimize(&article, "{title} {link}", 35, Some("更新")).await.unwrap();
        assert_eq!(result2, "更新\n\nあいうえおかき... http://example.com/1");
    }
}
