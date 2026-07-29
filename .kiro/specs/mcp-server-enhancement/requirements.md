# Requirements Document

## Project Description (Input)

「APIによるポスト機能(即時、時間指定、「次のタイミング」)を用意し、MCPからアクセスできる形を追加できないでしょうか。この場合、何らかの認証がないと無関係なところからもやられそうなので注意が必要ですが。」

## 調査結果と本specの位置づけ

調査の結果、MCPサーバ機能はすでに実装済みであり、要望された3種類の投稿はいずれもMCP経由で実行可能であることが判明した。

| 要望 | 対応するMCP tool |
|---|---|
| 即時投稿 | `post_now` |
| 時間指定予約 | `add_schedule` の `at` 引数 |
| 次のタイミング | `add_schedule` の `auto_slot: true` |

認証も `auth_middleware` により `/api/mcp/*` を保護済みである。

一方で、提案者が懸念したとおり認証面には実際の脆弱性が複数存在する。加えて機能ギャップ、テスト不足、ドキュメント皆無という課題がある。本specはそれらの解消を目的とする。

## 用語

- **MCP**: Model Context Protocol。AIクライアントから外部ツールを呼び出すためのプロトコル
- **tool**: MCPが公開する呼び出し可能な機能の単位
- **次のタイミング**: `SlotFinder` が算出する、各SNSの次に投稿可能な時間枠
- **レガシー互換モード**: `config.yml` に `mcp` 節がない既存ユーザー向けの動作モード

## Requirements

### 1. セッションセキュリティ (#96)

#### 1.1 セッションIDの予測不可能性

現状のセッションIDは `chrono::Utc::now().timestamp_nanos_opt()` を `DefaultHasher` に通しただけで、エントロピーは実質タイムスタンプのみである。

- When a user logs in successfully, the system shall generate a session id from a CSPRNG with at least 256 bits of entropy.
- The system shall not include a timestamp in the generated session id.

#### 1.2 セッションの有効期限

現状はサーバ側にTTLがなく、プロセスが生きている限りセッションが永続する。

- While a session's `expires_at` is in the past, the system shall reject requests presenting that session id.
- When a login succeeds, the system shall remove all expired sessions from the session store.
- Where `web_auth.session_ttl_hours` is unset, the system shall use 24 hours as the default TTL.

#### 1.3 Cookie属性

無条件に `Secure` を付けると、素のHTTPで運用しているユーザーがログイン不能になる。

- Where the request is determined to originate over HTTPS, or `web_auth.cookie_secure` is `always`, the system shall include the `Secure` attribute in the session cookie.
- Where `web_auth.cookie_secure` is unset and the request is plain HTTP, the system shall not include the `Secure` attribute.
- The system shall include a `Max-Age` attribute equal to the configured session TTL.
- The system shall retain the `HttpOnly` and `SameSite=Lax` attributes.

#### 1.4 定時間比較

- When comparing a presented API key with the configured key, the system shall use a constant-time comparison.

#### 1.5 ミドルウェアの適用回数

現状 `auth_middleware` が `api_routes` と外側Routerの両方に適用されており、`/api/*` では2回走る。

- The system shall apply `auth_middleware` exactly once per request.

### 2. ログイン保護 (#97)

#### 2.1 レート制限

現状 `POST /login` にレート制限がなく、`/login` は public path なので総当たりが無制限に可能である。

- When more than `web_auth.login_max_attempts` failed login attempts occur within `web_auth.login_window_seconds` for the same key, the system shall respond `429 Too Many Requests` with a `Retry-After` header.
- When a login succeeds, the system shall reset the failure counter for that key.
- Where `login_max_attempts` and `login_window_seconds` are unset, the system shall use 5 attempts and 300 seconds as defaults.
- Where the client IP address is unavailable, the system shall fall back to keying the counter by username.
- The system shall not use the `X-Forwarded-For` header as the rate limit key.

#### 2.2 ログアウトの安全化

現状 `/logout` はGETで状態変更するため、`<img src="/logout">` 相当で強制ログアウトさせられる。`SameSite=Lax` はtop-level GETを通すため防げない。

- When `POST /logout` is received with a valid session cookie, the system shall remove the session and shall respond `303 See Other` to `/login` with an expired cookie.
- The system shall not expose `GET /logout`.

### 3. MCP専用APIキーの分離 (#98)

現状 MCP と Web UI が同じ `web_auth.secret_key` を共用しており、漏洩時の影響範囲が広い。

- Where `mcp.api_key` is configured, the system shall accept requests to `/api/mcp/*` only when the presented key matches `mcp.api_key`, unless `mcp.allow_web_secret_key` is `true`.
- Where the `mcp` section is absent from the configuration, the system shall continue to accept `web_auth.secret_key` for `/api/mcp/*` and shall log a warning recommending a dedicated key.
- Where `mcp.enabled` is `false`, the system shall respond `401 Unauthorized` to all `/api/mcp/*` requests.
- Where the `mcp` section is present but `api_key` is unset and `enabled` is unspecified, the system shall respond `401 Unauthorized` and shall log the configuration problem.
- The system shall not authenticate `/api/mcp/*` requests using the browser session cookie.
- When `mcp.api_key` equals `web_auth.secret_key`, the system shall log a warning at startup.
- When an existing `config.yml` without an `mcp` section is loaded, the system shall parse it without error.
- When the configuration is written back during password migration, the system shall not add fields that were absent from the original file.

### 4. メディアパス検証 (#99)

現状 MCP tool の `media` 引数はパス検証がなく、認証済みクライアントが任意のファイルをSNSへ送信できる。

- When a media path outside the allowed directories is supplied to any MCP tool, the system shall reject the request with an error and shall not copy or transmit the file.
- When a symbolic link resolving outside the allowed directories is supplied, the system shall reject the request.
- When a file whose magic bytes do not match a supported image or video format is supplied, the system shall reject the request.
- When a file larger than 10 MB is supplied, the system shall reject the request.
- Where `mcp.allowed_media_dirs` is unset, the system shall permit only `data/uploads` and `data`.
- The system shall write uploaded media only under `AppState.upload_dir`.
- While tests run, `AppState.upload_dir` shall be a temporary directory.

### 5. MCP toolの機能ギャップ (#100)

- When `get_next_slots` is called, the system shall return the next available slot for each configured SNS, matching the result of `GET /api/next-slots`.
- When `sensitive: true` is supplied to `add_schedule` or `post_now`, the system shall propagate the flag to `ScheduledPost.sensitive` / `PostContent.sensitive`.
- When a target SNS whose posting client is not implemented is specified, the system shall return an explicit message stating that the SNS is not supported.
- The system shall construct SNS clients through a single shared factory in `src/sns/mod.rs`.

### 6. テスト (#95)

現状 MCP のテストは2本のみで、いずれも存在しないセッションに対して 202 が返ることしか検証していない。

- When `tools/list` is requested, the response shall contain every tool name defined in the tool registry.
- When an unknown method is requested, the response shall contain `error.code == -32601`.
- When a JSON-RPC notification without `id` is given, the response builder shall return no response.
- When a client connects to `GET /api/mcp/sse`, the system shall emit an `endpoint` event whose session id is registered in `AppState.mcp_sessions`.
- Where a test exercises a tool that performs an outbound HTTP call, the test shall inject a `wiremock::MockServer::uri()` and shall not access the external network.
- Where a test writes files, the test shall use a temporary directory and shall not modify `data/`.

### 7. ドキュメント (#101)

現状 README / docs に MCP の記述が一切ない。

- The README shall describe the MCP endpoints, the authentication header, and an `mcpServers` configuration example using an RFC 2606 reserved domain.
- The documentation shall list all breaking changes with migration instructions.
- The `config.yml.template` shall include an `mcp` section.

## 非機能要件

### カバレッジ

- The coverage threshold in `coverage-threshold.txt` shall never be lowered.
- When measured coverage exceeds the threshold, the threshold shall be raised within the same pull request.

### 後方互換性

- When an existing `config.yml` is loaded, the system shall operate without requiring any configuration change, except for the documented breaking changes.

## 破壊的変更

| 変更 | 影響 | 移行方法 |
|---|---|---|
| `GET /logout` の廃止 | ブックマークからのログアウトが 404 になる | Web UI のログアウトボタンを使う |
| MCP tool の media パス制限 | `data/` 外のパスを渡していたワークフローが失敗する | `mcp.allowed_media_dirs` に対象ディレクトリを追加する |

## 非ゴール

以下は本specの対象外とし、別Issueで扱う。

- Threads / Tumblr の `SnsClient` 実装。現状 `src/sns/` に存在せず、README でも「移植対象外」とされている
- rmcp クレートへの移行、および Streamable HTTP トランスポート (2025-03-26 以降の仕様) への対応
- `CorsLayer::permissive()` の見直し。Web UI の挙動に影響するため分離する
