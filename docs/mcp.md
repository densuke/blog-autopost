# MCP サーバ

Blog AutoPost の Web サーバは MCP (Model Context Protocol) のエンドポイントを備えています。Claude Code などの AI クライアントから、予約投稿の管理や即時投稿を操作できます。

## できること

| やりたいこと | 使う tool |
|---|---|
| 今すぐ投稿する | `post_now` |
| 時刻を指定して予約する | `add_schedule` の `at` |
| 次に投稿できる枠へ予約する | `add_schedule` の `auto_slot: true` |
| 次に投稿できる枠を調べる (予約しない) | `get_next_slots` |
| 予約の一覧を見る | `list_schedules` |
| 予約を変更する | `update_schedule` |
| 予約を削除する | `delete_schedule` |

## エンドポイント

2つのトランスポートを併置しています。**新しく設定する場合は Streamable HTTP を使ってください。**

### Streamable HTTP (推奨、MCP 2025-03-26 以降)

| メソッド | パス | 挙動 |
|---|---|---|
| `POST` | `/api/mcp` | JSON-RPC リクエストを受け、レスポンスをボディで返す。通知には `202` |
| `GET` | `/api/mcp` | `405`。サーバ発のストリームは提供しない |
| `DELETE` | `/api/mcp` | `405`。セッションを持たないため終了操作もない |

セッションIDは払い出しません。tool の実行にサーバ側の状態が要らないためです。`Mcp-Session-Id` を送る必要はありません。

対応バージョンは `2025-06-18` / `2025-03-26` / `2024-11-05` です。`MCP-Protocol-Version` ヘッダで別の版を指定すると `400` を返します。

### HTTP+SSE (非推奨、MCP 2024-11-05)

| メソッド | パス | 役割 |
|---|---|---|
| `GET` | `/api/mcp/sse` | SSE 接続を確立する。最初に `endpoint` イベントでメッセージ送信先を通知する |
| `POST` | `/api/mcp/message?session_id=...` | JSON-RPC リクエストを受け付ける。レスポンスは SSE 側へ流れる |

MCP 仕様でこのトランスポートは非推奨になりました。既存の接続のために残していますが、新規は Streamable HTTP を使ってください。

### クライアントの対応状況

| クライアント | Streamable HTTP | HTTP+SSE |
|---|---|---|
| Claude Code | 対応 (`--transport http`) | 対応 (`--transport sse`、非推奨) |
| Codex | 対応 | **非対応** |

## 設定

### API キーを用意する

MCP 専用の API キーを設定してください。Web UI の `secret_key` を共用すると、キーが漏れたときに画面操作までまとめて奪われます。

```bash
openssl rand -hex 32
```

`config.yml` に `mcp` 節を追加します。

```yaml
web_auth:
  username: "admin"
  password: "..."
  secret_key: "<WEB_SESSION_SECRET>"

mcp:
  api_key: "<MCP_API_KEY>"   # secret_key とは別の値にする
```

### 認証の動作

`mcp` 節を書いたかどうかで動作が変わります。

| `config.yml` の状態 | MCP の認証 |
|---|---|
| `mcp` 節なし | `web_auth.secret_key` で通る (従来どおり)。起動時に専用キーを推奨するログが出る |
| `mcp.api_key` あり | 専用キーのみ。`secret_key` では通らない |
| `mcp.api_key` + `allow_web_secret_key: true` | 両方通る (移行期間用) |
| `mcp.enabled: false` | 常に 401 |
| `mcp` 節はあるが `api_key` も `allow_web_secret_key` も未設定 | `secret_key` で通る。`allowed_media_dirs` などだけ設定した場合を壊さない |
| `allow_web_secret_key: false` のみ (キーなし) | 401。`secret_key` を使わせない意図とみなす |

既存の `config.yml` をそのまま使っている場合は従来どおり動きます。`api_key` を設定したときだけ分離モードへ切り替わります。

なお MCP のエンドポイントでは**ブラウザのセッション Cookie を受け付けません**。ヘッダで API キーを渡してください。

起動時にどの方針で動いているかがログに出ます。

```
MCP auth: mcp.api_key (dedicated)
```

### 設定項目

```yaml
mcp:
  # MCP エンドポイントを有効にするか。既定は true。
  enabled: true

  # MCP 専用の API キー。
  api_key: "<MCP_API_KEY>"

  # 移行期間だけ web_auth.secret_key も受け付けたい場合に true。既定は false。
  allow_web_secret_key: false

  # tool がメディアとして参照できるディレクトリ。
  # 未指定時は data/uploads と data のみ。
  allowed_media_dirs:
    - "data/uploads"
    - "data"

  # Origin ヘッダを付けたリクエストを受け付けるオリジン。
  # MCP クライアントは Origin を送らないため通常は設定不要。
  # 未設定時は Origin 付きのリクエストを拒否する (DNS リバインディング対策)。
  allowed_origins:
    - "https://ui.example.com"
```

## クライアントの設定

### コマンドで登録する (推奨)

`mcp` サブコマンドが、クライアントごとの形式へ変換して登録します。API キーは `config.yml` から読むので、手で貼り付ける必要はありません。

```bash
# Claude Code へ登録
blog-autopost-rs mcp install --client claude --url https://autopost.example.com

# Codex へ登録
blog-autopost-rs mcp install --client codex --url https://autopost.example.com

# 取り除く
blog-autopost-rs mcp uninstall --client claude

# 書き込まずに内容だけ見る
blog-autopost-rs mcp print --client codex --url https://autopost.example.com

# 何が起きるかだけ確認する
blog-autopost-rs mcp install --client codex --url https://autopost.example.com --dry-run
```

`--url` はベース URL でよく、`/api/mcp` は自動で付きます。古い `/api/mcp/sse` を渡した場合も新しいエンドポイントへ寄せます。

| オプション | 既定値 | 説明 |
|---|---|---|
| `--client` | (必須) | `claude` または `codex` |
| `--url` | (必須) | 接続先。`https://autopost.example.com` の形 |
| `--name` | `blog-autopost` | クライアントへ登録するサーバ名 |
| `--scope` | `user` | Claude Code のスコープ (`local` / `project` / `user`) |
| `--key-env-var` | `BLOG_AUTOPOST_MCP_KEY` | Codex がキーを読む環境変数の名前 |
| `--dry-run` | — | 変更せず内容だけ表示する |

API キーは `mcp.api_key` を使い、未設定なら `web_auth.secret_key` へフォールバックします (認証側の判定と同じ規則)。どちらも無ければキーの生成方法を案内してエラーになります。

Codex はキーを環境変数から読むため、`export` すべき内容が表示されます。**シェルの設定ファイルは書き換えません**ので、自分で追記してください。

以下は手で設定する場合の内容です。

### Claude Code

CLI で登録します。

```bash
claude mcp add --transport http blog-autopost \
  https://autopost.example.com/api/mcp \
  --header "X-Api-Key: <MCP_API_KEY>" \
  --scope user
```

`--scope` は `local` (既定、そのプロジェクトのみ) / `project` (`.mcp.json` に書かれ共有される) / `user` (全プロジェクト) から選びます。**キーを含む設定を版管理へ入れたくない場合は `project` を避けてください。**

削除と確認:

```bash
claude mcp remove blog-autopost
claude mcp list
```

JSON で直接書く場合 (`.mcp.json` / `~/.claude.json`):

```json
{
  "mcpServers": {
    "blog-autopost": {
      "type": "http",
      "url": "https://autopost.example.com/api/mcp",
      "headers": {
        "X-Api-Key": "<MCP_API_KEY>"
      }
    }
  }
}
```

`type` を省略すると stdio サーバと解釈されて動きません。`http` (または別名 `streamable-http`) を必ず指定してください。

### Codex

`~/.codex/config.toml` に書きます。

```toml
[mcp_servers.blog-autopost]
url = "https://autopost.example.com/api/mcp"
bearer_token_env_var = "BLOG_AUTOPOST_MCP_KEY"
startup_timeout_sec = 30
```

**キーは環境変数名で指定します。** TOML に直接書く `bearer_token` は受け付けられません。

```bash
export BLOG_AUTOPOST_MCP_KEY="<MCP_API_KEY>"
```

環境によっては rmcp クライアントを有効にする必要があります。

```toml
[features]
experimental_use_rmcp_client = true
```

Codex は Streamable HTTP のみに対応しており、`/api/mcp/sse` では接続できません。

### 疎通確認

```bash
# Streamable HTTP: レスポンスがボディで返る
curl -s -X POST http://localhost:8080/api/mcp \
  -H "X-Api-Key: <MCP_API_KEY>" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}' | jq .

# 分離できていれば web_auth.secret_key では 401 になる
curl -i -X POST http://localhost:8080/api/mcp \
  -H "X-Api-Key: <WEB_SESSION_SECRET>" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}'
```

`Authorization: Bearer <MCP_API_KEY>` でも認証できます。

## tool の詳細

### `post_now`

今すぐ投稿します。予約はしません。

| 引数 | 型 | 説明 |
|---|---|---|
| `text` | string | 投稿本文 (必須) |
| `sns` | string | 送信先SNS名。カンマ区切り。省略時は投稿可能な全SNS |
| `media` | string[] | 添付するローカル画像のパス。許可ディレクトリ配下のみ |
| `link` | string | 添付するリンクURL |
| `sensitive` | boolean | 添付をセンシティブとして扱う (現状 Misskey のみ有効) |

`sns` にはアカウント名 (`mstdn-main`)、種別名 (`mastodon`)、表示ラベル (`Mastodon (mstdn-main)`) のいずれも指定できます。大文字小文字は区別しません。

### `add_schedule`

予約投稿を追加します。`at` と `auto_slot` のどちらかが必要です。

| 引数 | 型 | 説明 |
|---|---|---|
| `text` | string | 投稿本文 (必須) |
| `at` | string | 投稿予定時刻。RFC3339 (`2030-06-20T18:00:00+09:00`) / `YYYY-MM-DD HH:MM:SS` / `YYYY-MM-DD HH:MM` |
| `auto_slot` | boolean | 次に投稿できる枠を自動で探す。SNS ごとに個別の予約を作る |
| `sns` | string | 投稿先SNS名。カンマ区切り。省略時は投稿可能な全SNS |
| `media` | string[] | 添付するローカル画像のパス。許可ディレクトリ配下のみ |
| `link` | string | 添付するリンクURL |
| `sensitive` | boolean | 添付をセンシティブとして扱う (現状 Misskey のみ有効) |

`media` に渡したファイルは予約の時点で `data/uploads` へ複製されます。元のファイルを投稿時刻より前に消しても投稿できます。

### `get_next_slots`

各SNSの次に投稿できる枠を調べます。予約はしません。

| 引数 | 型 | 説明 |
|---|---|---|
| `sns` | string | 対象SNS名。カンマ区切り。省略時は全SNS |

`allowed_timings` を設定していないSNSは「いつでも投稿可能」として扱われ、直近の時刻が返ります。

### `list_schedules`

予約の一覧を時刻の昇順で返します。

| 引数 | 型 | 説明 |
|---|---|---|
| `status` | string | ステータスで絞り込む (`予約済み` / `投稿済み` / `失敗`) |

### `update_schedule`

予約を変更します。渡した項目だけが更新されます。

| 引数 | 型 | 説明 |
|---|---|---|
| `id` | string | 対象の予約ID (必須) |
| `text` | string | 変更後の本文 |
| `at` | string | 変更後の予定時刻 |
| `sns` | string | 変更後のSNS名 |
| `status` | string | 変更後のステータス |
| `link` | string | 変更後のリンクURL |

### `delete_schedule`

| 引数 | 型 | 説明 |
|---|---|---|
| `id` | string | 対象の予約ID (必須) |

## メディアの制限

`media` 引数に渡せるファイルには制限があります。検証を通さないと、認証済みのクライアントが任意のファイルをSNSへ送信できてしまうためです。

- `mcp.allowed_media_dirs` (未指定時は `data/uploads` と `data`) の配下にあること
- シンボリックリンクは解決後の場所で判定する
- 中身が対応する画像または動画であること (拡張子ではなくマジックバイトで判断)
- 10MB 以下であること

`data/` の外にあるファイルを添付したい場合は `allowed_media_dirs` にそのディレクトリを追加してください。

## 未対応のSNS

Threads と Tumblr は `config.yml` に設定を書けますが、投稿クライアントの実装がありません。`sns` 引数で名指しすると、その旨のエラーが返ります。

```
These SNS accounts are configured but posting is not implemented in this build:
threads-main. Remove them from the 'sns' argument.
```

`sns` を省略した場合は、投稿できるSNSだけが対象になります。

## 動作確認の手順

```bash
# 1. サーバを起動する
just run-web

# 2. 起動時のログで認証モードを確認する
#    "MCP auth: mcp.api_key (dedicated)" などが出る

# 3. クライアントを設定して接続する

# 4. 一連の操作を試す
#    get_next_slots → add_schedule (auto_slot) → list_schedules → delete_schedule
```

## 制限事項

- 対応しているメソッドは `initialize` / `initialized` / `tools/list` / `tools/call` です。`resources/*` と `prompts/*` は実装していません
- サーバからクライアントへ自発的にメッセージを送りません。`GET /api/mcp` は `405` を返します
- セッションを持たないため `Mcp-Session-Id` を払い出しません。ストリームの再開 (`Last-Event-ID`) にも対応していません
- HTTP+SSE 側の `mcp_sessions` はプロセスのメモリ上にあります。複数インスタンスへロードバランスすると、SSE を張った先と POST の到達先がずれて動きません。Streamable HTTP はステートレスなのでこの制約を受けません

## リバースプロキシ配下での運用

自宅から外部のサーバへ繋ぐ場合の注意点です。

サーバーの導入・更新そのものの手順は [deploy.md](deploy.md) を参照してください。

### HTTPS を使う

API キーは `X-Api-Key` または `Authorization` ヘッダの平文で送られます。HTTP で外部公開すると経路上でキーが読めます。

あわせてプロキシから `X-Forwarded-Proto: https` を渡してください。これがないと Web UI 側の Cookie に `Secure` が付きません (`cookie_secure` の既定 `auto` がこのヘッダを見ています)。

### パスを書き換えない

エンドポイントのパスは `/api/mcp` と `/api/mcp/sse` に固定されており、設定で変えられません。

サブパスで配信する構成 (`example.com/autopost/api/mcp` → バックエンドの `/api/mcp`) は、HTTP+SSE 側が `endpoint` イベントで相対パスを返すため動きません。**サブドメインかポートで分けてください。**

Streamable HTTP はパスを自分で組み立てないため、サブパス配信でも動きます。

### SSE を使う場合はタイムアウトを延ばす

HTTP+SSE 側を使う場合、nginx なら次の設定が必要です。

```nginx
proxy_read_timeout 3600s;
proxy_buffering off;
```

既定の 60 秒で切られる構成だと接続が落ちます。Streamable HTTP はリクエストごとに完結するのでこの設定は不要です。
