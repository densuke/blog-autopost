# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

このファイルを参照するときは、グラウンドルールとして ~/.claude/CLAUDE.md を参照してください。
ワークスペースなど隔離空間で作業しているために読み込めない場合は、利用者にその旨をお知らせください。読めそうな場所に持ち込むよう努力します。

## やりとりをする言語について

このファイルを参照できる場所にある環境では、日本語での会話を原則としてください。
ただ、プログラミング上で発生するエラーメッセージなどは英語のままで表記し、かっこ付けで日本語訳を付けるようにしてください。


## プロジェクト概要

Blog AutoPost CLIは、ブログのRSS/Atomフィードを監視し、新しい記事が投稿された際に各種SNS（X、Bluesky、Misskey、Mastodon）へ自動投稿するPythonツールです。

## 開発環境とコマンド

### セットアップ
```bash
uv sync  # 依存関係のインストール
```

### 実行コマンド
```bash
# 通常実行
uv run -m src.main

# ドライラン（テスト実行）
uv run -m src.main --dry-run

# デバッグ情報付きドライラン
uv run -m src.main --debug --dry-run

# 記事数制限付きドライラン
uv run -m src.main --dry-run --limit 2

# カスタム設定ファイル使用
uv run -m src.main --config custom_config.yml
```

### テスト実行
```bash
# 全テスト実行
uv run pytest

# カバレッジ付きテスト実行
uv run pytest --cov=src

# 特定テストファイル実行
uv run pytest tests/test_main.py

# 詳細出力でテスト実行
uv run pytest -v
```

## アーキテクチャ概要

### コア構成要素

**ArticleManager** (`src/article_manager.py`):
- RSS/Atomフィードの解析
- 新着記事の検出とデータ永続化
- 投稿テキストの生成（アナウンス文機能含む）

**ConfigManager** (`src/config_manager.py`):
- YAML設定ファイル（`config.yml`）の読み込み・管理
- SNS認証情報とブログフィードURLの管理

**Plugin System** (`src/plugin_loader.py`, `src/plugins/`):
- プラグインアーキテクチャによるSNS投稿機能
- 各SNSは個別のプラグインクラスとして実装
- `SocialMediaPlugin`基底クラスを継承し`post(title, link)`メソッドを実装

### プラグイン実装例
- `x.py`: X (旧Twitter) 投稿
- `bluesky.py`: Bluesky投稿
- `misskey.py`: Misskey投稿
- `mastodon.py`: Mastodon投稿

### データ管理
- 記事データ: `data/articles.json` - 既投稿記事の追跡用
- 設定: `config.yml` - SNS認証情報とフィード設定

## 重要な設計パターン

### プラグインの動的読み込み
`plugin_loader.py:load_plugins()`は設定ファイルのSNSセクションを基に、対応するプラグインモジュールを動的にインポートし、クラス名の規約（plugin名の先頭を大文字化）に従ってインスタンス化します。

### 記事の重複排除
`ArticleManager`は前回実行時の記事リストと最新記事を比較し、新着記事のみを抽出します。初回実行時はすべての記事が検出されるため、`--dry-run`での事前確認を推奨します。

### エラーハンドリング
各SNSプラグインでの投稿エラーは個別にキャッチされ、他のSNSへの投稿継続を保証します。

## 注意事項

- エントリーポイントの設定に問題があるため`uv run blog-autopost`は使用不可
- `uv run -m src.main`での実行を使用すること
- 設定ファイル`config.yml`はテンプレート（`config.yml.template`）からコピーして作成
- APIキーなどの秘密情報はgitに含めないよう注意
