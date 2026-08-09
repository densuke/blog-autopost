# サーバーへの導入と更新

Linux サーバーで常駐させる手順です。systemd のユーザーサービスとして動かす構成を前提にしています。

配布物は GitHub Releases にあります。CI がタグごとに x86_64 Linux (musl / 静的リンク) と aarch64 macOS のアーカイブを作るため、サーバー側でビルドする必要はありません。

## 構成

```
~/work/
├── bap-kicker.sh              # 起動スクリプト
└── blog-autopost/
    ├── blog-autopost-rs       # バイナリ
    ├── static/                # Web UI (バイナリと必ず一緒に更新する)
    ├── config.yml             # 設定 (更新時は触らない)
    └── data/                  # 記事の既読管理と予約 (更新時は触らない)
```

`static/` を置く場所は実行時のカレントディレクトリからの相対パスです。起動スクリプトで `cd` してから実行してください。

## 初回の導入

### 1. 配布物を取得する

```bash
mkdir -p ~/work/blog-autopost && cd ~/work/blog-autopost

VERSION=0.1.7
BASE=https://github.com/densuke/blog-autopost/releases/download/v${VERSION}
FILE=blog-autopost-rs-x86_64-unknown-linux-musl-${VERSION}.tar.gz

curl -sSL -O ${BASE}/${FILE}
curl -sSL -O ${BASE}/${FILE}.sha256
sha256sum -c ${FILE}.sha256
```

**チェックサムの照合は必ず行ってください。** `OK` が出なければ展開しないこと。

```bash
tar -xzf ${FILE}
cp blog-autopost-rs/blog-autopost-rs .
cp -r blog-autopost-rs/static .
cp blog-autopost-rs/config.yml.template .
chmod +x blog-autopost-rs
```

### 2. 設定する

```bash
cp config.yml.template config.yml
chmod 600 config.yml
```

`config.yml` には SNS の認証情報が入るため、パーミッションは `600` にしてください。編集内容は [config.yml.template](../config.yml.template) のコメントを参照してください。

最低限、`web_auth` と投稿先の `sns` を設定します。MCP を使う場合は `mcp.api_key` も設定してください (後述)。

### 3. 起動スクリプトを作る

`~/work/bap-kicker.sh`:

```bash
#!/bin/bash
cd blog-autopost || exit 1
PATH=$HOME/.local/bin:$PATH
exec ./blog-autopost-rs serve --port 9999
```

```bash
chmod +x ~/work/bap-kicker.sh
```

### 4. systemd ユーザーサービスを登録する

`~/.config/systemd/user/blog-autopost.service`:

```ini
[Unit]
Description=blog autopost checker (user service)
# ユーザー空間では network-online.target が期待どおり働かないことがあるため、
# 起動順序に依存せず Restart=always で回復させる。
After=network.target

[Service]
Type=simple
WorkingDirectory=/home/YOUR_USER/work
ExecStart=/home/YOUR_USER/work/bap-kicker.sh
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
```

```bash
systemctl --user daemon-reload
systemctl --user enable --now blog-autopost.service
systemctl --user status blog-autopost.service
```

ログアウトしても動かし続けるには linger を有効にします。

```bash
loginctl enable-linger $USER
```

## 更新

**バイナリと `static/` は必ず一緒に更新してください。** Web UI の HTML はバイナリ側の仕様に追従しており、片方だけ入れ替えると画面の一部が動かなくなります。実例として 0.1.7 では `GET /logout` を廃止したため、古い `index.html` のままだとログアウトが 405 になります。

`config.yml` と `data/` は触りません。設定は後方互換が保たれており、新しい項目はすべて任意です。

### 1. バックアップを取る

```bash
cd ~/work/blog-autopost
BK=~/work/blog-autopost-backup-$(date +%Y%m%d%H%M%S)
mkdir -p $BK
cp blog-autopost-rs $BK/
cp -r static $BK/
cp config.yml $BK/
echo "バックアップ: $BK"
```

### 2. 新しい版を取得して照合する

```bash
cd /tmp && rm -rf bap-upgrade && mkdir bap-upgrade && cd bap-upgrade

VERSION=0.1.7
BASE=https://github.com/densuke/blog-autopost/releases/download/v${VERSION}
FILE=blog-autopost-rs-x86_64-unknown-linux-musl-${VERSION}.tar.gz

curl -sSL -O ${BASE}/${FILE}
curl -sSL -O ${BASE}/${FILE}.sha256
sha256sum -c ${FILE}.sha256

tar -xzf ${FILE}
./blog-autopost-rs/blog-autopost-rs --version
```

### 3. サービスを止める

**実行中のバイナリは置き換えられません。** 先に止めてください。

```bash
systemctl --user stop blog-autopost.service
systemctl --user is-active blog-autopost.service   # inactive を確認
```

停止中は予約投稿が実行されません。投稿予定時刻が近い場合はずらしてから作業してください。

### 4. 入れ替える

```bash
cd ~/work/blog-autopost
SRC=/tmp/bap-upgrade/blog-autopost-rs

cp $SRC/blog-autopost-rs ./blog-autopost-rs
chmod +x ./blog-autopost-rs
cp $SRC/static/*.html $SRC/static/*.css $SRC/static/*.js ./static/

./blog-autopost-rs --version
```

### 5. 起動して確認する

```bash
systemctl --user start blog-autopost.service
sleep 3
journalctl --user -u blog-autopost.service --since "1 minute ago" --no-pager | tail -5
```

起動ログに認証モードとリッスン先が出ます。

```
MCP auth: mcp.api_key (dedicated)
Web UI listening on http://0.0.0.0:9999
```

動作確認:

```bash
KEY=<web_auth.secret_key の値>

# 稼働バージョン。update_available が false なら最新
curl -s http://127.0.0.1:9999/api/version -H "X-Api-Key: $KEY"

# ログイン画面が出るか
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9999/login

# 認証なしは 401 になるか
curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:9999/api/config
```

ブラウザでログインし直してください。**更新でセッションIDの形式が変わった場合、既存のログインは無効になります。**

### 6. 後片付け

```bash
rm -rf /tmp/bap-upgrade
```

バックアップはしばらく残し、問題がないことを確認してから削除してください。

## ロールバック

バックアップを戻して再起動します。

```bash
systemctl --user stop blog-autopost.service
BK=~/work/blog-autopost-backup-XXXXXXXX   # 戻したい世代
cd ~/work/blog-autopost
cp $BK/blog-autopost-rs ./blog-autopost-rs
cp -r $BK/static/. ./static/
systemctl --user start blog-autopost.service
./blog-autopost-rs --version
```

`data/` は更新で触っていないため、そのまま使えます。

## MCP 専用キーの設定

MCP を使う場合、`web_auth.secret_key` を共用せず専用キーを設定してください。共用していると、キーが漏れたときに Web UI の操作までまとめて奪われます。

起動ログに次の警告が出ていたら未分離の状態です。

```
MCP auth: web_auth.secret_key (legacy). Consider setting mcp.api_key ...
```

### 設定手順

```bash
cd ~/work/blog-autopost
cp config.yml /tmp/config.yml.before-mcp    # 念のため
openssl rand -hex 32                         # 生成された値を控える
```

`config.yml` の末尾に追記します。

```yaml
mcp:
  api_key: <生成した値>
```

反映前に構文を確認してください。

```bash
python3 -c "import yaml; yaml.safe_load(open('config.yml')); print('YAML OK')"
systemctl --user restart blog-autopost.service
```

起動ログが `MCP auth: mcp.api_key (dedicated)` に変われば分離できています。

### 分離できたかの確認

```bash
WEBKEY=<web_auth.secret_key>
MCPKEY=<mcp.api_key>
BODY='{"jsonrpc":"2.0","method":"tools/list","id":1}'

# MCP は専用キーだけを受け付ける
curl -s -o /dev/null -w "MCP  専用キー: %{http_code} (200)\n" -X POST http://127.0.0.1:9999/api/mcp \
  -H "X-Api-Key: $MCPKEY" -H "Content-Type: application/json" -d "$BODY"
curl -s -o /dev/null -w "MCP  web キー: %{http_code} (401)\n" -X POST http://127.0.0.1:9999/api/mcp \
  -H "X-Api-Key: $WEBKEY" -H "Content-Type: application/json" -d "$BODY"

# 通常 API は secret_key だけを受け付ける
curl -s -o /dev/null -w "API  web キー: %{http_code} (200)\n" http://127.0.0.1:9999/api/config -H "X-Api-Key: $WEBKEY"
curl -s -o /dev/null -w "API  専用キー: %{http_code} (401)\n" http://127.0.0.1:9999/api/config -H "X-Api-Key: $MCPKEY"
```

括弧内が期待値です。双方向で 401 になっていれば、どちらのキーが漏れてももう一方には届きません。

クライアントへの登録は [docs/mcp.md](mcp.md) を参照してください。

## リバースプロキシ配下での公開

外部へ公開する場合の注意点です。詳細は [docs/mcp.md](mcp.md) の「リバースプロキシ配下での運用」にもあります。

**HTTPS を使ってください。** API キーはヘッダの平文で送られるため、HTTP で公開すると経路上で読めます。

プロキシから `X-Forwarded-Proto: https` を渡してください。これがないとセッション Cookie に `Secure` が付きません (`cookie_secure` の既定 `auto` がこのヘッダを見ています)。

パスは書き換えないでください。エンドポイントは固定で、サブパス配信 (`example.com/autopost/...`) は旧 SSE トランスポートが動きません。サブドメインかポートで分けてください。

nginx の例:

```nginx
location / {
    proxy_pass http://127.0.0.1:9999;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;

    # 旧 SSE トランスポートを使う場合に必要
    proxy_read_timeout 3600s;
    proxy_buffering off;
}
```

## 困ったとき

### サービスが起動しない

```bash
journalctl --user -u blog-autopost.service --no-pager | tail -30
```

設定ファイルの構文エラーが多いパターンです。

```bash
python3 -c "import yaml; yaml.safe_load(open('config.yml')); print('YAML OK')"
```

### ポートが塞がっている

```bash
ss -tlnp | grep 9999
```

前のプロセスが残っている場合は止めてください。

### 画面の一部が動かない

`static/` の更新漏れがほぼ確実です。バイナリと同じ版の `static/` に入れ替えてください。

```bash
ls -la ~/work/blog-autopost/static/     # 更新日時をバイナリと比べる
```

### バージョンが上がっていない

`/api/version` の `current` を見てください。`update_available` が `true` なら、より新しい版が公開されています。

```bash
curl -s http://127.0.0.1:9999/api/version -H "X-Api-Key: <secret_key>"
```

この確認は1日1回、起動時とその後24時間ごとに行われます。取得に失敗しても稼働そのものには影響しません。
