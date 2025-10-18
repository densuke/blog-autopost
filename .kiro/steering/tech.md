# 技術スタック (tech.md)

## 1. アーキテクチャ

このプロジェクトは、Pythonで記述されたCLIツールとFastAPIベースのWebアプリケーションを併せ持つ構成です。主要なコンポーネントは以下の通りです。

- **CLI (`main.py`)**: コマンドライン引数の解析、設定の読み込み、フィード監視・直接投稿・メンテナンスコマンドの実行を担います。
- **コンテンツ取得 (`article_manager.py`)**: RSS/Atomフィードの解析、新着記事検出、処理済み記事の永続化（JSON）を担当します。
- **設定管理 (`config_manager.py`)**: `config.yml`を読み込み、SNS認証情報の環境変数上書きやWebサーバー設定、セッション用シークレットキーを提供します。
- **共通投稿ロジック (`src/web/core_posting_logic.py`, `src/web/posting_service.py`)**: CLIとWeb UIの双方から利用される投稿パイプライン。プラグインのロード、テキスト最適化、メディア処理をラップします。
- **Web UI (`src/web/main_web.py`, `src/web/runner.py`)**: FastAPIアプリ本体とUvicorn起動エントリーポイント。ログイン画面、テンプレート描画、予約投稿API、APSchedulerの起動をまとめています。
- **スケジューラと永続化 (`src/web/scheduler_service.py`, `src/web/post_executor.py`, `src/web/scheduled_post_store_sqlite.py`, `src/web/dao.py`, `src/web/models.py`)**: APSchedulerで予約投稿を監視し、SQLite + SQLAlchemy経由で予約情報を永続化します。JSONベースの`ScheduledPostStore`は互換用に残存します。
- **SNSプラグイン群 (`src/plugins/*.py`)**: 各SNSへの投稿ロジックをカプセル化したモジュール群。X/Bluesky/Threads/Misskey/Mastodon/Tumblrなどに対応しています。

## 2. 使用技術

### 2.1. 言語

- **Python 3.12+**

### 2.2. 主要ライブラリ

`pyproject.toml` で管理されています。

- **フィード解析 / スクレイピング**: `feedparser`（RSS/Atomの解析）、`requests`（HTTPアクセス）、`beautifulsoup4`（OGP抽出など）。
- **SNS連携**: `tweepy`（X API）、`atproto`（Bluesky API）。その他のSNSはプラグイン内の直接HTTP実装で対応します。
- **メディア処理**: `Pillow`（画像リサイズ）。音声→動画変換などはシステム依存の`ffmpeg`を呼び出します。
- **Webアプリケーション**: `fastapi`（API本体）、`uvicorn[standard]`（開発用ASGIサーバー）、`python-multipart`（ファイルアップロード）、`jinja2`（HTMLテンプレート）、`itsdangerous`（セッション署名）。
- **スケジューラ / 永続化**: `apscheduler`（バックグラウンドジョブ管理）、`sqlalchemy`（SQLiteベースの予約投稿ストア）。
- **構成管理**: `pyyaml`（設定ファイル読み込み）。
- **テスト**: `pytest`, `pytest-cov`。

### 2.3. パッケージ管理

- **`uv`**: 高速なPythonパッケージインストーラおよびリゾルバ。依存関係のインストールや仮想環境の管理に使用します。

## 3. 開発環境

### 3.1. 必須ツール

- **Python 3.12** 以降
- **`uv`**: `pip install uv` でインストール。
- **`git`**: バージョン管理。
- **`just`**: コマンドランナー（オプションだが推奨）。
- **`ffmpeg`**: 音声ファイルを動画に変換するために必要（メディア投稿機能）。

### 3.2. セットアップ手順

1. リポジトリをクローン:
   ```bash
   git clone <repository_url>
   cd blog-autopost
   ```
2. 仮想環境の作成と依存関係のインストール:
   ```bash
   uv sync
   ```

## 4. 主要なコマンド

`justfile` に主要な開発コマンドが定義されています。

- **ブログ更新チェックと投稿**:
  ```bash
  just blog-check
  # または uv run -m src.main
  ```
- **テキストの直接投稿**:
  ```bash
  just post-text "投稿したい内容"
  # または uv run -m src.main --text "投稿したい内容"
  ```
- **ドライラン実行**:
  ```bash
  just dry-run
  # または uv run -m src.main --dry-run
  ```
- **Web UIの起動**:
  ```bash
  just run-web
  # または uv run -m src.web.runner
  ```
- **テストの実行**:
  ```bash
  just test
  # または uv run pytest
  ```
- **依存関係の同期**:
  ```bash
  just sync
  # または uv sync
  ```

## 5. 環境変数

このプロジェクトでは、`config.yml` ファイルを使用して設定を管理しており、直接的な環境変数の使用は必須ではありません。ただし、CI/CD環境などでは、`config.yml` の内容を環境変数経由で動的に生成することが想定されます。Web UIを利用する場合は`web_auth.username` / `web_auth.password` / `web_auth.secret_key`を設定する必要があります。SNS認証情報は`X_CONSUMER_KEY`などの環境変数で上書き可能です。

## 6. ポート設定

FastAPI Web UIを起動する場合は、`config.yml`の`web_server.host` / `web_server.port`設定に従ってサーバーが起動します（デフォルトは`127.0.0.1:8000`）。CLIのみを利用する場合はポートは使用しません。
