# Implementation Plan

親Issue: #92

各タスクは1PR = 1子Issueに対応する。完了条件は `requirements.md` のEARS要件を参照。

## Phase 0: spec (#93)

- [ ] 0.1 `.kiro/specs/mcp-server-enhancement/` に requirements.md / design.md / tasks.md / spec.json を作成
  - ブランチ: `docs/spec-mcp-server-enhancement`

## Phase 1: MCPモジュール分割 (#94)

振る舞い不変。移動のみに徹し機能変更を混ぜない。

- [ ] 1.1 `src/web/mcp/mod.rs` を作成し、`mcp_sse_handler` / `mcp_message_handler` / `SessionCleanupStream` を移動
- [ ] 1.2 `src/web/mcp/protocol.rs` を作成し、`build_mcp_response()` を抽出。`dispatch_mcp_response()` をSSE送出担当として分離
- [ ] 1.3 `src/web/mcp/tools.rs` を作成し、tool定義と `handle_tool_call` を移動
- [ ] 1.4 `src/web/mod.rs` のルート登録をパス変更に追従
- [ ] 1.5 既存テストがimportパス変更のみで通ることを確認
  - ブランチ: `refactor/web-mcp-module-split`

## Phase 2: テスト整備 (#95)

- [ ] 2.1 `build_mcp_response` の単体テスト
  - initialize のフィールド、initialized が `None`、tools/list の全name と `inputSchema.required`
  - 未知メソッド `-32601`、未知tool名 `-32603`、id が数値/文字列/null のエコーバック
- [ ] 2.2 スケジュール系 tool のテスト
  - `list_schedules`: 空、statusフィルタ、時刻ソート順
  - `add_schedule`: 3つの日時形式、`auto_slot: true`、`at` も `auto_slot` もない場合のエラー、`sns` カンマ区切り
  - `update_schedule`: not found、部分更新、`updated_at` が進むこと
  - `delete_schedule`: 成功と not found
- [ ] 2.3 `post_now` のテスト
  - wiremock の `MockServer::uri()` を `instance_url` に注入。成功/失敗/一部失敗
- [ ] 2.4 SSE統合テスト
  - `event: endpoint` と session_id、`mcp_sessions` 登録、POST後のレスポンスフレーム
  - `tokio::time::timeout` で囲む。drop確認はリトライループ
- [ ] 2.5 `just cov-check` で実測し `coverage-threshold.txt` を引き上げ
  - ブランチ: `test/mcp-protocol-coverage`

## Phase 3: セッションセキュリティ (#96)

- [ ] 3.1 `Cargo.toml` に `getrandom = "0.3"` と `subtle = "2"` を追加
- [ ] 3.2 `Session` 構造体を定義し `AppState.sessions` を `RwLock<HashMap<String, Session>>` へ変更
- [ ] 3.3 セッションID生成を `getrandom::fill` ベースへ置換 (256bit、タイムスタンプを含めない)
- [ ] 3.4 `WebAuthConfig` に `session_ttl_hours` / `cookie_secure` を追加 (`skip_serializing_if` 必須)
- [ ] 3.5 ミドルウェアでの期限切れセッションのlazy削除、`login_submit` での全走査purge
- [ ] 3.6 Cookie に `Max-Age` と条件付き `Secure` を付与。`X-Forwarded-Proto` 判定
- [ ] 3.7 `constant_time_eq` ヘルパを作り Bearer / X-Api-Key の比較を置換
- [ ] 3.8 `api_routes` 側の `from_fn_with_state` を削除し、ミドルウェア適用を1箇所に集約
  - ブランチ: `feat/session-security`

## Phase 4: メディアパス検証 (#99)

Phase 3 と並行可能。

- [ ] 4.1 `AppState.upload_dir: PathBuf` を導入し、`upload_media` と `add_schedule` の `data/uploads` ハードコードを置換
- [ ] 4.2 `validate_media_path()` を実装 (canonicalize → 許可ディレクトリ判定 → ファイル判定 → マジックバイト → サイズ)
- [ ] 4.3 `add_schedule` / `post_now` のmedia処理を `validate_media_path` 経由へ
- [ ] 4.4 検証テスト (正常 / 許可外 / `..` / シンボリックリンク / 非画像 / サイズ超過)
  - ブランチ: `fix/mcp-media-path-validation`

## Phase 5: ログイン保護 (#97)

Phase 3 に依存。

- [ ] 5.1 `LoginRateLimiter` を実装 (`tokio::time::Instant` のスライディングウィンドウ)
- [ ] 5.2 `WebAuthConfig` に `login_max_attempts` / `login_window_seconds` を追加
- [ ] 5.3 `axum::serve` を `into_make_service_with_connect_info::<SocketAddr>()` へ変更
- [ ] 5.4 `login_submit` を `Option<ConnectInfo<SocketAddr>>` で受け、鍵はIP、なければusername
- [ ] 5.5 超過時に `429` + `Retry-After`、成功時にカウンタリセット
- [ ] 5.6 `/logout` を POST 化し、`GET /logout` を削除
- [ ] 5.7 `static/index.html:50` のログアウトリンクをフォームPOSTへ置換
- [ ] 5.8 テスト (`tokio::time::pause()` / `advance()` で窓の開閉を検証)
  - ブランチ: `feat/login-ratelimit-and-post-logout`

## Phase 6: MCP専用APIキー (#98)

Phase 3 に依存。Phase 5 と並行可能。

- [ ] 6.1 `McpConfig` を定義し `Config.mcp` を追加 (全フィールドに `skip_serializing_if` 必須)
- [ ] 6.2 `auth_middleware` に `/api/mcp/` 分岐を追加。Bearer / X-Api-Key のみ、Cookieセッション不可、失敗は常に401
- [ ] 6.3 2モード判定 (レガシー互換 / 分離) を実装
- [ ] 6.4 起動時ログ (認証モード、専用キー推奨の警告、キー重複の警告)
- [ ] 6.5 `config.yml.template` に `mcp` 節を追加
- [ ] 6.6 テスト (専用キー200 / secret_key 401 / レガシー互換 / Cookie 401)
- [ ] 6.7 「未設定の新フィールドが書き戻しで増えない」回帰テスト
  - ブランチ: `feat/mcp-api-key`

## Phase 7: MCP toolギャップ (#100)

Phase 4 に依存。

- [ ] 7.1 `build_selected_clients()` を `src/sns/mod.rs` に実装 (第2戻り値に未対応種別)
- [ ] 7.2 `manual_post` / `post_now` を `build_selected_clients` へ移行
- [ ] 7.3 未対応SNS指定時に明示エラーを返す。`add_schedule` のSNS名列挙も揃える
- [ ] 7.4 `collect_next_slots()` を抽出し、HTTPハンドラと共用
- [ ] 7.5 `get_next_slots` tool を追加
- [ ] 7.6 `sensitive` を inputSchema に追加し、`add_schedule` / `post_now` で値を渡す
- [ ] 7.7 テスト
  - ブランチ: `feat/mcp-tool-gaps`

## Phase 8: ドキュメント (#101)

- [ ] 8.1 `docs/mcp.md` を新規作成 (エンドポイント、tool一覧、キー生成、`mcpServers` 設定例)
- [ ] 8.2 `README.md` に MCP 節を新設。`secret_key` 説明に別キー推奨を追記。破壊的変更を明記
- [ ] 8.3 `config.yml.template` の `web.session.secret_key` デッド設定を整理。Tumblrフィールド名不整合を修正
- [ ] 8.4 `.kiro/steering/product.md` / `structure.md` を更新
- [ ] 8.5 `spec.json` を完了状態へ。`CLAUDE.md` の Active Specifications 表に追記
  - ブランチ: `docs/mcp-guide`

## 各PR共通の検証

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
just test
just cov-check
```
