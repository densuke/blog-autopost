use async_trait::async_trait;
use feed_rs::parser;
use reqwest;

use crate::article::models::Article;
use crate::article::traits::FeedFetcher;

/// フィード取得時に用いる User-Agent。
///
/// YouTube の RSS サーバー(`www.youtube.com/feeds/videos.xml`)は、User-Agent が
/// 無い/非ブラウザのリクエストに対して 404 の HTML を返すため、feed-rs が
/// 「no root element」で失敗する。ブラウザ風の UA を付けることで正常な XML を
/// 受け取れるようにする。
const FEED_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

pub struct DefaultFeedFetcher {
    client: reqwest::Client,
}

impl DefaultFeedFetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(FEED_USER_AGENT)
            .build()
            .unwrap_or_default();
        Self { client }
    }

    /// フィードの内容（XML/JSONなどのバイト列）を解析して `Article` のリストに変換する
    fn parse_feed(content: &[u8], feed_name: &str) -> anyhow::Result<Vec<Article>> {
        let feed = parser::parse(content)?;
        let mut articles = Vec::new();

        for entry in feed.entries {
            let title = entry.title.map(|t| t.content).unwrap_or_default();
            let link = entry
                .links
                .into_iter()
                .next()
                .map(|l| l.href)
                .unwrap_or_default();

            let published_parsed = entry
                .published
                .or(entry.updated)
                .unwrap_or_else(chrono::Utc::now);

            // 概要欄(summary / content / media:description)からハッシュタグを抽出する。
            // YouTubeはタイトルにタグが無く media:description にタグが入ることがある。
            let mut description = String::new();
            if let Some(summary) = &entry.summary {
                description.push_str(&summary.content);
                description.push(' ');
            }
            if let Some(content) = &entry.content
                && let Some(body) = &content.body
            {
                description.push_str(body);
                description.push(' ');
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
            let image_url = entry
                .media
                .into_iter()
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

/// フィード取得の最大試行回数(初回 + リトライ)。YouTube の RSS は同一 URL でも
/// 成功率が3割程度まで落ちることがあるため、複数回リトライして取りこぼしを防ぐ。
const FEED_MAX_ATTEMPTS: usize = 6;
/// リトライ時の基準待ち時間。試行ごとに倍にして待機する(1s, 2s, 4s...)。
const FEED_RETRY_BASE_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
/// バックオフ待ち時間の上限。指数的に伸びすぎないよう頭打ちにする。
const FEED_RETRY_MAX_DELAY: std::time::Duration = std::time::Duration::from_secs(8);

impl DefaultFeedFetcher {
    /// フィードを1回だけ取得・解析する。非 2xx ステータスや解析失敗はエラーとする。
    async fn fetch_once(
        &self,
        feed_url: &str,
        feed_name: &str,
        verbose: bool,
    ) -> anyhow::Result<Vec<Article>> {
        if verbose {
            eprintln!("[verbose] GET {feed_url}");
        }
        let response = self.client.get(feed_url).send().await?;
        let status = response.status();

        if verbose {
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

        // 非 2xx はフィード本文が得られないため、リトライ対象のエラーとして扱う。
        // YouTube の RSS は断続的に 404/500 を返すことがあるため重要。
        if !status.is_success() {
            anyhow::bail!("HTTP status {} for feed '{}'", status, feed_name);
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

    /// 診断情報付きでフィードを取得・解析する。バックオフ付きでリトライする。
    ///
    /// `verbose` が真のとき、HTTP ステータス・Content-Type・本文サイズを表示し、
    /// 解析に失敗した場合は本文の先頭部分をダンプして原因究明を助ける。
    ///
    /// YouTube の RSS エンドポイントは同一 URL でも 200/404/500 を不定期に返す
    /// ことがあるため、失敗時は待機して再取得を試みる。
    pub async fn fetch_articles_verbose(
        &self,
        feed_url: &str,
        feed_name: &str,
        verbose: bool,
    ) -> anyhow::Result<Vec<Article>> {
        let mut last_err = None;
        for attempt in 1..=FEED_MAX_ATTEMPTS {
            match self.fetch_once(feed_url, feed_name, verbose).await {
                Ok(articles) => return Ok(articles),
                Err(e) => {
                    if attempt < FEED_MAX_ATTEMPTS {
                        let delay = (FEED_RETRY_BASE_DELAY * 2u32.pow((attempt - 1) as u32))
                            .min(FEED_RETRY_MAX_DELAY);
                        if verbose {
                            eprintln!(
                                "[verbose] attempt {attempt}/{FEED_MAX_ATTEMPTS} failed: {e}. retrying in {:?}...",
                                delay
                            );
                        }
                        tokio::time::sleep(delay).await;
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err
            .unwrap_or_else(|| anyhow::anyhow!("フィード '{}' の取得に失敗しました", feed_name)))
    }
}

impl Default for DefaultFeedFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeedFetcher for DefaultFeedFetcher {
    async fn fetch_articles(
        &self,
        feed_url: &str,
        feed_name: &str,
    ) -> anyhow::Result<Vec<Article>> {
        self.fetch_articles_verbose(feed_url, feed_name, false)
            .await
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
        assert_eq!(
            articles[0].published_parsed,
            Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap()
        );
        assert_eq!(articles[0].image_url, None);

        assert_eq!(articles[1].title, "Article 2");
        assert_eq!(articles[1].link, "http://example.com/article2");
        assert_eq!(articles[1].feed_name, "test_feed");
        assert_eq!(
            articles[1].published_parsed,
            Utc.with_ymd_and_hms(2024, 1, 2, 15, 30, 0).unwrap()
        );
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
        assert_eq!(
            articles[0].published_parsed,
            Utc.with_ymd_and_hms(2024, 1, 3, 10, 0, 0).unwrap()
        );
    }
}
