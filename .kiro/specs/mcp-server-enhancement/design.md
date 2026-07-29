# Design Document

## 現状のアーキテクチャ

| 項目 | 場所 |
|---|---|
| MCPルート登録 | `src/web/mod.rs:50-51` |
| JSON-RPC処理 (手書き、rmcp不使用) | `src/web/routes.rs:997-1182` |
| tool実装 (5個) | `src/web/routes.rs:1184-1597` |
| 認証ミドルウェア | `src/web/mod.rs:132-198` |
| セッションID生成 | `src/web/routes.rs:852-857` |
| Cookie属性 | `src/web/routes.rs:864` |

トランスポートは HTTP+SSE (2024-11-05仕様)。`GET /api/mcp/sse` で `event: endpoint` を返し、クライアントは `POST /api/mcp/message?session_id=...` へJSON-RPCを送る。レスポンスはSSE側へ流れる。

公開tool: `list_schedules` / `add_schedule` / `update_schedule` / `delete_schedule` / `post_now`

## 設計判断と根拠

### 依存追加は `getrandom` と `subtle` の2つのみ

どちらも `Cargo.lock` に既に入っている (`getrandom` は bcrypt 経由で 0.3.4、`subtle` は ring 経由)。直接依存への昇格で済み、依存グラフは増えない。

`rand` のフル依存は不要。必要なのは `getrandom::fill(&mut [u8; 32])` の1関数だけであり、hex化は `format!("{:02x}")` の数行で足りる。

### レート制限に `tower-governor` を使わない

`tower-governor` は `governor` + `dashmap` + `quanta` を引き込み、依存が重い。一方、要件は「`POST /login` だけ、単一プロセス、インメモリ」なので `HashMap<Key, Vec<Instant>>` のスライディングウィンドウで足りる。

決め手は `tokio::time::Instant` を使えること。dev-dependency に設定済みの `tokio/test-util` の `pause()` / `advance()` により、テストから時間を進めて窓の開閉を検証できる。実時間のsleepに頼らないテストが書ける。

```rust
/// ログイン試行のレート制限器。窓内の試行回数を鍵ごとに数える。
pub struct LoginRateLimiter {
    attempts: tokio::sync::Mutex<HashMap<String, Vec<tokio::time::Instant>>>,
    max_attempts: usize,
    window: std::time::Duration,
}
```

### レート制限の鍵は IP、フォールバックは username

現状 `src/web/mod.rs:110` の `axum::serve(listener, app)` では `ConnectInfo<SocketAddr>` が使えない。`app.into_make_service_with_connect_info::<SocketAddr>()` へ変更する。

ハンドラは `Option<ConnectInfo<SocketAddr>>` で受ける。テストの `oneshot` では `None` になるため、`Option` にしないとテストが 500 になる。またこの設計により、配線漏れがあっても「1人の失敗が全ユーザーをロックする」事故を防げる。

`X-Forwarded-For` は採用しない。偽装で制限を回避できるため。プロキシ配下でのIP単位制限が必要になったら別途検討する。

### MCPキーは `WebAuthConfig` ではなく独立した `McpConfig` に置く

理由は3つある。

1. `WebAuthConfig` はログイン時のbcrypt移行で `config.yml` へ丸ごと書き戻される (`src/web/routes.rs:831-850`)。同じ構造体にMCPキーを置くと、Webログインの副作用でMCP設定が書き換わる経路ができてしまう
2. 「漏洩時の影響範囲を限定する」という目的上、設定ファイル上でも節を分けたほうが意図が伝わる
3. 将来MCP側だけの設定 (許可メディアディレクトリ、公開toolの絞り込み) が増える見込みがある

```rust
/// MCP サーバ機能の設定。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct McpConfig {
    /// MCP エンドポイントを有効にするか。未指定時は true。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// MCP 専用 API キー。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// web_auth.secret_key でも MCP を認証できるようにするか。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_web_secret_key: Option<bool>,
    /// MCP tool がメディアとして参照できるディレクトリ。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_media_dirs: Option<Vec<String>>,
}
```

すべてのフィールドに `skip_serializing_if = "Option::is_none"` を付けるのは互換性上の必須条件である。これがないと、bcrypt移行時の書き戻しで既存ユーザーの `config.yml` に `mcp: null` などが挿入される。

### MCP認証は「`mcp` 節の有無」で2モードに分ける

| `config.yml` の状態 | `/api/mcp/*` の認証 |
|---|---|
| `mcp:` 節なし | `web_auth.secret_key` で通る (レガシー互換)。起動時に専用キー推奨の警告を1回 |
| `mcp.api_key` あり | 専用キーのみ。`secret_key` は不可 |
| `mcp.api_key` + `allow_web_secret_key: true` | 両方通る (移行期間用の明示オプトイン) |
| `mcp.enabled: false` | 常に 401 |
| `mcp:` 節あり・`api_key` なし | 401 (設定不備として明示ログ) |

既定を必ずレガシー互換側にする。分離モードを既定にすると、アップグレードした瞬間に既存のMCP接続が全部 401 になる。

「節を書いた = 意図的に設定した」とみなすことで、既存ユーザーを壊さずセキュアな既定を両立できる。

なお `Config` に `#[serde(flatten)] extra` があるため、フィールド名のタイポは黙って `extra` に吸われエラーにならない。起動時に「MCP認証: 専用キー / レガシー互換」を1行ログに出し、設定が効いているか確認できるようにする。

### MCPパスではCookieセッションを受け付けない

`/api/mcp/*` は Bearer / X-Api-Key のみ受け付け、失敗時は常に 401 を返す (リダイレクトしない)。

ブラウザに乗ったセッションでCSRF的にMCP tool (= 即時SNS投稿) を叩ける経路を塞ぐのが狙い。`CorsLayer::permissive()` (`src/web/mod.rs:36`) が入っている以上、この遮断は実質的な防御になる。

### Cookieの `Secure` は3値設定で既定は自動判定

```yaml
web_auth:
  cookie_secure: auto   # auto (既定) | always | never
```

- `auto`: `X-Forwarded-Proto: https` またはリクエストのschemeで HTTPS 由来と判定できたときだけ付与
- `always`: 常に付与 (TLS終端が確実な環境)
- `never`: 付与しない (明示的なエスケープハッチ)

`auto` はリバースプロキシ配下 (この構成が主流) で正しく効き、素のHTTP運用を壊さない。未知の値は `auto` にフォールバックして警告ログを出す。

`SameSite` は `Lax` を維持する。`/logout` を同一サイトのフォームPOSTにするため、`Lax` でもCookieは送信される。`Strict` にすると外部リンクからの遷移でログイン済み表示にならずUXが落ちる。

### セッションTTLはスライディング延長しない

```rust
/// ログイン済みセッションの1件分。
pub struct Session {
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
```

`AppState.sessions` を `RwLock<HashMap<String, Session>>` へ変更する。

アクセスのたびに `expires_at` を伸ばすスライディング延長は入れない。全リクエストで書き込みロックが必要になり、絶対期限のほうが実装もテストも単純になる。

掃除は2段構え。ミドルウェアでの当該キーのlazy削除と、`login_submit` 時の全走査purge。単一ユーザー運用でエントリ数は微小なので、バックグラウンドタスクは不要。

### MCP実装を `src/web/mcp/` へ分割する

`src/web/routes.rs` は 2690行。MCP部分 (939-1597の約660行) を切り出す。

```
src/web/mcp/mod.rs        SSE/message ハンドラ、SessionCleanupStream
src/web/mcp/protocol.rs   build_mcp_response() / initialize / tools/list
src/web/mcp/tools.rs      tool 定義と dispatch、各 tool 実装
```

分割の主目的はテスタビリティである。現状 `mcp_message_handler` (`src/web/routes.rs:977-995`) が `tokio::spawn` して 202 を返すため、レスポンス本体を検証できない。

```rust
/// JSON-RPC リクエストを処理し、クライアントへ返すべきレスポンスを返す。
/// 通知 (id なし) の場合は None を返す。SSE への送出は行わない。
pub(crate) async fn build_mcp_response(
    state: Arc<AppState>,
    req: serde_json::Value,
) -> Option<serde_json::Value>;

/// build_mcp_response の結果を、指定セッションの SSE ストリームへ送出する。
async fn dispatch_mcp_response(state: Arc<AppState>, session_id: &str, req: Value) -> Result<()>;
```

これでテストが `build_mcp_response` を直接awaitしてJSONをアサートでき、`tokio::spawn` 越しのflakyな検証を避けられる。将来 Streamable HTTP トランスポート (POSTのレスポンスボディにJSON-RPC応答を直接返す形式) へ進む場合も、この分離がそのまま土台になる。

分割PRは移動のみに徹し、機能変更を混ぜない。`git log --follow` を効かせるためである。

### メディアパス検証

```rust
/// MCP から渡されたメディアパスを検証し、正規化された絶対パスを返す。
///
/// 許可ディレクトリ配下にあること、対応する画像/動画形式であること、
/// サイズ上限内であることを確認する。シンボリックリンクは解決後に再判定する。
pub(crate) fn validate_media_path(
    allowed_dirs: &[PathBuf],
    input: &str,
    max_bytes: u64,
) -> anyhow::Result<PathBuf>;
```

検証順序:

1. `std::fs::canonicalize` でシンボリックリンクと `..` を解決
2. 許可ディレクトリも canonicalize した上で `starts_with` 判定
3. ファイルであること (ディレクトリ・FIFO等を弾く)
4. マジックバイト検査。`src/sns/mod.rs` の `is_supported_image` を再利用する
5. サイズ 10MB 以下 (`upload_media` の `max_size` と同値)

バイパス設定 (`allow_any_media_path` 等) は用意しない。セキュリティ修正の意味が失われるため。任意パスを渡していた既存ユーザーは `mcp.allowed_media_dirs` への追加で対応する。

あわせて `AppState.upload_dir: PathBuf` を導入する。現状 `data/uploads` が `src/web/routes.rs:519` (`upload_media`) と `src/web/routes.rs:1265` (`add_schedule`) にハードコードされており、「テストは `data/` を汚さない」という規約を守れない。本番は `data/uploads`、テストは `TempDir` 配下とする。

### SNSクライアント構築の共通化

同じ処理が3箇所に重複している。

- `manual_post` (`src/web/routes.rs:176-231`)
- `post_now` (`src/web/routes.rs:1440-1540`) — 別実装
- `sns::build_clients_from_config` (`src/sns/mod.rs:57-127`) — `Arc` 版

`src/sns/mod.rs` に集約する。

```rust
/// 設定から SNS クライアント群を構築する。
/// `targets` が Some のとき、アカウント名・表示ラベル・種別名のいずれかで絞り込む。
/// 戻り値の第2要素は、設定にはあるが投稿クライアント未実装の種別名。
pub fn build_selected_clients(
    config: &Config,
    targets: Option<&[String]>,
) -> (Vec<Arc<dyn SnsClient + Send + Sync>>, Vec<String>);
```

第2戻り値が本設計の要点である。`ThreadsClient` / `TumblrClient` はそもそも存在せず (`src/sns/mod.rs:53` にスキップ処理を明記)、`post_now` のmatchを広げても投稿先がない。第2戻り値により「Threadsを指定したのに黙って無視された」ではなく「threads-main は現在の実装では投稿に対応していません」と明示できる。

なお `add_schedule` のSNS名列挙 (`src/web/routes.rs:1249-1255`) は Threads/Tumblr を含んでおり、`post_now` との間で不整合がある。ここも揃える。

### `sensitive` フラグは配管済み

`PostContent.sensitive` (`src/sns/models.rs:11`)、`ScheduledPost.sensitive` (`src/scheduled/models.rs:17`)、`executor.rs:74` の受け渡しはすでに実装されている。実効はMisskeyのみ (`src/sns/misskey.rs:48`)。

必要なのはMCP側で値を渡すことだけである。現状 `add_schedule` はセットしておらず、`post_now` は `src/web/routes.rs:1560` で `false` 固定になっている。どちらもバグ扱いで修正する。

## 実装順序

```
PR0 (spec) → PR1 (分割) → PR2 (テスト) → PR3 (セッション) ─┬→ PR4 (レート制限) ─┐
                                        └→ PR6 (メディア) ─┼→ PR5 (MCPキー) ────┼→ PR8 (docs)
                                                            └→ PR7 (toolギャップ) ─┘
```

PR1 / PR2 を先頭に置く理由は2つある。

1. 現状MCPのテストは「202が返る」しか見ておらず (`src/web/routes.rs:2468-2518`)、以降の全変更がノーガードである。先に特性テストで既存挙動を固定しないと、TDDのRedが「壊したのか元からか」を判別できない
2. カバレッジ閾値 (`coverage-threshold.txt` = 80、引き下げ禁止) の観点でも、機能追加の前にテストを積む順序が必須になる。逆順にすると機能追加の時点で閾値を割り、閾値を下げられないためPRがマージできなくなる

## リスク

### 既存ユーザーの config.yml 互換性

最重要のリスクである。bcrypt移行時の全書き戻し (`src/web/routes.rs:831-850`) があるため、新規フィールドすべてに `skip_serializing_if = "Option::is_none"` を付ける。「未設定の新フィールドが書き戻しで増えない」ことを検証する回帰テストをPR5に含める。

`WebAuthConfig` の `username` / `password` は非Optionである。ここに非Optionのフィールドを足さない。

### 運用事故

- Secure Cookie による HTTP 運用ユーザーのログイン不能 → `auto` 既定と `X-Forwarded-Proto` 判定で回避
- レート制限の `ConnectInfo` 未配線で全ユーザー共通鍵になる → `Option<ConnectInfo>` の username フォールバックで防ぐ

### テストの非決定性

SSEテストは keep-alive で無限ストリームになるため、必ず `tokio::time::timeout` で上限を切る。`SessionCleanupStream::drop` は内部で `tokio::spawn` するため、削除確認は固定sleepではなくリトライループで行う。
