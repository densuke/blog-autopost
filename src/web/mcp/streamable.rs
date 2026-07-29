//! Streamable HTTP トランスポート (MCP 2025-03-26 以降)。
//!
//! 単一のエンドポイント `/api/mcp` で POST を受け、JSON-RPC のレスポンスを
//! そのままボディで返す。旧来の HTTP+SSE (`/api/mcp/sse`) も併置してあり、
//! 古いクライアントはそちらを使える。
//!
//! サーバからクライアントへ自発的にメッセージを送る必要がないため、
//! SSE ストリームは開かず、`GET` と `DELETE` には 405 を返す。
//! セッションIDも払い出さない (tool の実行にサーバ側の状態が要らない)。

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

use super::protocol;
use crate::web::AppState;

/// `MCP-Protocol-Version` ヘッダの名前。
const PROTOCOL_VERSION_HEADER: &str = "MCP-Protocol-Version";

/// `POST /api/mcp` — JSON-RPC リクエストを受け、レスポンスをボディで返す。
///
/// 通知 (`id` を持たないリクエスト) には `202 Accepted` を空ボディで返す。
pub async fn mcp_post_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    if let Some(rejection) = reject_bad_request(&state, &headers) {
        return rejection;
    }

    let Ok(rpc_req) = serde_json::from_str::<serde_json::Value>(&body) else {
        // JSON として読めない入力は id を持てないため、id なしのエラーを返す
        return parse_error_response();
    };

    match protocol::build_mcp_response(state, rpc_req).await {
        // リクエストへの応答は JSON で返す
        Some(response) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            serde_json::to_string(&response)
                .unwrap_or_else(|_| r#"{"jsonrpc":"2.0","error":{"code":-32603,"message":"Failed to serialize response"},"id":null}"#.to_string()),
        )
            .into_response(),
        // 通知は受け付けた旨だけを返す
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// `GET /api/mcp` — サーバ発の SSE ストリームは提供しない。
///
/// 仕様はこの場合 405 を返すよう定めている。
pub async fn mcp_get_handler() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        "This server does not offer a server-to-client SSE stream at this endpoint.",
    )
        .into_response()
}

/// `DELETE /api/mcp` — セッションを持たないため終了操作もない。
pub async fn mcp_delete_handler() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        "This server does not use sessions, so there is nothing to terminate.",
    )
        .into_response()
}

/// リクエストを受け付けられない場合にその応答を返す。
///
/// 受け付けられる場合は `None` を返す。
fn reject_bad_request(state: &Arc<AppState>, headers: &HeaderMap) -> Option<Response> {
    if let Some(response) = reject_disallowed_origin(state, headers) {
        return Some(response);
    }
    reject_unsupported_protocol_version(headers)
}

/// 対応していないプロトコルバージョンを拒否する。
///
/// ヘッダが無い場合は仕様の既定版とみなして受け付ける。
fn reject_unsupported_protocol_version(headers: &HeaderMap) -> Option<Response> {
    let value = headers.get(PROTOCOL_VERSION_HEADER)?.to_str().ok();

    match value {
        Some(v) if protocol::is_supported_protocol_version(v) => None,
        other => Some(
            (
                StatusCode::BAD_REQUEST,
                format!(
                    "Unsupported MCP-Protocol-Version: {}. Supported: {}",
                    other.unwrap_or("(invalid)"),
                    protocol::SUPPORTED_PROTOCOL_VERSIONS.join(", ")
                ),
            )
                .into_response(),
        ),
    }
}

/// 許可していないオリジンからのリクエストを拒否する。
///
/// DNS リバインディング攻撃を防ぐため、仕様が `Origin` の検証を求めている。
/// `Origin` を送るのはブラウザだけで、MCP クライアントは送らない。
/// そのためヘッダがある場合だけ検証し、既定では拒否する。
/// ブラウザから使う必要があれば `mcp.allowed_origins` に列挙する。
fn reject_disallowed_origin(state: &Arc<AppState>, headers: &HeaderMap) -> Option<Response> {
    let origin = headers.get(header::ORIGIN)?.to_str().ok().unwrap_or("");

    if is_origin_allowed(state, origin) {
        return None;
    }

    Some(
        (
            StatusCode::FORBIDDEN,
            format!(
                "Origin not allowed: {}. Add it to mcp.allowed_origins if this is intended.",
                origin
            ),
        )
            .into_response(),
    )
}

/// このオリジンを受け付けるかどうかを返す。
fn is_origin_allowed(state: &Arc<AppState>, origin: &str) -> bool {
    let Some(allowed) = state
        .config
        .mcp
        .as_ref()
        .and_then(|m| m.allowed_origins.as_deref())
    else {
        // 未設定ならブラウザからの利用は想定しない
        return false;
    };

    allowed.iter().any(|a| a == "*" || a == origin)
}

/// JSON として解釈できなかった入力への応答を返す。
fn parse_error_response() -> Response {
    (
        StatusCode::BAD_REQUEST,
        [(header::CONTENT_TYPE, "application/json")],
        r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"},"id":null}"#,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;
    use crate::web::tests::{TestApp, setup_test_app, setup_test_app_with_config};
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const SECRET: &str = "test-secret-token";

    fn app() -> TestApp {
        setup_test_app(Some(SECRET.to_string()))
    }

    /// `/api/mcp` への POST を組み立てる。
    fn post(body: &str, extra: &[(&str, &str)]) -> Request<Body> {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/mcp")
            .header("X-Api-Key", SECRET)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        for (name, value) in extra {
            builder = builder.header(*name, *value);
        }
        builder.body(Body::from(body.to_string())).unwrap()
    }

    /// JSON-RPC リクエストの POST を組み立てる。
    fn rpc_post(value: serde_json::Value) -> Request<Body> {
        post(&value.to_string(), &[])
    }

    /// レスポンスボディを JSON として読み出す。
    async fn json_body(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("ボディの読み出しに失敗");
        serde_json::from_slice(&bytes).expect("JSONとして解釈できない")
    }

    // --- POST: リクエスト ---

    /// initialize のレスポンスがボディで返る。
    #[tokio::test]
    async fn postはレスポンスをボディで返す() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": { "protocolVersion": "2025-06-18" },
                "id": 1
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json"),
            "SSE ではなく JSON で返す"
        );

        let json = json_body(response).await;
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["protocolVersion"], "2025-06-18");
    }

    /// tools/list もボディで返る。
    #[tokio::test]
    async fn tools_listをボディで返す() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/list",
                "id": 2
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;

        let names: Vec<&str> = json["result"]["tools"]
            .as_array()
            .expect("tools は配列")
            .iter()
            .filter_map(|t| t["name"].as_str())
            .collect();
        assert!(names.contains(&"post_now"));
        assert!(names.contains(&"get_next_slots"));
    }

    /// tools/call も同じ経路で実行できる。
    #[tokio::test]
    async fn tools_callをボディで返す() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": { "name": "list_schedules", "arguments": {} },
                "id": 3
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;
        assert_eq!(json["result"]["content"][0]["type"], "text");
    }

    /// 未知のメソッドもエラーレスポンスをボディで返す。
    #[tokio::test]
    async fn 未知のメソッドはエラーをボディで返す() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "resources/list",
                "id": 4
            })))
            .await
            .unwrap();

        // JSON-RPC のエラーは HTTP としては成功で返す
        assert_eq!(response.status(), StatusCode::OK);
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], -32601);
    }

    // --- POST: 通知 ---

    /// 通知には 202 を空ボディで返す。
    #[tokio::test]
    async fn 通知には202を返す() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "initialized"
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(bytes.is_empty(), "通知への応答は空ボディ");
    }

    /// id を持たないリクエストも通知として扱う。
    #[tokio::test]
    async fn id無しのリクエストも202になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/list"
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    // --- POST: 不正な入力 ---

    /// JSON として読めない入力は 400 とパースエラーを返す。
    #[tokio::test]
    async fn 壊れたjsonは400になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(post("{ this is not json", &[]))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json = json_body(response).await;
        assert_eq!(json["error"]["code"], -32700);
        assert!(json["id"].is_null());
    }

    // --- GET / DELETE ---

    /// GET は 405 を返す。
    #[tokio::test]
    async fn getは405になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp")
                    .header("X-Api-Key", SECRET)
                    .header(header::ACCEPT, "text/event-stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // サーバ発ストリームを提供しないことを示す
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    /// DELETE も 405 を返す。
    #[tokio::test]
    async fn deleteは405になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/mcp")
                    .header("X-Api-Key", SECRET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    // --- 認証 ---

    /// キーが無ければ 401 を返す。
    #[tokio::test]
    async fn 認証なしは401になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/mcp")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "jsonrpc": "2.0", "method": "initialize", "id": 1 })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// Cookie セッションでは認証されない。
    #[tokio::test]
    async fn cookieセッションでは認証されない() {
        let app = app();
        {
            let mut sessions = app.state.sessions.write().await;
            sessions.insert(
                "live".to_string(),
                crate::web::session::Session::new("admin".to_string(), 24),
            );
        }

        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/mcp")
                    .header(header::COOKIE, "session_id=live")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::json!({ "jsonrpc": "2.0", "method": "initialize", "id": 1 })
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // ブラウザのセッションで即時投稿を叩ける経路を残さない
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// 専用キーを設定すると secret_key では通らない。
    #[tokio::test]
    async fn 専用キー設定時はsecret_keyを拒否する() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.mcp = Some(McpConfig {
                api_key: Some("mcp-only-key".to_string()),
                ..Default::default()
            });
        });

        let make = |key: &str| {
            Request::builder()
                .method("POST")
                .uri("/api/mcp")
                .header("X-Api-Key", key)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({ "jsonrpc": "2.0", "method": "initialize", "id": 1 })
                        .to_string(),
                ))
                .unwrap()
        };

        let ok = app
            .router
            .clone()
            .oneshot(make("mcp-only-key"))
            .await
            .unwrap();
        assert_eq!(ok.status(), StatusCode::OK);

        let rejected = app.router.clone().oneshot(make(SECRET)).await.unwrap();
        assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);
    }

    // --- MCP-Protocol-Version ヘッダ ---

    /// 対応している版のヘッダは受け付ける。
    #[tokio::test]
    async fn 対応する版のヘッダを受け付ける() {
        let app = app();

        for version in protocol::SUPPORTED_PROTOCOL_VERSIONS {
            let response = app
                .router
                .clone()
                .oneshot(post(
                    &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                        .to_string(),
                    &[(PROTOCOL_VERSION_HEADER, version)],
                ))
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK, "版: {}", version);
        }
    }

    /// 対応していない版のヘッダは 400 で拒否する。
    #[tokio::test]
    async fn 未対応の版のヘッダは400になる() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(post(
                &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                    .to_string(),
                &[(PROTOCOL_VERSION_HEADER, "1999-01-01")],
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    /// ヘッダが無ければ既定版とみなして受け付ける。
    #[tokio::test]
    async fn 版のヘッダが無くても受け付ける() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/list",
                "id": 1
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- Origin 検証 ---

    /// Origin が付いていると既定では拒否する。
    #[tokio::test]
    async fn 未設定ならoriginつきを拒否する() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(post(
                &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                    .to_string(),
                &[("Origin", "https://evil.example.com")],
            ))
            .await
            .unwrap();

        // DNS リバインディング対策。MCP クライアントは Origin を送らない
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    /// 許可したオリジンは受け付ける。
    #[tokio::test]
    async fn 許可したoriginは受け付ける() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.mcp = Some(McpConfig {
                allowed_origins: Some(vec!["https://ui.example.com".to_string()]),
                ..Default::default()
            });
        });

        let response = app
            .router
            .clone()
            .oneshot(post(
                &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                    .to_string(),
                &[("Origin", "https://ui.example.com")],
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// 許可リストに無いオリジンは拒否する。
    #[tokio::test]
    async fn 許可リスト外のoriginを拒否する() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.mcp = Some(McpConfig {
                allowed_origins: Some(vec!["https://ui.example.com".to_string()]),
                ..Default::default()
            });
        });

        let response = app
            .router
            .clone()
            .oneshot(post(
                &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                    .to_string(),
                &[("Origin", "https://evil.example.com")],
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    /// ワイルドカードを設定すると任意のオリジンを受け付ける。
    #[tokio::test]
    async fn ワイルドカードは任意のoriginを受け付ける() {
        let app = setup_test_app_with_config(Some(SECRET.to_string()), |config| {
            config.mcp = Some(McpConfig {
                allowed_origins: Some(vec!["*".to_string()]),
                ..Default::default()
            });
        });

        let response = app
            .router
            .clone()
            .oneshot(post(
                &serde_json::json!({ "jsonrpc": "2.0", "method": "tools/list", "id": 1 })
                    .to_string(),
                &[("Origin", "https://anything.example.com")],
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// Origin が無いリクエストは検証の対象にならない。
    #[tokio::test]
    async fn origin無しは検証しない() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(rpc_post(serde_json::json!({
                "jsonrpc": "2.0",
                "method": "tools/list",
                "id": 1
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // --- 旧トランスポートとの併存 ---

    /// 旧来の SSE エンドポイントも引き続き使える。
    #[tokio::test]
    async fn 旧sseエンドポイントも残っている() {
        let app = app();

        let response = app
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/sse")
                    .header("X-Api-Key", SECRET)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
