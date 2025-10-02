# Implementation Plan

- [x] 1. CLIコマンドの追加
- [x] 1.1 `main.py` に新しいサブコマンド `touch-rss-posted` を追加する。
  - `argparse` を使用してサブコマンドを定義する。
  - サブコマンドのヘルプメッセージを記述する。
  - _Requirements: 1.1_

- [x] 1.2 `main.py` で `touch-rss-posted` サブコマンドが実行された際に、`ArticleManager` の新しいメソッドを呼び出すロジックを追加する。
  - _Requirements: 1.1_

- [x] 2. `ArticleManager` の機能拡張
- [x] 2.1 `src/article_manager.py` に `force_mark_all_as_posted` メソッドを追加する。
  - このメソッドは `config_manager` からRSSフィードのURLリストを取得する。
  - _Requirements: 1.1, 2.1_

- [x] 2.2 `force_mark_all_as_posted` メソッド内で、各RSSフィードからすべての記事のURLを取得する。
  - `feedparser` を使用してRSSフィードをパースする。
  - _Requirements: 1.1, 2.1_

- [x] 2.3 `data/processed_articles.json` に `__forced_posted_urls__` キーを追加し、取得したすべての記事URLをリストとして保存するロジックを実装する。
  - JSONファイルの読み込み、更新、書き込み処理を実装する。
  - _Requirements: 1.1, 2.1_

- [x] 2.4 既存のRSSフィード処理ロジック (`ArticleManager` 内の `get_new_articles` など) が `__forced_posted_urls__` リストを参照し、リストに含まれるURLを新しい記事として扱わないように修正する。
  - _Requirements: 1.2, 2.2_

- [x] 3. ユーザーへのフィードバック
- [x] 3.1 `main.py` で `touch-rss-posted` コマンドの実行結果（成功/失敗、処理された記事数など）をユーザーに表示する。
  - _Requirements: 3.1_

- [x] 4. テストの追加
- [x] 4.1 `tests/test_article_manager.py` に `force_mark_all_as_posted` メソッドのユニットテストを追加する。
  - モックの `config_manager` と `feedparser` を使用して、JSONファイルが正しく更新されることを検証する。
  - _Requirements: 1.1, 2.1_

- [x] 4.2 `tests/test_main.py` に `touch-rss-posted` サブコマンドの結合テストを追加する。
  - `subprocess` を使用してCLIコマンドを実行し、期待される出力とJSONファイルの状態を検証する。
  - `--dry-run` オプションが正しく機能し、ファイルが変更されないことを検証するテストを追加する。
  - _Requirements: 1.1, 2.1, 3.1_

- [x] 5. `justfile` の更新
- [x] 5.1 `justfile` に `touch-rss-posted` コマンドを実行するためのエントリを追加する。
  - _Requirements: 1.1_