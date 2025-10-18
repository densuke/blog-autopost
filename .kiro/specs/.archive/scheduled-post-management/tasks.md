# Implementation Plan

- [x] 1. 予約投稿管理機能のWeb APIを実装する
- [x] 1.1 `ScheduledPostAPI`のエンドポイントを定義する
  - GET `/scheduled-posts` (全予約投稿取得)
  - GET `/scheduled-posts/{id}` (特定予約投稿取得)
  - POST `/scheduled-posts` (予約投稿作成)
  - PUT `/scheduled-posts/{id}` (予約投稿更新)
  - DELETE `/scheduled-posts/{id}` (予約投稿削除)
  - POST `/scheduled-posts/{id}/re-execute` (予約投稿再実行)
  - POST `/scheduled-posts/{id}/send-now` (予約投稿即時送信)
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 2.1, 2.2, 2.3, 2.4, 3.1, 3.2, 3.3, 4.1, 4.2, 4.3, 5.1, 5.2, 5.3_

- [x] 1.2 `ScheduledPostAPI`の入力バリデーションとエラーハンドリングを実装する
  - リクエストボディの形式検証
  - 必須フィールドの欠落チェック
  - SNS制限違反チェック
  - 認証されていないリクエストの拒否 (401)
  - 予約投稿が見つからない場合のエラー (404)
  - 実行済み投稿の編集・削除・即時送信拒否 (409)
  - 成功済み投稿の再実行拒否 (409)
  - _Requirements: 2.4, 3.3, 4.3, 5.3, Error Strategy, Error Categories and Responses_

- [x] 2. 予約投稿データストア (`ScheduledPostStore`) を実装する
- [x] 2.1 `data/scheduled_posts.json` ファイルの読み書き機能を実装する
  - JSONファイルの存在チェックと初期化
  - 予約投稿リストの読み込みと保存
  - _Requirements: Decision: 予約投稿データの永続化にJSONファイルを採用する_

- [x] 2.2 `ScheduledPostStore`のCRUD操作メソッドを実装する
  - `get_all_posts()`: 全予約投稿を取得
  - `get_post_by_id(post_id)`: 特定予約投稿を取得
  - `create_post(post)`: 予約投稿を作成
  - `update_post(post_id, updates)`: 予約投稿を更新
  - `delete_post(post_id)`: 予約投稿を削除
  - _Requirements: ScheduledPostStore Contract Definition_

- [x] 2.3 `ScheduledPost`ドメインモデルを定義する
  - `id`, `scheduled_at`, `content`, `media_files`, `target_sns`, `status`, `error_message`, `created_at`, `updated_at` フィールド
  - `scheduled_at`の未来日時制約
  - `status`による操作制限
  - _Requirements: Domain Model: ScheduledPost_

- [x] 3. 投稿実行サービス (`PostExecutor`) を実装する
- [x] 3.1 `PostExecutor`の`execute_post`メソッドを実装する
  - `CorePostingLogic` (既存の `main.py` の投稿処理をラップしたもの) を呼び出す
  - 投稿結果に基づいて `ScheduledPostStore` のステータスを更新する (成功/失敗)
  - _Requirements: PostExecutor Contract Definition, 4.1, 4.2, 5.2_

- [x] 4. スケジューラー (`Scheduler`) バックグラウンドプロセスを実装する
- [x] 4.1 `APScheduler` を利用して定期実行ロジックを実装する
  - `ScheduledPostStore` を定期的に監視し、実行日時が来た予約投稿を検出
  - 検出した予約投稿を `PostExecutor` に渡す
  - _Requirements: Decision: 予約投稿の実行にバックグラウンドスケジューラー (`APScheduler`) を導入する, Scheduler Contract Definition_

- [x] 4.2 スケジューラープロセスの起動と停止メカニズムを実装する
  - Webアプリケーションとは独立したプロセスとして起動
  - プロセス停止時の復旧ロジック (未実行の過去の予約投稿の検出と実行)
  - _Requirements: Scheduler Contract Definition_

- [x] 5. Web UIとの統合と表示ロジックを実装する
- [x] 5.1 予約投稿の一覧表示機能をWeb UIに実装する
  - 各予約投稿の投稿日時、内容の概要、対象SNS、現在のステータスを表示
  - 失敗した予約投稿の強調表示
  - _Requirements: 1.1, 1.3, 1.4_

- [x] 5.2 予約投稿の編集・取り消し・再実行・即時送信機能をWeb UIに実装する
  - 編集フォームの表示とデータバインディング
  - 確認ダイアログの表示
  - APIエンドポイントへのリクエスト送信
  - _Requirements: 2.1, 2.2, 2.3, 3.1, 3.2, 4.1, 4.2, 5.1, 5.2_

- [ ] 6. 既存のコア投稿ロジックのラップと再利用
- [x] 6.1 既存の `src/main.py` の投稿処理を `CorePostingLogic` としてラップする
  - `SNSPlugins`, `MediaServices`, `TextOptimizer` を再利用
  - _Requirements: Existing Architecture Analysis, Architecture Integration_

- [x] 7. テストと品質保証
- [x] 7.1 各コンポーネントのユニットテストを実装する
  - `ScheduledPostStore`, `ScheduledPostAPI`, `PostExecutor`, `Scheduler`
  - _Requirements: Unit Tests_

- [x] 7.2 統合テストとE2Eテストを実装する
  - Web UIからの一連の操作のテスト
  - スケジュール実行のテスト
  - _Requirements: Integration Tests, E2E/UI Tests_

- [x] 8. 認証・認可とセキュリティ対策
- [x] 8.1 予約投稿APIへの認証・認可を実装する
  - 既存の `src/web/auth_service.py` を利用
  - _Requirements: 401 (Unauthorized), Security Considerations_

- [x] 8.2 入力バリデーションとデータ保護を強化する
  - `scheduled_posts.json` のファイルパーミッション設定
  - メディアファイルの安全な保存
  - _Requirements: Security Considerations_

- [x] 9. ログと監視
- [x] 9.1 エラーロギングと監視機能を実装する
  - Webアプリケーションのエラーログ出力
  - スケジューラーと `PostExecutor` の実行ログ出力
  - _Requirements: Monitoring_