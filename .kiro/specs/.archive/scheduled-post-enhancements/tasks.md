# 実装計画: 予約投稿機能の拡張

- [x] 1. バックエンドの機能拡張
- [x] 1.1 `scheduled_post_store`にソート機能を追加する
  - `get_all_posts` メソッドを修正し、`sort_by` 引数を追加する。
  - `sort_by` の値（`date_asc`, `date_desc`, `status_failed`, `status_completed`）に基づいて投稿リストをソートするロジックを実装する。
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 1.2 投稿リスト取得用の新しいJSON APIエンドポイントを作成する
  - `main_web.py` に `GET /api/posts` エンドポイントを新設する。
  - このエンドポイントは `sort_by` クエリパラメータを受け付け、ソートされた投稿リストをJSON形式で返すようにする。
  - _Requirements: 3.1, 3.2_

- [x] 1.3 既存のルートエンドポイントを修正する
  - `main_web.py` の `GET /` エンドポイントを修正し、`sort_by` クエリパラメータを受け付けるようにする。
  - サーバーサイドでの初回レンダリング時に、指定されたソート順で投稿リストを表示する。
  - _Requirements: 1.1, 1.3_

- [x] 2. フロントエンドの機能実装
- [x] 2.1 ソート選択用のUIをHTMLに追加する
  - `templates/index.html` の予約投稿一覧テーブルの上に、ソートオプションを選択するための `<select>` ドロップダウンメニューを配置する。
  - _Requirements: 1.2_

- [x] 2.2 投稿リストを非同期で更新するJavaScript関数を実装する
  - `fetchAndRenderPosts(sortBy)` という名前のJavaScript関数を作成する。
  - この関数は、`/api/posts` エンドポイントから投稿データを取得し、HTMLテーブルの `<tbody>` の内容を動的に再構築する。
  - _Requirements: 1.3, 3.2_

- [x] 2.3 ソートUIのイベントハンドラを実装する
  - ソート用ドロップダウンの `change` イベントを捕捉するイベントリスナーを追加する。
  - イベント発生時、選択されたソート順で `fetchAndRenderPosts` 関数を呼び出す。
  - _Requirements: 1.3_

- [x] 2.4 定期的な自動更新機能を実装する
  - `setInterval` を使用して、5分ごとに `fetchAndRenderPosts` 関数を現在のソート順で呼び出すタイマーをセットする。
  - _Requirements: 3.1_

- [x] 3. テストと検証
- [x] 3.1 バックエンドの単体テストを追加する
  - `scheduled_post_store.py` のソート機能に対する単体テストを作成し、すべてのソートオプションが正しく動作することを確認する。
  - _Requirements: 1.1, 1.2, 1.3_

- [x] 3.2 APIの結合テストを追加する
  - FastAPIの `TestClient` を使用して、`GET /api/posts` が `sort_by` パラメータに応じて正しいデータを返すことを検証するテストを追加する。
  - _Requirements: 3.1, 3.2_

- [x] 3.3 データクリーンアップ要件の検証
  - 手動テストで `scheduled_posts.json` からエントリを削除し、次回のUI更新（自動または手動）でその投稿が一覧から消えることを確認する。
  - この動作は既存の設計でカバーされているはずだが、念のため確認する。
  - _Requirements: 2.1, 2.2_