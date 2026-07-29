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

    fn app_with_auth() -> TestApp {
        setup_test_app(Some(SECRET.to_string()))
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
}
