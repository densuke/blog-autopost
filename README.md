# Blog AutoPost CLI

Blog AutoPost CLIは、指定したブログの更新を定期的にチェックし、新しい記事が投稿された場合に各種SNS（X、Bluesky、Misskey）へ自動的にポストするコマンドラインツールです。

## 概要

このツールは、以下の機能を提供します。

- RSS/Atomフィードを介したブログの更新チェック
- 新着記事の検出と記録
- プラグイン形式による各種SNS（X、Bluesky、Misskey）への自動ポスト

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

    **注意**: 現在、エントリーポイントの設定により、`uv run blog-autopost`での実行は正常に動作しません。代わりに`uv run -m src.main`を使用してください。

## 設定

プロジェクトルートにある `config.yml.template` ファイルをコピーして `config.yml` を作成します。
そして、以下の内容を編集してブログのフィードURLやSNSのAPIキーを設定します。


### `config.yml` の例

```yaml
# SNS投稿時のアナウンス文。不要な場合は""（空文字列）にするか、この行を削除してください。
announcement_text: "ブログを更新しました！"

blog:
  feed_url: "https://example.com/feed" # あなたのブログのRSS/AtomフィードURLを設定してください

sns:
  x: # X (旧Twitter) の設定
    consumer_key: "CONSUMER_KEY" # X Developer Portalで取得
    consumer_secret: "CONSUMER_SECRET" # X Developer Portalで取得
    access_token: "ACCESS_TOKEN" # X Developer Portalで取得
    access_token_secret: "ACCESS_TOKEN_SECRET" # X Developer Portalで取得
  bluesky: # Blueskyの設定
    identifier: "YOUR_BLUESKY_IDENTIFIER" # あなたのBlueskyハンドル（例: @yourhandle.bsky.social）または登録メールアドレス
    password: "YOUR_BLUESKY_APP_PASSWORD" # 生成したアプリパスワード
  misskey: # Misskeyの設定
    instance_url: "https://misskey.io" # 使用するMisskeyインスタンスのURL
    access_token: "YOUR_MISSKEY_ACCESS_TOKEN" # 生成したアクセストークン
  mastodon: # Mastodonの設定
    instance_url: "https://mastodon.social" # 使用するMastodonインスタンスのURL
    access_token: "YOUR_MASTODON_ACCESS_TOKEN" # 生成したアクセストークン

---

## Mastodonアクセストークンの取得方法

1. MastodonのWebサイトでログインし、右上の「ユーザー設定」→「開発」→「新機アプリ」をクリックします。
2. 「アプリケーション名」を入力し、必要に応じて権限（"投稿の作成" など）を有効にします。
3. アプリ作成後、「アクセストークン」が表示されるのでコピーします。
4. `config.yml` の `sns.mastodon.access_token` に貼り付けてください。
5. `sns.mastodon.instance_url` には、あなたが利用するMastodonインスタンスのURL（例: https://mastodon.social）を設定してください。

### 例
```yaml
sns:
  mastodon:
    instance_url: "https://mastodon.social"
    access_token: "取得したアクセストークン"
```

---

### 複数アカウント対応

Blog AutoPost CLIは、同一のSNSサービスで複数のアカウントにポストする機能をサポートしています。

#### 配列形式の設定（複数アカウント）

複数のアカウントを設定する場合は、`sns`セクションを配列形式で記述します：

```yaml
sns:
  - type: mastodon
    name: "mastodon-main"
    instance_url: "https://mastodon.social"
    access_token: "メインアカウントのトークン"
  - type: mastodon
    name: "mstdn-jp"
    instance_url: "https://mstdn.jp"
    access_token: "mstdn.jpアカウントのトークン"
  - type: x
    name: "x-personal"
    consumer_key: "個人アカウントのコンシューマーキー"
    consumer_secret: "個人アカウントのコンシューマーシークレット"
    access_token: "個人アカウントのアクセストークン"
    access_token_secret: "個人アカウントのアクセストークンシークレット"
  - type: x
    name: "x-business"
    consumer_key: "ビジネスアカウントのコンシューマーキー"
    consumer_secret: "ビジネスアカウントのコンシューマーシークレット"
    access_token: "ビジネスアカウントのアクセストークン"
    access_token_secret: "ビジネスアカウントのアクセストークンシークレット"
```

#### 設定項目

- `type`: SNSの種類（`x`, `bluesky`, `misskey`, `mastodon`）
- `name`: アカウントの識別名（同一`type`内で一意である必要があります）
- その他: 各SNSで必要な認証情報

#### 後方互換性

従来のオブジェクト形式の設定も引き続きサポートされます：

```yaml
sns:
  mastodon:
    instance_url: "https://mastodon.social"
    access_token: "あなたのアクセストークン"
  x:
    consumer_key: "CONSUMER_KEY"
    consumer_secret: "CONSUMER_SECRET"
    access_token: "ACCESS_TOKEN"
    access_token_secret: "ACCESS_TOKEN_SECRET"
```

オブジェクト形式の場合、`name`は自動的に`type`名が使用されます。

## 各種APIキーの取得方法

#### X (旧Twitter)

1.  [X Developer Portal](https://developer.twitter.com/en/portal/dashboard) にアクセスし、開発者アカウントをセットアップします。
2.  新しいプロジェクトとAppを作成します。
3.  Appの設定で、必要な権限（例: `Read and Write`）を付与します。
4.  `Keys and tokens` セクションから、`Consumer Key (API Key)`、`Consumer Secret (API Secret)`、`Access Token`、`Access Token Secret` を取得し、`config.yml` に設定します。

#### Bluesky (AT Protocol)

Blueskyに投稿するには、`identifier`（ユーザー名またはメールアドレス）と`password`（**アプリパスワード**の使用を強く推奨）が必要です。

**アプリパスワードの取得方法:**

1.  Blueskyのウェブサイトまたは公式アプリにログインします。
2.  設定画面に移動します。
3.  「App Passwords」または「アプリパスワード」のような項目を探します。
4.  新しいアプリパスワードを生成します。このパスワードは一度しか表示されないため、安全な場所に控えてください。
5.  生成されたアプリパスワードを`config.yml`の`sns.bluesky.password`に設定します。

`config.yml`の例:

```yaml
sns:
  bluesky:
    identifier: "YOUR_BLUESKY_IDENTIFIER" # あなたのBlueskyハンドル（例: @yourhandle.bsky.social）または登録メールアドレス
    password: "YOUR_BLUESKY_APP_PASSWORD" # 生成したアプリパスワード
```

#### Misskey

Misskeyに投稿するには、インスタンスURLとアクセストークンが必要です。

**アクセストークンの取得方法:**

1. Misskeyインスタンス（例: https://misskey.io）にログインします。
2. 右上のユーザーアイコンをクリックし、「設定」を選択します。
3. 左側のメニューから「API」を選択します。
4. 「アクセストークン」タブを選択します。
5. 「アクセストークンを作成」ボタンをクリックします。
6. トークンの名前を入力（例: 「Blog Auto Post」）します。
7. 必要な権限を選択します：
   - `ノートを作成・削除する` - 投稿するために必要
8. 「作成」ボタンをクリックしてトークンを生成します。
9. 生成されたトークンをコピーして、`config.yml`の`sns.misskey.access_token`に設定します。

**注意**: アクセストークンは秘密情報なので、公開リポジトリにコミットしないよう注意してください。

`config.yml`の例:

```yaml
sns:
  misskey:
    instance_url: "https://misskey.io" # 使用するMisskeyインスタンスのURL
    access_token: "YOUR_MISSKEY_ACCESS_TOKEN" # 生成したアクセストークン
```

**他のMisskeyインスタンスを使用する場合:**
`instance_url`を変更することで、misskey.io以外のMisskeyインスタンスでも使用できます。

```yaml
sns:
  misskey:
    instance_url: "https://your-misskey-instance.com"
    access_token: "そのインスタンスで生成したアクセストークン"
```

## 使い方

設定が完了したら、以下のコマンドでツールを実行できます。

### 基本的な実行

```bash
uv run -m src.main
```

### オプション

- `--config <ファイル名>`: 設定ファイルを指定（デフォルト: config.yml）
- `--dry-run`: 実際にSNSに投稿せず、投稿内容のみを表示
- `--limit <数>`: 処理する記事数を制限（新しい記事から指定した数まで）
- `--debug`: 詳細なデバッグ情報を表示

### 使用例

```bash
# 通常の実行
uv run -m src.main

# ドライラン（テスト実行）
uv run -m src.main --dry-run

# 最新2記事のみをドライランで確認
uv run -m src.main --dry-run --limit 2

# デバッグ情報を表示して実行
uv run -m src.main --debug --dry-run
```

### 初回実行について

初回実行時には、ブログの既存記事がすべて検出され、SNSにポストされます。2回目以降は、前回実行時以降に新しく投稿された記事のみが検出され、ポストされます。

初回実行時に大量の記事を投稿したくない場合は、`--dry-run`オプションを使用して記事リストを保存してから、実際の投稿を開始することをお勧めします。

## 対応SNS

このツールは現在、以下のSNSに対応しています：

- **X (旧Twitter)**: consumer_key、consumer_secret、access_token、access_token_secretが必要
- **Bluesky**: identifier（ユーザー名またはメールアドレス）とpassword（アプリパスワード推奨）が必要
- **Misskey**: instance_urlとaccess_tokenが必要
- **Mastodon**: instance_urlとaccess_tokenが必要

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
