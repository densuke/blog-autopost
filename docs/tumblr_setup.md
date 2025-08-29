# Tumblr API 設定ガイド

このドキュメントでは、Blog AutoPostツールでTumblrに投稿するために必要なAPI認証情報の取得方法を説明します。

## 前提条件

- Tumblrアカウントが必要です
- 投稿先のTumblrブログを持っている必要があります

## API認証情報の取得手順

### 1. Tumblr開発者アプリケーションの登録

1. **Tumblr Developer Portalにアクセス**
   - ブラウザで [https://www.tumblr.com/oauth/apps](https://www.tumblr.com/oauth/apps) を開きます
   - Tumblrアカウントでログインします

2. **新しいアプリケーションを作成**
   - 「Register Application」ボタンをクリックします
   - アプリケーション情報を入力します：
     - **Application name**: 分かりやすい名前（例：「Blog AutoPost」）
     - **Application website**: あなたのブログのURL
     - **Application description**: アプリの説明（例：「ブログの新着記事を自動投稿するツール」）
     - **Default callback URL**: `http://localhost:8080/callback` を入力
   - 利用規約に同意してアプリケーションを作成します

3. **API Key情報を確認**
   - アプリケーション作成後、以下の情報が表示されます：
     - **OAuth Consumer Key** → これが `client_id` になります
     - **OAuth Consumer Secret** → これが `client_secret` になります

### 2. OAuth 2.0 Access Tokenの取得

TumblrはOAuth 2.0を使用するため、アクセストークンの取得が必要です。

#### 方法1: 手動でアクセストークンを取得する場合

1. **認証URLの作成**
   
   以下の形式でブラウザでアクセスします（YOUR_CLIENT_IDを実際の値に置き換えてください）：
   
   ```
   https://www.tumblr.com/oauth2/authorize?client_id=YOUR_CLIENT_ID&response_type=code&scope=write&redirect_uri=http://localhost:8080/callback&state=random_string
   ```

2. **認証の承認**
   - Tumblrの認証画面で「Allow」をクリックします
   - リダイレクト先のURLに認証コードが含まれます：
     ```
     http://localhost:8080/callback?code=AUTHORIZATION_CODE&state=random_string
     ```
   - この `AUTHORIZATION_CODE` を保存します

3. **アクセストークンの取得**
   
   以下のcurlコマンドでアクセストークンを取得します：
   
   ```bash
   curl -X POST https://api.tumblr.com/v2/oauth2/token \
     -H "Content-Type: application/x-www-form-urlencoded" \
     -d "grant_type=authorization_code" \
     -d "code=AUTHORIZATION_CODE" \
     -d "client_id=YOUR_CLIENT_ID" \
     -d "client_secret=YOUR_CLIENT_SECRET" \
     -d "redirect_uri=http://localhost:8080/callback"
   ```
   
   レスポンス例：
   ```json
   {
     "access_token": "your_access_token_here",
     "token_type": "Bearer",
     "expires_in": 3600,
     "refresh_token": "your_refresh_token_here"
   }
   ```

#### 方法2: Pythonスクリプトを使用する場合

以下のPythonスクリプトを使用してアクセストークンを取得できます：

```python
#!/usr/bin/env python3
"""
Tumblr OAuth 2.0 アクセストークン取得スクリプト
"""
import requests
import urllib.parse
from http.server import HTTPServer, BaseHTTPRequestHandler
import threading
import webbrowser
import sys

class CallbackHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path.startswith('/callback'):
            query = urllib.parse.urlparse(self.path).query
            params = urllib.parse.parse_qs(query)
            
            if 'code' in params:
                self.server.auth_code = params['code'][0]
                self.send_response(200)
                self.send_header('Content-type', 'text/html')
                self.end_headers()
                self.wfile.write(b'<html><body><h1>Authorization successful!</h1><p>You can close this window.</p></body></html>')
            else:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b'Authorization failed')
        else:
            self.send_response(404)
            self.end_headers()

def get_tumblr_access_token():
    # 認証情報を入力
    client_id = input("Tumblr Client ID を入力してください: ")
    client_secret = input("Tumblr Client Secret を入力してください: ")
    
    # OAuth2認証URLを生成
    auth_url = f"https://www.tumblr.com/oauth2/authorize?client_id={client_id}&response_type=code&scope=write&redirect_uri=http://localhost:8080/callback&state=blog-autopost"
    
    # ローカルサーバーを起動
    server = HTTPServer(('localhost', 8080), CallbackHandler)
    server.auth_code = None
    
    # バックグラウンドでサーバーを起動
    server_thread = threading.Thread(target=server.serve_forever)
    server_thread.daemon = True
    server_thread.start()
    
    print("認証URLをブラウザで開きます...")
    print(f"URL: {auth_url}")
    webbrowser.open(auth_url)
    
    # 認証コードを待機
    print("認証を完了してください...")
    while server.auth_code is None:
        pass
    
    auth_code = server.auth_code
    server.shutdown()
    
    print(f"認証コード取得: {auth_code}")
    
    # アクセストークンを取得
    token_url = "https://api.tumblr.com/v2/oauth2/token"
    token_data = {
        "grant_type": "authorization_code",
        "code": auth_code,
        "client_id": client_id,
        "client_secret": client_secret,
        "redirect_uri": "http://localhost:8080/callback"
    }
    
    response = requests.post(token_url, data=token_data)
    
    if response.status_code == 200:
        token_info = response.json()
        print("\\n=== アクセストークン取得成功 ===")
        print(f"Access Token: {token_info['access_token']}")
        print(f"Token Type: {token_info['token_type']}")
        print(f"Expires In: {token_info.get('expires_in', 'N/A')} seconds")
        if 'refresh_token' in token_info:
            print(f"Refresh Token: {token_info['refresh_token']}")
        return token_info['access_token']
    else:
        print(f"トークン取得エラー: {response.status_code}")
        print(f"エラー詳細: {response.text}")
        return None

if __name__ == "__main__":
    access_token = get_tumblr_access_token()
    if access_token:
        print("\\n.envファイルまたは設定ファイルに以下の情報を追加してください:")
        print(f"TUMBLR_ACCESS_TOKEN={access_token}")
```

### 3. ブログ名の確認

投稿先のTumblrブログ名を確認します：

- ブログのURLが `https://example.tumblr.com` の場合、ブログ名は `example` です
- メインブログの場合は、Tumblrのユーザー名がブログ名になります

### 4. 設定ファイルの更新

取得した認証情報を設定に追加します。

#### 環境変数を使用する場合（推奨）

`.env` ファイルに以下の情報を追加：

```bash
# Tumblr 認証情報
TUMBLR_CLIENT_ID=your_client_id_here
TUMBLR_CLIENT_SECRET=your_client_secret_here
TUMBLR_ACCESS_TOKEN=your_access_token_here
TUMBLR_BLOG_NAME=your_blog_name_here
```

#### config.ymlファイルを使用する場合

`config.yml` ファイルのSNSセクションに以下を追加：

```yaml
sns:
  - type: tumblr
    name: "tumblr-main"
    client_id: "YOUR_TUMBLR_CLIENT_ID"
    client_secret: "YOUR_TUMBLR_CLIENT_SECRET"
    access_token: "YOUR_TUMBLR_ACCESS_TOKEN"
    blog_name: "YOUR_BLOG_NAME"
    tags:  # オプション: 自動タグ付け
      - "blog"
      - "auto-post"
```

## 投稿のテスト

設定が完了したら、ドライランでテストを実行します：

```bash
# テキスト投稿のテスト
uv run -m src.main --text "テスト投稿です" --dry-run --debug --sns tumblr

# RSS監視機能のテスト（投稿数制限付き）
uv run -m src.main --dry-run --debug --limit 1
```

## 注意事項

1. **アクセストークンの有効期限**
   - Tumblrのアクセストークンには有効期限があります
   - トークンが期限切れになった場合は、新しいトークンを取得する必要があります

2. **投稿制限**
   - Tumblr APIには投稿回数の制限があります
   - 過度な投稿は避け、適切な間隔で使用してください

3. **プライバシー設定**
   - 投稿するブログのプライバシー設定を確認してください
   - プライベートブログの場合、適切な権限設定が必要です

4. **コンテンツポリシー**
   - Tumblrのコミュニティガイドラインを遵守してください
   - 不適切なコンテンツの自動投稿は避けてください

## トラブルシューティング

### よくある問題と解決方法

**Q: 認証エラーが発生します**
- APIキーとシークレットが正しいか確認してください
- アクセストークンが有効か確認してください
- ブログ名が正しいか確認してください

**Q: 投稿が失敗します**
- ブログの投稿権限があるか確認してください
- 投稿内容がTumblrのポリシーに準拠しているか確認してください
- APIの制限に達していないか確認してください

**Q: 画像投稿ができません**
- 画像ファイルのサイズと形式を確認してください
- ファイルパスが正しいか確認してください

## 参考リンク

- [Tumblr API Documentation](https://www.tumblr.com/docs/en/api/v2)
- [Tumblr Developer Portal](https://www.tumblr.com/oauth/apps)
- [OAuth 2.0 Authorization Framework](https://tools.ietf.org/html/rfc6749)