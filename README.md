# Blog AutoPost CLI

Blog AutoPost CLIは、指定したブログの更新を定期的にチェックし、新しい記事が投稿された場合に各種SNSへ自動的にポストするコマンドラインツールです。

## 概要

このツールは、以下の機能を提供します。

- RSS/Atomフィードを介したブログの更新チェック
- 新着記事の検出と記録
- プラグイン形式による各種SNSへの自動ポスト

## 動作要件

- Python 3.12以上
- uv (Pythonパッケージ管理ツール)

## セットアップ

1.  **リポジトリのクローン**

    ```bash
    git clone https://github.com/your-username/blog-autopost.git
    cd blog-autopost
    ```

2.  **Python環境のセットアップ**

    `uv` を使用して依存関係をインストールします。

    ```bash
    uv sync
    ```

## 設定

プロジェクトルートにある `config.yml` ファイルを編集して、ブログのフィードURLやSNSのAPIキーを設定します。

### `config.yml` の例

```yaml
blog:
  feed_url: "https://example.com/feed" # あなたのブログのRSS/AtomフィードURLを設定してください

sns:
  x: # X (旧Twitter) の設定
    consumer_key: "YOUR_CONSUMER_KEY" # X Developer Portalで取得
    consumer_secret: "YOUR_CONSUMER_SECRET" # X Developer Portalで取得
    access_token: "YOUR_ACCESS_TOKEN" # X Developer Portalで取得
    access_token_secret: "YOUR_ACCESS_TOKEN_SECRET" # X Developer Portalで取得
```

### 各種APIキーの取得方法

#### X (旧Twitter)

1.  [X Developer Portal](https://developer.twitter.com/en/portal/dashboard) にアクセスし、開発者アカウントをセットアップします。
2.  新しいプロジェクトとAppを作成します。
3.  Appの設定で、必要な権限（例: `Read and Write`）を付与します。
4.  `Keys and tokens` セクションから、`Consumer Key (API Key)`、`Consumer Secret (API Secret)`、`Access Token`、`Access Token Secret` を取得し、`config.yml` に設定します。

## 使い方

設定が完了したら、以下のコマンドでツールを実行できます。

```bash
uv run blog-autopost
```

初回実行時には、ブログの既存記事がすべて検出され、SNSにポストされます。2回目以降は、前回実行時以降に新しく投稿された記事のみが検出され、ポストされます。

## プラグインの追加

`src/plugins/` ディレクトリに新しいPythonファイルを作成することで、他のSNSへのポスト機能を追加できます。

新しいプラグインは、`src/plugins/__init__.py` で定義されている `SocialMediaPlugin` クラスを継承し、`post` メソッドを実装する必要があります。

例: `src/plugins/your_sns.py`

```python
from . import SocialMediaPlugin

class YourSns(SocialMediaPlugin):
    def __init__(self, api_key, api_secret):
        # ここにSNSの初期化処理を記述
        self.api_key = api_key
        self.api_secret = api_secret

    def post(self, title: str, link: str):
        # ここにSNSへの投稿ロジックを記述
        print(f"'{title}' を {link} で YourSns に投稿しました。")
```

`config.yml` に新しいSNSの設定を追加することで、ツールが自動的にプラグインを読み込みます。

```yaml
sns:
  your_sns: # プラグイン名と一致させる
    api_key: "YOUR_API_KEY"
    api_secret: "YOUR_API_SECRET"
```

## 開発者向け

### テスト

(TODO: テストの記述)

### コミットメッセージ規約

このプロジェクトでは、以下のコミットメッセージ規約を使用します。

- `feat`: 新機能やクラス、関数などを追加した時
- `fix`: バグを修正した時
- `chore`: コードへの影響が無い変更の時（ビルド・リリースなど）
- `docs`: ドキュメントを変更した時
- `style`: コードの意味的に影響がない変更の時（空白、フォーマット、セミコロンの欠落など）
- `refactor`: バグ修正、機能追加を行わないコード変更の時
- `perf`: パフォーマンス改善するコード変更の時
- `test`: テストの追加や修正の時
- `etc`: その他の変更の時

例: `[feat] ユーザー登録機能の追加`
