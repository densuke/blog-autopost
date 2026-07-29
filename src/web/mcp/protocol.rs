//! MCP の JSON-RPC プロトコル処理。
//!
//! トランスポート (SSE) から切り離してあり、[`build_mcp_response`] は
//! リクエストの JSON を受けてレスポンスの JSON を返すだけの純粋な処理である。
//! 送出は [`super::dispatch_mcp_response`] が担当する。

use serde_json::json;
use std::sync::Arc;

use super::tools;
use crate::web::AppState;

/// このサーバが実装する MCP のプロトコルバージョン。
const PROTOCOL_VERSION: &str = "2024-11-05";

/// JSON-RPC リクエストを処理し、クライアントへ返すべきレスポンスを返す。
///
/// 通知 (`id` を持たないリクエスト) の場合は `None` を返す。
/// SSE への送出は行わないため、テストから直接呼び出して結果を検証できる。
pub(crate) async fn build_mcp_response(
    state: Arc<AppState>,
    req: serde_json::Value,
) -> Option<serde_json::Value> {
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = req.get("id").cloned();
    let has_id = id.is_some();

    let response = match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "result": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "blog-autopost-rs",
                    "version": "0.1.0"
                }
            },
            "id": id
        }),
        "initialized" => return None,
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "result": {
                "tools": tools::tool_definitions()
            },
            "id": id
        }),
        "tools/call" => {
            let params = req.get("params");
            let name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            match tools::handle_tool_call(state, name, arguments).await {
                Ok(res_val) => json!({
                    "jsonrpc": "2.0",
                    "result": {
                        "content": [
                            {
                                "type": "text",
                                "text": res_val
                            }
                        ]
                    },
                    "id": id
                }),
                Err(e) => json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32603,
                        "message": format!("Tool execution error: {:?}", e)
                    },
                    "id": id
                }),
            }
        }
        _ => json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32601,
                "message": format!("Method not found: {}", method)
            },
            "id": id
        }),
    };

    if has_id { Some(response) } else { None }
}
