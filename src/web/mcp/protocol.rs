//! MCP の JSON-RPC プロトコル処理。
//!
//! トランスポート (SSE) から切り離してあり、[`build_mcp_response`] は
//! リクエストの JSON を受けてレスポンスの JSON を返すだけの純粋な処理である。
//! 送出は [`super::dispatch_mcp_response`] が担当する。

use serde_json::json;
use std::sync::Arc;

use super::tools;
use crate::web::AppState;

/// このサーバが受け付ける MCP のプロトコルバージョン。新しい順に並べる。
///
/// `2024-11-05` は HTTP+SSE トランスポートの版で、後方互換のために残している。
pub(crate) const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] =
    ["2025-06-18", "2025-03-26", "2024-11-05"];

/// 対応バージョンのうち最も新しいもの。
pub(crate) const LATEST_PROTOCOL_VERSION: &str = SUPPORTED_PROTOCOL_VERSIONS[0];

/// 指定されたバージョンを受け付けられるかを返す。
///
/// `MCP-Protocol-Version` ヘッダが無い場合、仕様は `2025-03-26` を
/// 想定するよう定めている。このサーバは版によって応答を変えないため、
/// ヘッダの有無で挙動は変わらない。
pub(crate) fn is_supported_protocol_version(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

/// クライアントの要求に応じて返すプロトコルバージョンを決める。
///
/// 要求された版に対応していればそれを返す。対応していない、または
/// 指定が無い場合は、こちらが対応する最新版を返して交渉に委ねる。
fn negotiate_protocol_version(requested: Option<&str>) -> &'static str {
    match requested {
        Some(v) => SUPPORTED_PROTOCOL_VERSIONS
            .into_iter()
            .find(|s| *s == v)
            .unwrap_or(LATEST_PROTOCOL_VERSION),
        None => LATEST_PROTOCOL_VERSION,
    }
}

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
        "initialize" => {
            // クライアントが要求した版に合わせて返す。合わせないと
            // 新しい版のクライアントが古い版を掴んだまま動いてしまう
            let requested = req
                .get("params")
                .and_then(|p| p.get("protocolVersion"))
                .and_then(|v| v.as_str());

            json!({
                "jsonrpc": "2.0",
                "result": {
                    "protocolVersion": negotiate_protocol_version(requested),
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "blog-autopost-rs",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                },
                "id": id
            })
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::tests::setup_test_app;

    /// テスト用のアプリ状態を作る。
    ///
    /// `TestApp` を丸ごと返すのは `TempDir` を生かしておくためで、
    /// 先に落とすと予約投稿の保存先が消えてしまう。
    fn app() -> crate::web::tests::TestApp {
        setup_test_app(Some("test-secret-token".to_string()))
    }

    /// JSON-RPC リクエストを組み立てる。
    fn rpc(method: &str, id: serde_json::Value) -> serde_json::Value {
        json!({ "jsonrpc": "2.0", "method": method, "id": id })
    }

    // --- initialize ---

    /// initialize はプロトコルバージョンとサーバ情報を返す。
    #[tokio::test]
    async fn initialize_はサーバ情報を返す() {
        let app = app();
        let res = build_mcp_response(app.state.clone(), rpc("initialize", json!(1)))
            .await
            .expect("id付きなのでレスポンスが返るはず");

        assert_eq!(res["jsonrpc"], "2.0");
        assert_eq!(res["id"], 1);
        // 版の指定が無ければ対応する最新版を返す
        assert_eq!(res["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
        assert_eq!(res["result"]["serverInfo"]["name"], "blog-autopost-rs");
        assert_eq!(
            res["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
        // tools ケイパビリティを宣言していないとクライアントが tools/list を呼ばない
        assert!(res["result"]["capabilities"]["tools"].is_object());
    }

    // --- プロトコルバージョンの交渉 ---

    /// 要求された版に対応していればそれを返す。
    #[tokio::test]
    async fn 要求された版に対応していればそれを返す() {
        let app = app();

        for requested in SUPPORTED_PROTOCOL_VERSIONS {
            let req = json!({
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": { "protocolVersion": requested },
                "id": 1
            });
            let res = build_mcp_response(app.state.clone(), req)
                .await
                .expect("レスポンスが返るはず");

            assert_eq!(
                res["result"]["protocolVersion"], requested,
                "要求: {}",
                requested
            );
        }
    }

    /// 対応していない版を要求されたら最新版を返して交渉に委ねる。
    #[tokio::test]
    async fn 未対応の版を要求されたら最新版を返す() {
        let app = app();
        let req = json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": { "protocolVersion": "1999-01-01" },
            "id": 1
        });

        let res = build_mcp_response(app.state.clone(), req)
            .await
            .expect("レスポンスが返るはず");

        assert_eq!(res["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);
    }

    #[test]
    fn 対応バージョンを判定できる() {
        assert!(is_supported_protocol_version("2024-11-05"));
        assert!(is_supported_protocol_version("2025-03-26"));
        assert!(is_supported_protocol_version("2025-06-18"));

        assert!(!is_supported_protocol_version("1999-01-01"));
        assert!(!is_supported_protocol_version(""));
    }

    #[test]
    fn 最新版と仕様の既定版は対応範囲に含まれる() {
        assert!(is_supported_protocol_version(LATEST_PROTOCOL_VERSION));
        // ヘッダが無いとき仕様が想定する版も受け付けられる必要がある
        assert!(is_supported_protocol_version("2025-03-26"));
        // 旧トランスポートの版も残す
        assert!(is_supported_protocol_version("2024-11-05"));
    }

    // --- initialized (通知) ---

    /// initialized は通知なのでレスポンスを返さない。
    #[tokio::test]
    async fn initialized_はレスポンスを返さない() {
        let app = app();
        let req = json!({ "jsonrpc": "2.0", "method": "initialized" });

        assert!(build_mcp_response(app.state.clone(), req).await.is_none());
    }

    /// id を持たないリクエストは通知とみなし、レスポンスを返さない。
    #[tokio::test]
    async fn id無しのリクエストはレスポンスを返さない() {
        let app = app();
        let req = json!({ "jsonrpc": "2.0", "method": "tools/list" });

        assert!(build_mcp_response(app.state.clone(), req).await.is_none());
    }

    // --- tools/list ---

    /// tools/list はレジストリの全 tool を返す。
    #[tokio::test]
    async fn tools_listはレジストリの全toolを返す() {
        let app = app();
        let res = build_mcp_response(app.state.clone(), rpc("tools/list", json!(2)))
            .await
            .expect("レスポンスが返るはず");

        let tools = res["result"]["tools"].as_array().expect("tools は配列");
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

        // レジストリ側と突き合わせ、追加漏れ・削除漏れを検出する
        let expected: Vec<String> = super::tools::tool_definitions()
            .iter()
            .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
            .collect();
        assert_eq!(names, expected);

        // 現行の公開 tool
        assert_eq!(
            names,
            vec![
                "list_schedules",
                "add_schedule",
                "update_schedule",
                "delete_schedule",
                "post_now",
                "get_next_slots",
            ]
        );
    }

    /// 各 tool は inputSchema を持ち、必須項目が宣言されている。
    #[tokio::test]
    async fn 各toolのinput_schemaが必須項目を宣言している() {
        let app = app();
        let res = build_mcp_response(app.state.clone(), rpc("tools/list", json!(3)))
            .await
            .expect("レスポンスが返るはず");
        let tools = res["result"]["tools"].as_array().expect("tools は配列");

        let required_of = |name: &str| -> Vec<String> {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .and_then(|t| t["inputSchema"]["required"].as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default()
        };

        assert_eq!(required_of("add_schedule"), vec!["text"]);
        assert_eq!(required_of("post_now"), vec!["text"]);
        assert_eq!(required_of("update_schedule"), vec!["id"]);
        assert_eq!(required_of("delete_schedule"), vec!["id"]);
        // 絞り込み条件が任意の tool は required を持たない
        assert!(required_of("list_schedules").is_empty());
        assert!(required_of("get_next_slots").is_empty());

        // sensitive は投稿系の tool で受け付ける
        let has_property = |name: &str, prop: &str| -> bool {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .is_some_and(|t| t["inputSchema"]["properties"][prop].is_object())
        };
        assert!(has_property("add_schedule", "sensitive"));
        assert!(has_property("post_now", "sensitive"));

        for t in tools {
            assert!(
                t["description"].as_str().is_some_and(|d| !d.is_empty()),
                "description が必要: {}",
                t["name"]
            );
            assert_eq!(t["inputSchema"]["type"], "object", "tool: {}", t["name"]);
        }
    }

    // --- エラー ---

    /// 未知のメソッドは -32601 (Method not found) を返す。
    #[tokio::test]
    async fn 未知のメソッドは32601を返す() {
        let app = app();
        let res = build_mcp_response(app.state.clone(), rpc("resources/list", json!(4)))
            .await
            .expect("レスポンスが返るはず");

        assert_eq!(res["error"]["code"], -32601);
        assert!(
            res["error"]["message"]
                .as_str()
                .expect("メッセージがあるはず")
                .contains("resources/list"),
            "メソッド名がメッセージに含まれること"
        );
        assert!(res.get("result").is_none());
    }

    /// 未知の tool 名は -32603 (Internal error) を返す。
    #[tokio::test]
    async fn 未知のtool名は32603を返す() {
        let app = app();
        let req = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": { "name": "no_such_tool", "arguments": {} },
            "id": 5
        });

        let res = build_mcp_response(app.state.clone(), req)
            .await
            .expect("レスポンスが返るはず");

        assert_eq!(res["error"]["code"], -32603);
        assert!(
            res["error"]["message"]
                .as_str()
                .expect("メッセージがあるはず")
                .contains("no_such_tool")
        );
    }

    /// params を省略した tools/call も落ちずにエラーとして返る。
    #[tokio::test]
    async fn params無しのtools_callはエラーになる() {
        let app = app();
        let req = json!({ "jsonrpc": "2.0", "method": "tools/call", "id": 6 });

        let res = build_mcp_response(app.state.clone(), req)
            .await
            .expect("レスポンスが返るはず");

        assert_eq!(res["error"]["code"], -32603);
    }

    // --- tools/call の成功形 ---

    /// tool の実行結果は content 配列のテキストとして返る。
    #[tokio::test]
    async fn tools_callの結果はcontentテキストで返る() {
        let app = app();
        let req = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": { "name": "list_schedules", "arguments": {} },
            "id": 7
        });

        let res = build_mcp_response(app.state.clone(), req)
            .await
            .expect("レスポンスが返るはず");

        assert_eq!(res["id"], 7);
        assert_eq!(res["result"]["content"][0]["type"], "text");
        let text = res["result"]["content"][0]["text"]
            .as_str()
            .expect("テキストが入る");
        assert!(text.contains("予約投稿一覧"));
    }

    // --- id のエコーバック ---

    /// id は型を保ったまま返す (数値・文字列・null)。
    #[tokio::test]
    async fn idは型を保ってエコーバックされる() {
        let app = app();

        let res = build_mcp_response(app.state.clone(), rpc("initialize", json!("abc")))
            .await
            .expect("文字列idでもレスポンスが返る");
        assert_eq!(res["id"], "abc");

        let res = build_mcp_response(app.state.clone(), rpc("initialize", json!(42)))
            .await
            .expect("数値idでもレスポンスが返る");
        assert_eq!(res["id"], 42);

        // JSON-RPC 上 id:null は「値を持つ」ので通知とは区別される
        let res = build_mcp_response(app.state.clone(), rpc("initialize", json!(null)))
            .await
            .expect("null idでもレスポンスが返る");
        assert!(res["id"].is_null());
    }
}
