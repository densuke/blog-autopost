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
2. 「アプリケーション名」を入力し、必要な権限を有効にします：
   - **基本権限**: `write:statuses` （投稿作成用）
   - **メディア権限**: `write:media` （メディア添付機能用）
3. アプリ作成後、「アクセストークン」が表示されるのでコピーします。
4. `config.yml` の `sns.mastodon.access_token` に貼り付けてください。
5. `sns.mastodon.instance_url` には、あなたが利用するMastodonインスタンスのURL（例: https://mastodon.social）を設定してください。

**メディア添付機能について:**
- メディア添付機能を使用する場合、`write:media` スコープが必要です
- この権限がないと `This action is outside the authorized scopes` エラーが発生します
- 既存のアプリケーションでも権限を変更できますが、新しいアクセストークンの生成が必要です

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

#### Mastodonの複数インスタンス使用時の注意

複数のMastodonインスタンス（例：mastodon.socialとmstdn.jp）に同時に投稿する場合、**フェデレーション機能**により以下の現象が発生する可能性があります：

- 異なるインスタンスからの投稿が同じタイムラインに表示される
- 一見すると「重複投稿」のように見える場合がある
- これはMastodonの正常な仕様です

**回避方法：**
```bash
# 特定のインスタンスのみに投稿
uv run -m src.main --text "投稿内容" --sns mastodon-social

# または設定で使用するインスタンスを1つに限定
```

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

### Blueskyリンクカード機能

Blog AutoPost CLIは、Blueskyでのリッチなリンクプレビュー表示をサポートしています。

#### 機能概要

- **自動リンクカード生成**: ブログ記事の投稿時に自動でリンクカードを作成
- **画像付きプレビュー**: 記事の画像をサムネイルとして表示
- **メタデータ抽出**: 記事のタイトル、説明文を自動取得
- **アフィリエイト画像除外**: 設定により不適切な画像を自動除外

#### 画像取得戦略

リンクカード用の画像は以下の優先順位で自動取得されます：

1. **featured_image**: RSS内のenclosure/media:contentタグ
2. **first_content_image**: 記事本文の最初の画像
3. **og_image**: ページのOGP画像（Open Graph Protocol）
4. **default**: 設定で指定したデフォルト画像

#### 設定方法

```yaml
blog:
  feed_url: "https://example.com/feed"
  # Blueskyリンクカード機能用の画像設定
  image_settings:
    default_image: ""                    # デフォルト画像URL (空の場合は画像なし)
    enable_link_cards: true             # リンクカード機能を有効にする
    image_strategy:                     # 画像取得の優先順位
      - "featured_image"                # RSS内のenclosure/media:content
      - "first_content_image"           # 記事本文の最初の画像
      - "og_image"                      # ページのOGP画像
      - "default"                       # デフォルト画像
    image_filters:
      exclude_domains:                  # 除外ドメイン（アフィリエイト対策）
        - "amazon.co.jp"
        - "affiliate.rakuten.co.jp"
        - "px.a8.net"
        - "images-amazon.com"
      min_width: 200                    # 最小幅（ピクセル）
      min_height: 200                   # 最小高さ（ピクセル）
```

#### デフォルト画像の設定

```yaml
blog:
  image_settings:
    default_image: "https://yourblog.com/images/default-card.png"
    # または相対パス（推奨しません）
    # default_image: "/images/default-card.png"
```

#### 使用例

```bash
# リンクカード機能が有効な場合の通常実行
uv run -m src.main

# リンクカード機能付きドライラン
uv run -m src.main --dry-run --debug

# 特定記事数のみでテスト
uv run -m src.main --dry-run --limit 1
```

#### 動作の仕組み

1. **記事検出**: RSS/Atomフィードから新着記事を取得
2. **画像抽出**: 設定された戦略に従って画像URLを取得
3. **メタデータ取得**: 記事ページからタイトル・説明文を抽出
4. **リンクカード作成**: BlueskyのAPI形式でリンクカードを生成
5. **投稿実行**: テキストとリンクカードを組み合わせて投稿

#### 注意事項

- **Bluesky専用機能**: 現在はBlueskyのみリンクカード表示対応
- **画像サイズ制限**: Blueskyは1MBまでの画像をサポート
- **フィルタリング**: アフィリエイト関連の画像は自動除外
- **フォールバック**: 画像取得に失敗した場合は通常のテキスト投稿
- **デバッグ推奨**: 初回設定時は `--debug --dry-run` での動作確認を推奨

### URL短縮機能

Blog AutoPost CLIは、SNSの文字数制限に対応するためのURL短縮機能を提供しています。

#### 機能概要

- **自動URL短縮**: 投稿テキストが文字数制限を超過した場合、自動的にURLを短縮
- **文字数制限対応**: 各SNSの文字数制限を自動チェック
- **無料サービス**: is.gd APIを使用（APIキー不要）
- **フォールバック**: 短縮失敗時は元URLを使用

#### SNS別文字数制限

| SNS | 文字数制限 | 設定可能 |
|-----|------------|----------|
| X (旧Twitter) | 280文字 | ❌ |
| Bluesky | 300文字 | ❌ |
| Mastodon | 500文字 | ✅ |
| Misskey | 3000文字 | ✅ |

#### 投稿テキスト最適化

1. **標準形式**: `{title} {link}`で投稿
2. **文字数超過時**: URLを短縮して再チェック
3. **再度超過時**: タイトルをトリミングして`{title}... {short_link}`形式

#### 設定方法

```yaml
# URL短縮機能の設定
url_shortening:
  enabled: true           # URL短縮機能を有効にする
  service: "is.gd"       # 使用するサービス（現在はis.gdのみ）
  mode: "auto"           # 短縮動作モード

# URL短縮動作モード
# - "always": 常にURL短縮を実行
# - "auto": 文字数制限超過時のみ短縮（デフォルト）
# - "never": 文字数制限超過でもそのまま投稿

# SNS別文字数制限のカスタマイズ
character_limits:
  mastodon: 500          # Mastodonの文字数制限
  misskey: 3000          # Misskeyの文字数制限
```

#### 使用例

```yaml
# 例: 長いタイトルの記事投稿
# 元のURL: https://blog.example.com/posts/2025/07/very-long-article-title-about-technology
# 短縮後: https://is.gd/abc123
# 投稿テキスト: "長いタイトルの記事について... https://is.gd/abc123"
```

## 各種APIキーの取得方法

#### X (旧Twitter)

1.  [X Developer Portal](https://developer.twitter.com/en/portal/dashboard) にアクセスし、開発者アカウントをセットアップします。
2.  新しいプロジェクトとAppを作成します。
3.  Appの設定で、必要な権限を付与します：
    - **基本権限**: `Read and Write` （テキスト投稿用）
    - **メディア権限**: `Read and Write` （メディア添付機能を使用する場合は必須）
4.  `Keys and tokens` セクションから、`Consumer Key (API Key)`、`Consumer Secret (API Secret)`、`Access Token`、`Access Token Secret` を取得し、`config.yml` に設定します。

**メディア添付機能について:**
- メディア添付機能を使用する場合、**Elevated access**の申請と承認が必要です
- 申請には X API の使用目的を詳細に記述する必要があります
- 承認までに数日～数週間かかる場合があります
- **注意**: 個人的な用途では承認が困難な場合があります

**Elevated access申請方法:**
1. [X Developer Portal](https://developer.twitter.com/en/portal/dashboard) のメインダッシュボードで「Apply for Elevated access」をクリック
2. 使用目的、API使用方法、データの取り扱いについて詳細に記述
3. 承認後、Access Token の再生成が必要な場合があります

**一時的な対応:**
- Elevated access取得まではXでのメディア添付は利用できません
- `--sns` オプションでX以外のSNSのみを指定して使用してください
- 例: `uv run -m src.main --text "画像付き投稿" --media image.jpg --sns bluesky,mastodon`

#### Bluesky (AT Protocol)

Blueskyに投稿するには、`identifier`（ユーザー名またはメールアドレス）と`password`（**アプリパスワード**の使用を強く推奨）が必要です。

**アプリパスワードの取得方法:**

1.  Blueskyのウェブサイトまたは公式アプリにログインします。
2.  設定画面に移動します。
3.  「App Passwords」または「アプリパスワード」のような項目を探します。
4.  新しいアプリパスワードを生成します。このパスワードは一度しか表示されないため、安全な場所に控えてください。
5.  生成されたアプリパスワードを`config.yml`の`sns.bluesky.password`に設定します。

**メディア添付機能について:**
- Blueskyのメディア添付機能は画像のみ対応（JPEG, PNG, GIF, WebP）
- 標準のアプリパスワードでメディアアップロードが可能です
- 追加の権限設定は不要です

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
   - **基本権限**: `ノートを作成・削除する` （投稿用）
   - **メディア権限**: `ドライブを操作する` （メディア添付機能用）
8. 「作成」ボタンをクリックしてトークンを生成します。
9. 生成されたトークンをコピーして、`config.yml`の`sns.misskey.access_token`に設定します。

**メディア添付機能について:**
- メディア添付機能を使用する場合、「ドライブを操作する」権限が必要です
- この権限がないと `PERMISSION_DENIED` エラーが発生します
- 既存のトークンに権限を追加することはできないため、新しいトークンを作成してください

**注意**: アクセストークンは秘密情報なので、公開リポジトリにコミットしないよう注意してください。

#### センシティブコンテンツ設定

Misskeyでは、メディアファイルのアップロード時にセンシティブ（NSFW）フラグを設定できます：

```yaml
sns:
  misskey:
    instance_url: "https://misskey.io"
    access_token: "YOUR_MISSKEY_ACCESS_TOKEN"
    is_sensitive: false  # メディアファイルをセンシティブコンテンツとしてマークするかどうか（デフォルト: false）
```

**is_sensitiveオプション:**
- `true`: アップロードするすべてのメディアファイルがセンシティブコンテンツとしてマークされます
- `false`: 通常のメディアファイルとしてアップロードされます（デフォルト）
- 省略時: `false`として動作します

この設定により、成人向けコンテンツや注意が必要な画像を適切にマークして投稿できます。

`config.yml`の例:

```yaml
sns:
  misskey:
    instance_url: "https://misskey.io" # 使用するMisskeyインスタンスのURL
    access_token: "YOUR_MISSKEY_ACCESS_TOKEN" # 生成したアクセストークン
    is_sensitive: false  # センシティブコンテンツ設定（オプション）
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
- `--text <テキスト>`: 指定したテキストを直接SNSに投稿（RSS監視をスキップ）
- `--sns <SNS名,SNS名>`: 投稿するSNSを限定（カンマ区切りで複数指定可能）
- `--list-sns`: 登録されているSNSアカウントの一覧を表示
- `--optimize`: 直接投稿時にもテキスト最適化（URL短縮など）を適用
- `--media <ファイル>`: 投稿にメディアファイルを添付（複数回指定可能）
- `--sensitive`: Misskeyでメディアファイルをセンシティブコンテンツとしてマーク（設定ファイルの値を一時的に上書き）

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

# 直接テキストを投稿
uv run -m src.main --text "こんにちは、ブログを更新しました！"

# 特定のSNSのみに投稿
uv run -m src.main --text "X専用の投稿です" --sns x

# 複数のSNSを指定して投稿
uv run -m src.main --text "MastodonとBlueskyに投稿" --sns mastodon,bluesky

# 直接投稿のドライラン
uv run -m src.main --text "テスト投稿" --dry-run

# 登録されているSNSアカウント一覧を表示
uv run -m src.main --list-sns

# 直接投稿でURL短縮機能を使用
uv run -m src.main --text "長いURLです https://example.com/very-long-url" --optimize

# 特定のSNSのみに最適化投稿
uv run -m src.main --text "長いテキスト..." --sns x --optimize

# メディア付き投稿
uv run -m src.main --text "画像付き投稿です" --media image.jpg

# 複数メディア付き投稿
uv run -m src.main --text "複数の画像です" --media photo1.jpg --media photo2.png --media video.mp4

# メディア付きドライラン
uv run -m src.main --text "テスト投稿" --media image.jpg --dry-run

# 特定SNSにメディア付き投稿
uv run -m src.main --text "Mastodon専用画像" --media image.jpg --sns mastodon

# Misskeyでセンシティブコンテンツとして投稿
uv run -m src.main --text "センシティブな画像です" --media nsfw.jpg --sns misskey --sensitive

# 設定ファイルの値を一時的に上書きしてセンシティブ投稿
uv run -m src.main --text "注意が必要な内容" --media image.jpg --sensitive
```

### 直接テキスト投稿機能

`--text`オプションを使用することで、RSS/Atomフィードの監視をスキップして、指定したテキストを直接SNSに投稿できます。この機能は以下のような場面で役立ちます：

- ブログ以外の内容を投稿したい場合
- 文字数制限でエラーが発生した場合の部分的な再投稿
- テスト投稿やお知らせの投稿

#### メディア添付機能

`--media`オプションを使用することで、テキスト投稿にメディアファイル（画像・動画・音声）を添付できます：

```bash
# 単一メディア添付
uv run -m src.main --text "画像付き投稿" --media photo.jpg

# 複数メディア添付（複数回指定）
uv run -m src.main --text "複数メディア" --media image1.jpg --media image2.png --media video.mp4
```

##### SNS別対応状況

| SNS | 画像 | 動画 | 音声 | 最大数 | 混在 | 備考 |
|-----|------|------|------|--------|------|------|
| X | ⚠️ | ⚠️ | ⚠️ | 画像4枚/動画1本 | ❌ | **Elevated access要申請** |
| Bluesky | ✅ | ❌ | ❌ | 4枚 | ❌ | 画像のみ対応 |
| Mastodon | ✅ | ✅ | ✅ | 4個 | ✅ | 全形式対応 |
| Misskey | ✅ | ✅ | ✅ | 16個 | ✅ | 全形式対応 |

##### 対応ファイル形式

- **画像**: JPEG, PNG, GIF, WebP
- **動画**: MP4, MOV, WebM
- **音声**: MP3, M4A, AAC, WAV, OGG, FLAC

##### 自動変換機能

- **m4a音声ファイル**: X向けに無音黒画面付きMP4に自動変換
- **事前バリデーション**: 投稿前にSNS別制限をチェック
- **エラー回避**: 対応できない組み合わせは事前に警告表示

##### 注意事項

- **ffmpegが必要**: 音声ファイル変換時にffmpegが必要です
- **ファイルサイズ制限**: 各SNSのファイルサイズ制限が適用されます
- **権限設定**: メディア添付機能を使用するには、各SNSで適切な権限設定が必要です
- **事前検証**: 投稿前に組み合わせ制限をチェックし、適切な警告を表示します

##### メディア添付機能の現在の対応状況

| SNS | 対応状況 | 必要な設定 |
|-----|----------|------------|
| **Bluesky** | ✅ 完全対応 | 標準のアプリパスワード |
| **Mastodon** | ✅ 完全対応 | `write:media` スコープ |
| **Misskey** | ✅ 完全対応 | 「ドライブを操作する」権限 |
| **X** | ⚠️ 要申請 | **Elevated access申請が必要** |

##### 権限設定のトラブルシューティング

メディア添付でエラーが発生する場合：

1. **X**: 
   - エラー: `403 Forbidden` または `oauth1 app permissions`
   - 解決: **Elevated access**の申請が必要（承認まで数日～数週間）
   - 一時対応: `--sns bluesky,mastodon,misskey` でX以外を使用

2. **Mastodon**: 
   - エラー: `This action is outside the authorized scopes`
   - 解決: `write:media` スコープを含むアクセストークンを新規作成

3. **Misskey**: 
   - エラー: `PERMISSION_DENIED`
   - 解決: 「ドライブを操作する」権限を含むアクセストークンを新規作成

4. **Bluesky**: 
   - 通常は追加設定不要（標準のアプリパスワードで対応）

#### SNS限定機能

`--sns`オプションと組み合わせることで、投稿するSNSを限定できます：

- `--sns x`: Xのみに投稿
- `--sns mastodon,bluesky`: MastodonとBlueskyのみに投稿
- `--sns misskey-io,mastodon-social`: 特定のアカウント名を指定して投稿

#### SNSアカウント一覧表示

`--list-sns`オプションを使用することで、現在設定されているSNSアカウントの一覧を確認できます：

```bash
uv run -m src.main --list-sns
```

この機能は以下のような情報を表示します：
- 設定形式（配列形式/オブジェクト形式）
- アカウント名とSNS種別
- 認証情報の設定状況
- Mastodon/Misskeyのインスタンス情報

**注意事項:**
- `--text`オプション単体使用時は、URL短縮機能やannouncement_text機能は適用されません
- `--optimize`オプションと組み合わせることで、直接投稿時でもURL短縮機能を利用可能
- 直接指定したテキストがそのまま投稿されます（--optimize未使用時）
- 各SNSの文字数制限は適用されるため、事前に確認してください

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
