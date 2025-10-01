# 技術スタック (tech.md)

## 1. アーキテクチャ

このプロジェクトは、Pythonで記述されたコマンドラインインターフェース（CLI）アプリケーションです。主要なコンポーネントは以下の通りです。

- **コアロジック (`main.py`)**: コマンドライン引数の解析、設定の読み込み、処理の振り分けを担当します。
- **設定管理 (`config_manager.py`)**: `config.yml` ファイルから設定を読み込み、アプリケーション全体で利用可能にします。
- **記事管理 (`article_manager.py`)**: RSS/Atomフィードの解析、新着記事の検出、処理済み記事の永続化（JSON形式）を担当します。
- **プラグインローダー (`plugin_loader.py`)**: `src/plugins` ディレクトリからSNS投稿プラグインを動的に読み込み、初期化します。
- **SNSプラグイン群 (`src/plugins/*.py`)**: 各SNSへの投稿ロジックをカプセル化したモジュール群です。

## 2. 使用技術

### 2.1. 言語

- **Python 3.12+**

### 2.2. 主要ライブラリ

`pyproject.toml` で管理されています。

- **`feedparser`**: RSS/Atomフィードの解析。
- **`requests`**: HTTPリクエスト（リンクカードのメタデータ取得など）。
- **`beautifulsoup4`**: HTMLの解析（OGP画像などの抽出）。
- **`pyyaml`**: `config.yml` の読み込み。
- **`tweepy`**: X (旧Twitter) APIとの連携。
- **`atproto`**: Bluesky (AT Protocol) APIとの連携。
- **`Pillow`**: 画像のリサイズ処理。
- **`pytest`**: ユニットテストの実行。
- **`pytest-cov`**: テストカバレッジの計測。

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

このプロジェクトでは、`config.yml` ファイルを使用して設定を管理しており、直接的な環境変数の使用は必須ではありません。ただし、CI/CD環境などでは、`config.yml` の内容を環境変数経由で動的に生成することが想定されます。

## 6. ポート設定

このアプリケーションはCLIツールであり、サーバーとして動作する機能はないため、特定のポートを使用することはありません。
