use async_trait::async_trait;
use feed_rs::parser;
use reqwest;

use crate::article::models::Article;
use crate::article::traits::FeedFetcher;

pub struct DefaultFeedFetcher;

impl DefaultFeedFetcher {
    pub fn new() -> Self {
        Self
    }

    /// フィードの内容（XML/JSONなどのバイト列）を解析して `Article` のリストに変換する
    fn parse_feed(content: &[u8], feed_name: &str) -> anyhow::Result<Vec<Article>> {
        let feed = parser::parse(content)?;
        let mut articles = Vec::new();

        for entry in feed.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            let link = entry.links.into_iter().next().map(|l| l.href).unwrap_or_default();
            
            let published_parsed = entry.published
                .or(entry.updated)
                .unwrap_or_else(|| chrono::Utc::now());

            // 概要欄(summary / content / media:description)からハッシュタグを抽出する。
            // YouTubeはタイトルにタグが無く media:description にタグが入ることがある。
            let mut description = String::new();
            if let Some(summary) = &entry.summary {
                description.push_str(&summary.content);
                description.push(' ');
            }
            if let Some(content) = &entry.content {
                if let Some(body) = &content.body {
                    description.push_str(body);
                    description.push(' ');
                }
            }
            for media in &entry.media {
                if let Some(desc) = &media.description {
                    description.push_str(&desc.content);
                    description.push(' ');
                }
            }
            let tags = crate::text::tags::extract_hashtags(&description);

            // media に動画URL(YouTube等)が入っている場合があるため、
            // 画像らしいURLのみを採用する。
            let image_url = entry.media.into_iter()
                .flat_map(|m| m.content)
                .filter_map(|c| c.url.map(|u| u.to_string()))
                .find(|u| super::image_extractor::is_probable_image_url(u));

            articles.push(Article {
                title,
                link,
                published_parsed,
                image_url,
                feed_name: feed_name.to_string(),
                tags,
            });
        }

        Ok(articles)
    }
}

impl DefaultFeedFetcher {
    /// 診断情報付きでフィードを取得・解析する。
    ///
    /// `verbose` が真のとき、HTTP ステータス・Content-Type・本文サイズを表示し、
    /// 解析に失敗した場合は本文の先頭部分をダンプして原因究明を助ける。
    pub async fn fetch_articles_verbose(
        &self,
        feed_url: &str,
        feed_name: &str,
        verbose: bool,
    ) -> anyhow::Result<Vec<Article>> {
        if verbose {
            eprintln!("[verbose] GET {feed_url}");
        }
        let response = reqwest::get(feed_url).await?;

        if verbose {
            let status = response.status();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("(none)")
                .to_string();
            let final_url = response.url().to_string();
            eprintln!("[verbose] status = {status}");
            eprintln!("[verbose] content-type = {content_type}");
            eprintln!("[verbose] final url = {final_url}");
        }

        let bytes = response.bytes().await?;
        if verbose {
            eprintln!("[verbose] body size = {} bytes", bytes.len());
        }

        match Self::parse_feed(&bytes, feed_name) {
            Ok(articles) => {
                if verbose {
                    eprintln!("[verbose] parsed {} entries", articles.len());
                }
                Ok(articles)
            }
            Err(e) => {
                if verbose {
                    let head_len = bytes.len().min(1000);
                    let head = String::from_utf8_lossy(&bytes[..head_len]);
                    eprintln!("[verbose] parse failed: {e}");
                    eprintln!("[verbose] body head ({head_len} bytes):\n{head}");
                }
                Err(e)
            }
        }
    }
}

impl Default for DefaultFeedFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeedFetcher for DefaultFeedFetcher {
    async fn fetch_articles(&self, feed_url: &str, feed_name: &str) -> anyhow::Result<Vec<Article>> {
        let response = reqwest::get(feed_url).await?;
        let bytes = response.bytes().await?;
        Self::parse_feed(&bytes, feed_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_parse_feed_rss() {
        let feed_xml = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <rss version="2.0">
                <channel>
                    <title>Test Feed</title>
                    <link>http://example.com/</link>
                    <description>Test Description</description>
                    <item>
                        <title>Article 1</title>
                        <link>http://example.com/article1</link>
                        <pubDate>Mon, 01 Jan 2024 12:00:00 GMT</pubDate>
                    </item>
                    <item>
                        <title>Article 2</title>
                        <link>http://example.com/article2</link>
                        <pubDate>Tue, 02 Jan 2024 15:30:00 GMT</pubDate>
                    </item>
                </channel>
            </rss>
        "#;

        let articles = DefaultFeedFetcher::parse_feed(feed_xml.as_bytes(), "test_feed").unwrap();
        
        assert_eq!(articles.len(), 2);
        
        assert_eq!(articles[0].title, "Article 1");
        assert_eq!(articles[0].link, "http://example.com/article1");
        assert_eq!(articles[0].feed_name, "test_feed");
        assert_eq!(articles[0].published_parsed, Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap());
        assert_eq!(articles[0].image_url, None);

        assert_eq!(articles[1].title, "Article 2");
        assert_eq!(articles[1].link, "http://example.com/article2");
        assert_eq!(articles[1].feed_name, "test_feed");
        assert_eq!(articles[1].published_parsed, Utc.with_ymd_and_hms(2024, 1, 2, 15, 30, 0).unwrap());
    }

    #[test]
    fn test_parse_feed_atom() {
        let feed_xml = r#"
            <?xml version="1.0" encoding="utf-8"?>
            <feed xmlns="http://www.w3.org/2005/Atom">
                <title>Test Atom Feed</title>
                <link href="http://example.com/"/>
                <updated>2024-01-01T12:00:00Z</updated>
                <author><name>John Doe</name></author>
                <id>urn:uuid:60a76c80-d399-11d9-b93C-0003939e0af6</id>
                <entry>
                    <title>Atom Article 1</title>
                    <link href="http://example.com/atom1"/>
                    <id>urn:uuid:1225c695-cfb8-4ebb-aaaa-80da344efa6a</id>
                    <updated>2024-01-03T10:00:00Z</updated>
                    <summary>Some text.</summary>
                </entry>
            </feed>
        "#;

        let articles = DefaultFeedFetcher::parse_feed(feed_xml.as_bytes(), "atom_feed").unwrap();
        
        assert_eq!(articles.len(), 1);
        
        assert_eq!(articles[0].title, "Atom Article 1");
        assert_eq!(articles[0].link, "http://example.com/atom1");
        assert_eq!(articles[0].feed_name, "atom_feed");
        assert_eq!(articles[0].published_parsed, Utc.with_ymd_and_hms(2024, 1, 3, 10, 0, 0).unwrap());
    }
}
