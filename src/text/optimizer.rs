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
        link_weight: usize,
        tags: &[String],
    ) -> anyhow::Result<String> {
        // announcement と link は省略できないため、その長さを先に計算する。
        // リンクはSNSにより文字数の数え方が異なる(X/Mastodonは一律23文字)ため、
        // 呼び出し側から渡された link_weight を使用する。
        let link_len = link_weight;
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

        let mut result = format!("{}{}", announcement_str, body);

        // タグは最も優先度が低い。本文(タイトル/リンク/アナウンス)を確定させた後、
        // 文字数上限に収まる範囲で末尾に付与する。入らないタグは捨てる。
        // 文字数計算はリンクを link_weight で換算する(本文には実URLが入るため補正)。
        if !tags.is_empty() {
            let link_actual = if template.contains("{link}") {
                article.link.chars().count()
            } else {
                0
            };
            let mut effective_len = result.chars().count() - link_actual + link_weight;
            let mut first = true;
            for tag in tags {
                // 1個目はタイトル/リンクと分けるため改行、以降は空白区切り。
                let separator = if first { "\n" } else { " " };
                let piece = format!("{}#{}", separator, tag);
                let add = piece.chars().count();
                if effective_len + add <= max_length {
                    result.push_str(&piece);
                    effective_len += add;
                    first = false;
                }
            }
        }

        Ok(result)
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
            tags: Vec::new(),
        };

        let link_weight = article.link.chars().count();
        let result = optimizer.optimize(&article, "{title} {link}", 100, Some("更新しました！"), link_weight, &[]).await.unwrap();
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
            tags: Vec::new(),
        };

        // max=50 とする。
        // update_str = "更新\n\n" (4 chars)
        // link = 20 chars
        // space = 1 chars
        // reserved = 25 chars.
        // title allowed = 50 - 25 - 3("...") = 22.  titleは20なのでそのまま入る。
        let result = optimizer.optimize(&article, "{title} {link}", 50, Some("更新"), 20, &[]).await.unwrap();
        assert_eq!(result, "更新\n\nあいうえおかきくけこさしすせそたちつてと http://example.com/1");

        // max=35 とする。
        // title allowed = 35 - 25 - 3 = 7
        let result2 = optimizer.optimize(&article, "{title} {link}", 35, Some("更新"), 20, &[]).await.unwrap();
        assert_eq!(result2, "更新\n\nあいうえおかき... http://example.com/1");
    }

    #[tokio::test]
    async fn test_optimize_link_weight_avoids_overtruncation() {
        // 実URLは長いが、X/Mastodon相当(重み23)で数える場合は
        // タイトルが余計に削られないことを確認する。
        let optimizer = DefaultTextOptimizer::new();
        let long_link = "https://example.com/2026/06/29/very-long-slug-article-id-123456";
        let article = Article {
            title: "あいうえおかきくけこ".into(), // 10 chars
            link: long_link.into(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test".into(),
            tags: Vec::new(),
        };

        // 重み23(t.co相当)、max=40。reserved = 23(link) + 1(space) = 24, title(10)は収まる
        let weighted = optimizer
            .optimize(&article, "{title} {link}", 40, None, 23, &[])
            .await
            .unwrap();
        assert_eq!(weighted, format!("あいうえおかきくけこ {}", long_link));

        // 実URL長で数えると上限を大きく超え、タイトルが空になる
        let actual_len = long_link.chars().count();
        let unweighted = optimizer
            .optimize(&article, "{title} {link}", 40, None, actual_len, &[])
            .await
            .unwrap();
        assert_eq!(unweighted, format!(" {}", long_link));
    }

    #[tokio::test]
    async fn test_optimize_appends_tags_within_limit() {
        let optimizer = DefaultTextOptimizer::new();
        let article = Article {
            title: "タイトル".into(),
            link: "http://example.com/1".into(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test".into(),
            tags: Vec::new(),
        };

        // 上限に余裕があれば末尾にタグを改行+空白区切りで付与する
        let tags = vec!["Rust".to_string(), "tech".to_string()];
        let result = optimizer
            .optimize(&article, "{title} {link}", 100, None, 20, &tags)
            .await
            .unwrap();
        assert_eq!(result, "タイトル http://example.com/1\n#Rust #tech");
    }

    #[tokio::test]
    async fn test_optimize_drops_tags_when_over_limit() {
        let optimizer = DefaultTextOptimizer::new();
        let article = Article {
            title: "タイトル".into(), // 4 chars
            link: "http://example.com/1".into(),
            published_parsed: Utc::now(),
            image_url: None,
            feed_name: "test".into(),
            tags: Vec::new(),
        };

        // max=30, link_weight=20。本文 "タイトル http://example.com/1" は
        // 換算 4 + 1 + 20 = 25 文字。残り5文字。
        // "\n#Rust"(6文字)は入らず捨てられ、本文のみ。
        let tags = vec!["Rust".to_string()];
        let result = optimizer
            .optimize(&article, "{title} {link}", 30, None, 20, &tags)
            .await
            .unwrap();
        assert_eq!(result, "タイトル http://example.com/1");
    }
}
