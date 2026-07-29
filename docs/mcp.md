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

トランスポートは HTTP+SSE (MCP 2024-11-05 仕様) です。

| メソッド | パス | 役割 |
|---|---|---|
| `GET` | `/api/mcp/sse` | SSE 接続を確立する。最初に `endpoint` イベントでメッセージ送信先を通知する |
| `POST` | `/api/mcp/message?session_id=...` | JSON-RPC リクエストを受け付ける。レスポンスは SSE 側へ流れる |

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

| `config.yml` の状態 | `/api/mcp/*` の認証 |
|---|---|
| `mcp` 節なし | `web_auth.secret_key` で通る (従来どおり)。起動時に専用キーを推奨するログが出る |
| `mcp.api_key` あり | 専用キーのみ。`secret_key` では通らない |
| `mcp.api_key` + `allow_web_secret_key: true` | 両方通る (移行期間用) |
| `mcp.enabled: false` | 常に 401 |
| `mcp` 節ありで `api_key` なし | 401 (設定不備として扱う) |

既存の `config.yml` をそのまま使っている場合は従来どおり動きます。`mcp` 節を追加したときだけ、専用キーによる分離モードへ切り替わります。

なお `/api/mcp/*` では**ブラウザのセッション Cookie は受け付けません**。ヘッダで API キーを渡してください。

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
```

## クライアントの設定

`X-Api-Key` ヘッダに MCP 専用キーを載せます。

```json
{
  "mcpServers": {
    "blog-autopost": {
      "type": "sse",
      "url": "https://autopost.example.com/api/mcp/sse",
      "headers": {
        "X-Api-Key": "<MCP_API_KEY>"
      }
    }
  }
}
```

`Authorization: Bearer <MCP_API_KEY>` でも認証できます。

疎通確認は curl でも行えます。

```bash
# endpoint イベントが返れば接続できている
curl -N -H "X-Api-Key: <MCP_API_KEY>" http://localhost:8080/api/mcp/sse

# 分離できていれば web_auth.secret_key では 401 になる
curl -i -H "X-Api-Key: <WEB_SESSION_SECRET>" http://localhost:8080/api/mcp/sse
```

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

- トランスポートは HTTP+SSE (2024-11-05 仕様) のみです。Streamable HTTP (2025-03-26 以降) には未対応です
- 対応しているメソッドは `initialize` / `initialized` / `tools/list` / `tools/call` です。`resources/*` と `prompts/*` は実装していません
