//! MCP (Model Context Protocol) サーバの実装。
//!
//! トランスポートは HTTP+SSE (2024-11-05 仕様)。
//! `GET /api/mcp/sse` で接続を張り、`event: endpoint` でメッセージ送信先を通知する。
//! クライアントは `POST /api/mcp/message?session_id=...` へ JSON-RPC を送り、
//! レスポンスは SSE ストリーム側へ流れる。
//!
//! - [`protocol`] : JSON-RPC の解釈とレスポンス組み立て
//! - [`tools`] : tool の定義と実行

pub(crate) mod protocol;
pub(crate) mod tools;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use super::AppState;

/// SSE 接続が Drop (切断) された際に `mcp_sessions` からセッションを削除するラッパーストリーム。
struct SessionCleanupStream<S> {
    inner: S,
    session_id: String,
    mcp_sessions: Arc<tokio::sync::RwLock<HashMap<String, tokio::sync::mpsc::Sender<Event>>>>,
}

impl<S> Drop for SessionCleanupStream<S> {
    fn drop(&mut self) {
        let mcp_sessions = self.mcp_sessions.clone();
        let session_id = self.session_id.clone();
        tokio::spawn(async move {
            let mut guard = mcp_sessions.write().await;
            guard.remove(&session_id);
            println!("MCP SSE Session disconnected & cleaned up: {}", session_id);
        });
    }
}

impl<S: tokio_stream::Stream + Unpin> tokio_stream::Stream for SessionCleanupStream<S> {
    type Item = S::Item;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

/// `GET /api/mcp/sse` — MCP の SSE 接続を確立する。
///
/// セッションを登録し、最初に `endpoint` イベントでメッセージ送信先の URL を返す。
pub async fn mcp_sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let session_id = format!("mcp-{}", chrono::Utc::now().timestamp_micros());
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    // セッションを登録
    {
        let mut mcp_sessions = state.mcp_sessions.write().await;
        mcp_sessions.insert(session_id.clone(), tx.clone());
        println!("MCP SSE Session connected: {}", session_id);
    }

    // 最初の endpoint イベントを送信して、クライアントへメッセージ送信先を指定する
    let endpoint_url = format!("/api/mcp/message?session_id={}", session_id);
    let init_event = Event::default().event("endpoint").data(endpoint_url);

    let _ = tx.send(init_event).await;

    // ストリームの作成
    let rx_stream = ReceiverStream::new(rx).map(Ok);

    // 切断検知時にセッション削除するストリームに変換
    let clean_stream = SessionCleanupStream {
        inner: rx_stream,
        session_id,
        mcp_sessions: state.mcp_sessions.clone(),
    };

    Sse::new(clean_stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// `POST /api/mcp/message` のクエリパラメータ。
#[derive(Deserialize)]
pub struct McpQuery {
    /// SSE 接続時に払い出されたセッション ID。
    pub session_id: String,
}

/// `POST /api/mcp/message` — JSON-RPC リクエストを受け付ける。
///
/// 処理は非同期に行い、即座に `202 Accepted` を返す。
/// レスポンス本体は SSE ストリーム側へ送出される。
pub async fn mcp_message_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<McpQuery>,
    Json(rpc_req): Json<serde_json::Value>,
) -> impl axum::response::IntoResponse {
    let state_clone = state.clone();
    let session_id = query.session_id.clone();

    tokio::spawn(async move {
        if let Err(e) = dispatch_mcp_response(state_clone, &session_id, rpc_req).await {
            println!(
                "Error handling MCP request for session {}: {:?}",
                session_id, e
            );
        }
    });

    StatusCode::ACCEPTED
}

/// JSON-RPC を処理し、結果を指定セッションの SSE ストリームへ送出する。
///
/// レスポンスを返さない通知の場合は何も送出しない。
async fn dispatch_mcp_response(
    state: Arc<AppState>,
    session_id: &str,
    req: serde_json::Value,
) -> anyhow::Result<()> {
    let Some(response) = protocol::build_mcp_response(state.clone(), req).await else {
        return Ok(());
    };

    let mcp_sessions = state.mcp_sessions.read().await;
    if let Some(tx) = mcp_sessions.get(session_id) {
        let event = Event::default()
            .event("message")
            .data(serde_json::to_string(&response)?);
        let _ = tx.send(event).await;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::web::tests::{TestApp, setup_test_app};
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use tower::ServiceExt;

    const SECRET: &str = "test-secret-token";

    /// SSE の読み取りに設ける上限。keep-alive で終わらないストリームを打ち切る。
    const SSE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    fn app_with_auth() -> TestApp {
        setup_test_app(Some(SECRET.to_string()))
    }

    /// SSE のレスポンスから次のデータフレームを1つ読み出す。
    ///
    /// ボディ全体を読むと keep-alive のせいで終わらないため、
    /// フレーム単位で取り出して打ち切る。
    async fn next_frame(body: &mut axum::body::Body) -> String {
        use http_body_util::BodyExt;

        let frame = tokio::time::timeout(SSE_TIMEOUT, body.frame())
            .await
            .expect("SSEフレームの待機がタイムアウトした")
            .expect("ストリームが終了した")
            .expect("フレームの読み出しに失敗");
        let data = frame.into_data().expect("データフレームであること");
        String::from_utf8(data.to_vec()).expect("UTF-8として解釈できる")
    }

    /// MCPのメッセージ受付は 202 Accepted を返す(処理は非同期)。
    #[tokio::test]
    async fn test_mcp_message_returns_accepted() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/api/mcp/message?session_id=test-session")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "initialize",
                    "id": 1
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    /// tools/list も同様に受け付けられる。
    #[tokio::test]
    async fn test_mcp_message_tools_list() {
        let app = app_with_auth();

        let request = Request::builder()
            .method("POST")
            .uri("/api/mcp/message?session_id=test-session")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "tools/list",
                    "id": 2
                })
                .to_string(),
            ))
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // --- SSE ---

    /// SSE 接続は最初に endpoint イベントでメッセージ送信先を通知する。
    #[tokio::test]
    async fn sse接続はendpointイベントを返す() {
        let app = app_with_auth();

        let request = Request::builder()
            .uri("/api/mcp/sse")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.starts_with("text/event-stream")),
            "SSE の Content-Type であること"
        );

        let mut body = response.into_body();
        let frame = next_frame(&mut body).await;

        assert!(
            frame.contains("event: endpoint"),
            "実際のフレーム: {}",
            frame
        );
        assert!(
            frame.contains("data: /api/mcp/message?session_id=mcp-"),
            "実際のフレーム: {}",
            frame
        );
    }

    /// SSE 接続で払い出したセッションは mcp_sessions に登録される。
    #[tokio::test]
    async fn sse接続はセッションを登録する() {
        let app = app_with_auth();

        assert!(
            app.state.mcp_sessions.read().await.is_empty(),
            "接続前は空であること"
        );

        let request = Request::builder()
            .uri("/api/mcp/sse")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();
        let mut body = response.into_body();
        let frame = next_frame(&mut body).await;

        let session_id = frame
            .split("session_id=")
            .nth(1)
            .expect("endpoint イベントに session_id が含まれる")
            .trim()
            .to_string();

        assert!(
            app.state
                .mcp_sessions
                .read()
                .await
                .contains_key(&session_id),
            "払い出したセッションが登録されていること: {}",
            session_id
        );
    }

    /// SSE 接続中に送った JSON-RPC の結果が、同じストリームへ流れてくる。
    #[tokio::test]
    async fn sseストリームへjson_rpcの結果が流れる() {
        let app = app_with_auth();

        let sse_request = Request::builder()
            .uri("/api/mcp/sse")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(sse_request).await.unwrap();
        let mut body = response.into_body();

        // 1フレーム目は endpoint イベント。ここから session_id を取り出す
        let endpoint_frame = next_frame(&mut body).await;
        let session_id = endpoint_frame
            .split("session_id=")
            .nth(1)
            .expect("session_id が含まれる")
            .trim()
            .to_string();

        let message_request = Request::builder()
            .method("POST")
            .uri(format!("/api/mcp/message?session_id={}", session_id))
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "initialize",
                    "id": 1
                })
                .to_string(),
            ))
            .unwrap();

        let accepted = app.router.clone().oneshot(message_request).await.unwrap();
        assert_eq!(accepted.status(), StatusCode::ACCEPTED);

        // 2フレーム目に JSON-RPC のレスポンスが載る
        let message_frame = next_frame(&mut body).await;
        assert!(
            message_frame.contains("event: message"),
            "実際のフレーム: {}",
            message_frame
        );

        let payload = message_frame
            .lines()
            .find_map(|l| l.strip_prefix("data: "))
            .expect("data 行があること");
        let json: serde_json::Value = serde_json::from_str(payload).expect("JSONとして解釈できる");

        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["serverInfo"]["name"], "blog-autopost-rs");
    }

    /// 通知 (id なし) はレスポンスを流さない。
    #[tokio::test]
    async fn 通知はsseへ流れない() {
        let app = app_with_auth();

        let sse_request = Request::builder()
            .uri("/api/mcp/sse")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(sse_request).await.unwrap();
        let mut body = response.into_body();
        let endpoint_frame = next_frame(&mut body).await;
        let session_id = endpoint_frame
            .split("session_id=")
            .nth(1)
            .expect("session_id が含まれる")
            .trim()
            .to_string();

        // 通知を送っても message イベントは流れない
        let notify = Request::builder()
            .method("POST")
            .uri(format!("/api/mcp/message?session_id={}", session_id))
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({ "jsonrpc": "2.0", "method": "initialized" }).to_string(),
            ))
            .unwrap();
        app.router.clone().oneshot(notify).await.unwrap();

        // 続けて id 付きを送り、届く1フレーム目が後者の結果であることを確かめる
        let call = Request::builder()
            .method("POST")
            .uri(format!("/api/mcp/message?session_id={}", session_id))
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::json!({ "jsonrpc": "2.0", "method": "initialize", "id": 9 })
                    .to_string(),
            ))
            .unwrap();
        app.router.clone().oneshot(call).await.unwrap();

        let frame = next_frame(&mut body).await;
        let payload = frame
            .lines()
            .find_map(|l| l.strip_prefix("data: "))
            .expect("data 行があること");
        let json: serde_json::Value = serde_json::from_str(payload).expect("JSONとして解釈できる");

        assert_eq!(json["id"], 9, "通知の分が挟まっていないこと");
    }

    /// SSE 接続が切れるとセッションは掃除される。
    #[tokio::test]
    async fn sse切断でセッションが掃除される() {
        let app = app_with_auth();

        let request = Request::builder()
            .uri("/api/mcp/sse")
            .header("X-Api-Key", SECRET)
            .body(Body::empty())
            .unwrap();

        let response = app.router.clone().oneshot(request).await.unwrap();
        let mut body = response.into_body();
        let frame = next_frame(&mut body).await;
        let session_id = frame
            .split("session_id=")
            .nth(1)
            .expect("session_id が含まれる")
            .trim()
            .to_string();

        assert!(
            app.state
                .mcp_sessions
                .read()
                .await
                .contains_key(&session_id)
        );

        // 接続を落とす。掃除は Drop 内の tokio::spawn で行われるため即時ではない
        drop(body);

        let deadline = std::time::Instant::now() + SSE_TIMEOUT;
        loop {
            if !app
                .state
                .mcp_sessions
                .read()
                .await
                .contains_key(&session_id)
            {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "切断後にセッションが掃除されない: {}",
                session_id
            );
            tokio::task::yield_now().await;
        }
    }
}
