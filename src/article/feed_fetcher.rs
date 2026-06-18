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

            let image_url = entry.media.into_iter()
                .flat_map(|m| m.content)
                .filter_map(|c| c.url.map(|u| u.to_string()))
                .next();

            articles.push(Article {
                title,
                link,
                published_parsed,
                image_url,
                feed_name: feed_name.to_string(),
            });
        }

        Ok(articles)
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
