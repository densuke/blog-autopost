pub mod bluesky;
pub mod mastodon;
pub mod misskey;
pub mod models;
pub mod traits;
pub mod x;

use reqwest::Client;

/// バイト列が対応する画像形式かどうかをマジックバイトで判定する。
///
/// HTMLやリダイレクト結果(例: og:imageに動画URLが入っていた場合)など、
/// 画像でないものを弾くために使う。
pub fn is_supported_image(bytes: &[u8]) -> bool {
    image::guess_format(bytes).is_ok()
}

/// 画像URLからバイナリとMIMEタイプをダウンロードする。
///
/// - ネットワーク/HTTPエラー時は `Err`
/// - 取得できたが中身が画像でない場合は `Ok(None)`(呼び出し側で静かにスキップ)
/// - 画像の場合は `Ok(Some((bytes, content_type)))`
pub async fn download_image(
    client: &Client,
    url: &str,
) -> anyhow::Result<Option<(Vec<u8>, String)>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image: {}",
            response.status()
        ));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = response.bytes().await?.to_vec();

    if !is_supported_image(&bytes) {
        return Ok(None);
    }

    Ok(Some((bytes, content_type)))
}

/// 設定から SNS クライアントのリストを構築する。
///
/// 対応していない種別(Threads / Tumblr / 不明な設定)はスキップする。
/// クライアントの生成に失敗したアカウントもスキップされ、残りの構築は継続する。
///
/// CLI と Web UI の双方から使うため、ここに置いている。
pub fn build_clients_from_config(
    config: &crate::config::Config,
) -> Vec<std::sync::Arc<dyn traits::SnsClient + Send + Sync>> {
    use crate::config::SnsConfig;
    use std::sync::Arc;

    let mut clients: Vec<Arc<dyn traits::SnsClient + Send + Sync>> = Vec::new();

    for sns_conf in &config.sns {
        match sns_conf {
            SnsConfig::Mastodon {
                instance_url,
                access_token,
                name,
            } => {
                if let Ok(client) = mastodon::MastodonClient::new(
                    instance_url.clone(),
                    access_token.clone(),
                    name.clone(),
                ) {
                    clients.push(Arc::new(client));
                }
            }
            SnsConfig::Misskey {
                instance_url,
                access_token,
                name,
                ..
            } => {
                if let Ok(client) = misskey::MisskeyClient::new(
                    instance_url.clone(),
                    access_token.clone(),
                    name.clone(),
                ) {
                    clients.push(Arc::new(client));
                }
            }
            SnsConfig::Bluesky {
                identifier,
                password,
                name,
            } => {
                if let Ok(client) =
                    bluesky::BlueskyClient::new(identifier.clone(), password.clone(), name.clone())
                {
                    clients.push(Arc::new(client));
                }
            }
            SnsConfig::X {
                consumer_key,
                consumer_secret,
                access_token,
                access_token_secret,
                name,
            } => {
                if let Ok(client) = x::XClient::new(
                    consumer_key.clone(),
                    consumer_secret.clone(),
                    access_token.clone(),
                    access_token_secret.clone(),
                    name.clone(),
                ) {
                    clients.push(Arc::new(client));
                }
            }
            _ => {}
        }
    }

    clients
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_image_true_for_png() {
        // PNGのマジックバイト
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert!(is_supported_image(&png));
    }

    #[test]
    fn test_is_supported_image_false_for_html() {
        let html = b"<!DOCTYPE html><html><head></head></html>";
        assert!(!is_supported_image(html));
    }

    #[test]
    fn test_is_supported_image_false_for_empty() {
        assert!(!is_supported_image(&[]));
    }

    mod download {
        use super::*;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        /// テスト用の 1x1 PNG バイト列を作る。
        fn tiny_png() -> Vec<u8> {
            use image::{ImageFormat, RgbaImage};
            let img = RgbaImage::new(1, 1);
            let mut buf = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut buf, ImageFormat::Png)
                .expect("PNGの生成に失敗");
            buf.into_inner()
        }

        /// 画像を取得できた場合はバイト列とContent-Typeを返す。
        #[tokio::test]
        async fn test_download_image_returns_bytes_and_mime() {
            let server = MockServer::start().await;
            let png = tiny_png();
            Mock::given(method("GET"))
                .and(path("/a.png"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_bytes(png.clone())
                        .insert_header("content-type", "image/png"),
                )
                .mount(&server)
                .await;

            let result = download_image(&Client::new(), &format!("{}/a.png", server.uri()))
                .await
                .expect("ダウンロードに失敗");

            let (bytes, mime) = result.expect("画像として認識されるはず");
            assert_eq!(bytes, png);
            assert_eq!(mime, "image/png");
        }

        /// Content-Type ヘッダが無い場合は application/octet-stream 扱いになる。
        #[tokio::test]
        async fn test_download_image_defaults_content_type() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/noheader"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(tiny_png()))
                .mount(&server)
                .await;

            let (_, mime) = download_image(&Client::new(), &format!("{}/noheader", server.uri()))
                .await
                .unwrap()
                .expect("画像として認識されるはず");

            assert_eq!(mime, "application/octet-stream");
        }

        /// 中身が画像でない場合は Ok(None) を返し、呼び出し側で静かにスキップできる。
        #[tokio::test]
        async fn test_download_image_returns_none_for_html() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/page"))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_string("<!DOCTYPE html><html></html>")
                        .insert_header("content-type", "text/html"),
                )
                .mount(&server)
                .await;

            let result = download_image(&Client::new(), &format!("{}/page", server.uri()))
                .await
                .expect("HTTPとしては成功しているのでErrにはならない");

            assert!(result.is_none());
        }

        /// HTTPエラー応答は Err になる。
        #[tokio::test]
        async fn test_download_image_errors_on_404() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/missing"))
                .respond_with(ResponseTemplate::new(404))
                .mount(&server)
                .await;

            let result = download_image(&Client::new(), &format!("{}/missing", server.uri())).await;

            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("404"),
                "エラーメッセージにステータスを含むこと: {}",
                msg
            );
        }

        #[tokio::test]
        async fn test_download_image_errors_on_500() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/boom"))
                .respond_with(ResponseTemplate::new(500))
                .mount(&server)
                .await;

            assert!(
                download_image(&Client::new(), &format!("{}/boom", server.uri()))
                    .await
                    .is_err()
            );
        }

        /// 接続できないURLはネットワークエラーとして Err になる。
        #[tokio::test]
        async fn test_download_image_errors_on_connection_failure() {
            // 起動直後に停止したサーバのURIを使い、接続不能な状態を作る
            let uri = {
                let server = MockServer::start().await;
                server.uri()
            };

            assert!(
                download_image(&Client::new(), &format!("{}/x.png", uri))
                    .await
                    .is_err()
            );
        }
    }

    mod build_clients {
        use super::*;
        use crate::config::{Config, SnsConfig};
        use std::collections::HashMap;

        fn config_with(sns: Vec<SnsConfig>) -> Config {
            Config {
                announcement_text: None,
                blog: None,
                sns,
                templates: HashMap::new(),
                default_allowed_timings: None,
                allowed_timings_tolerance_minutes: None,
                allowed_timings: None,
                web_auth: None,
                extra: HashMap::new(),
            }
        }

        #[test]
        fn test_empty_config_builds_nothing() {
            assert!(build_clients_from_config(&config_with(vec![])).is_empty());
        }

        /// 対応する4種がすべて構築され、順序も設定どおりになる。
        #[test]
        fn test_builds_all_supported_kinds() {
            let clients = build_clients_from_config(&config_with(vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Misskey {
                    name: "misskey-main".to_string(),
                    instance_url: "https://misskey.example.com".to_string(),
                    access_token: "t".to_string(),
                    is_sensitive: None,
                },
                SnsConfig::Bluesky {
                    name: "bsky-main".to_string(),
                    identifier: "id".to_string(),
                    password: "pw".to_string(),
                },
                SnsConfig::X {
                    name: "x-main".to_string(),
                    consumer_key: "ck".to_string(),
                    consumer_secret: "cs".to_string(),
                    access_token: "at".to_string(),
                    access_token_secret: "ats".to_string(),
                },
            ]));

            let kinds: Vec<&str> = clients.iter().map(|c| c.name()).collect();
            assert_eq!(kinds, vec!["mastodon", "misskey", "bluesky", "x"]);

            let accounts: Vec<&str> = clients.iter().map(|c| c.account_name()).collect();
            assert_eq!(
                accounts,
                vec!["mstdn-main", "misskey-main", "bsky-main", "x-main"]
            );
        }

        /// 移植対象外や不明な設定はスキップされ、残りの構築は継続する。
        #[test]
        fn test_skips_unsupported_kinds() {
            let clients = build_clients_from_config(&config_with(vec![
                SnsConfig::Threads {
                    name: "threads-main".to_string(),
                    user_id: "u".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Tumblr {
                    name: "tumblr-main".to_string(),
                    consumer_key: "ck".to_string(),
                    consumer_secret: "cs".to_string(),
                    oauth_token: "ot".to_string(),
                    oauth_secret: "os".to_string(),
                    blog_identifier: "b".to_string(),
                },
                SnsConfig::Unknown,
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://mstdn.example.com".to_string(),
                    access_token: "t".to_string(),
                },
            ]));

            assert_eq!(clients.len(), 1);
            assert_eq!(clients[0].account_name(), "mstdn-main");
        }

        /// 同じ種別の複数アカウントもそれぞれ構築される。
        #[test]
        fn test_builds_multiple_accounts_of_same_kind() {
            let clients = build_clients_from_config(&config_with(vec![
                SnsConfig::Mastodon {
                    name: "mstdn-main".to_string(),
                    instance_url: "https://a.example.com".to_string(),
                    access_token: "t".to_string(),
                },
                SnsConfig::Mastodon {
                    name: "mstdn-sub".to_string(),
                    instance_url: "https://b.example.com".to_string(),
                    access_token: "t".to_string(),
                },
            ]));

            assert_eq!(clients.len(), 2);
            assert_eq!(clients[1].account_name(), "mstdn-sub");
        }
    }
}
