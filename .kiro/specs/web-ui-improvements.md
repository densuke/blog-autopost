# Web UI改善仕様書

## プロジェクト概要
Blog AutoPost CLIのWeb UIを改善し、投稿記録の削除機能を強化する。

## 背景・課題
- 送信済みのメッセージや失敗したメッセージ記録が永続的に表示される
- 削除は個別実行のみで、複数件削除が効率的でない
- JSONベースのデータ保管により、CRUDと大規模データ処理が非効率

## 実装優先度

### Phase 0: SQLite移行（データレイヤー統合）
**優先度: 1（必須・基盤整備）**

後続フェーズの効率化のため、データレイヤーをJSON → SQLiteに統一。
既存APIとの互換性を維持し、段階的移行を実現。

#### タスク一覧
- [ ] Task 0.1: SQLAlchemy ORM モデル定義
  - ScheduledPost モデル定義（既存JSONスキーマ互換）
  - インデックス設定（created_at, status, target_sns）
  
- [ ] Task 0.2: SQLite初期化・マイグレーション
  - db/scheduled_posts.db 作成ロジック
  - JSON → SQLite移行スクリプト実装
  - 既存データの自動マイグレーション
  
- [ ] Task 0.3: DAO層実装
  - ScheduledPostDAO クラス実装
  - CRUD操作（create, read, update, delete）
  - 一括削除メソッド（batch_delete）
  
- [ ] Task 0.4: 既存APIの統合
  - main_web.py のエンドポイント動作確認
  - scheduled_post_store 互換性ラッパー実装
  - 後方互換性テスト

**完了条件:**
- 既存JSONデータが自動でSQLiteに移行される
- 全既存APIが変更なしで動作
- データ整合性が保証される

---

### Phase 1: 一括削除API実装
**優先度: 2（削除機能・コア機能）**

バックエンド一括削除エンドポイント実装。Phase 0のSQLiteに基づき効率的に実装。

#### タスク一覧
- [ ] Task 1.1: POST /api/scheduled-posts/batch-delete エンドポイント
  - 複数ID受け取り（JSON array）
  - SQL DELETE WHERE id IN (...) 実装
  - トランザクション処理・ロールバック対応
  - レスポンス：{ "deleted_count": N }
  
- [ ] Task 1.2: バリデーション・エラーハンドリング
  - 入力値チェック（ID形式、権限確認）
  - エラーレスポンス統一
  - ロギング（削除件数、削除者情報）

**完了条件:**
- 複数IDを受け取り、安全に一括削除
- エラー時のロールバック機能
- APIドキュメント作成

---

### Phase 2: フロントエンド一括削除UI実装
**優先度: 2（削除機能・UI）**

チェックボックスと一括削除ボタン追加。

#### タスク一覧
- [ ] Task 2.1: HTMLテーブル修正
  - 「すべて選択」チェックボックス（ヘッダ行）
  - 各行にチェックボックス追加
  
- [ ] Task 2.2: 削除操作UI追加
  - 「選択したものを削除」ボタン
  - 選択件数表示
  
- [ ] Task 2.3: JavaScript実装
  - チェック状態管理（全選択/解除）
  - 一括削除APIの呼び出し
  - 確認ダイアログ表示
  - 削除後のテーブル更新（リロード）

**完了条件:**
- チェックボックスで複数件選択可能
- 「選択したものを削除」で一括削除
- UX確認（確認ダイアログ、エラーメッセージ）

---

### Phase 3: ページネーション実装（Future）
**優先度: 3（次フェーズ）**

SQLite実装後、ページネーション追加。
- 初期表示: 10件/ページ
- ページサイズ変更可能
- 総件数表示

### Phase 4: フィルター機能実装（Future）
**優先度: 4（次フェーズ）**

ステータス別・SNS別フィルター追加。

---

## 技術スタック

### バックエンド
- **ORM**: SQLAlchemy
- **DB**: SQLite（ファイルベース）
- **マイグレーション**: 自動スクリプト（Alembic検討中）

### フロントエンド
- **UI**: 既存HTMLテーブル拡張
- **JavaScript**: Vanilla JS（外部ライブラリ不使用）

---

## ファイル変更一覧

### 新規作成
- `src/web/models.py` - SQLAlchemy ORM モデル定義
- `src/web/dao.py` - DAO層（CRUD操作）
- `scripts/migrate_json_to_sqlite.py` - マイグレーションスクリプト

### 変更対象
- `src/web/main_web.py` - 新エンドポイント追加、既存API統合
- `src/web/templates/index.html` - チェックボックス・一括削除UI追加
- `src/web/scheduled_post_store.py` - 互換性ラッパー（必要に応じて）

### 関連ファイル
- `data/articles.json` → `data/scheduled_posts.db` に段階的移行

---

## テスト戦略

### Unit Tests
- DAO層 CRUD操作テスト
- マイグレーション成功確認

### Integration Tests
- API エンドポイントテスト
- フロントエンド UI動作テスト

### Manual Tests
- 既存データ自動マイグレーション確認
- 大量データでのパフォーマンス確認

---

## リスク・考慮事項

1. **既存データ互換性**: JSON → SQLite移行時のデータ検証
2. **同時アクセス**: SQLiteの並行性制限（通常使用では問題なし）
3. **バックアップ**: DB ファイルのバージョン管理

---

## 完了条件

- Phase 0: SQLite移行完了、既存API全動作確認
- Phase 1: 一括削除API実装完了、テスト完了
- Phase 2: フロントエンド一括削除UI完成、UX確認完了
