# Web UI改善プロジェクト - 完成

## 概要
Blog AutoPost CLIのWeb UIを改善。削除機能の強化とSQLite移行を完了。

## ブランチ
`feature/web-ui-improvements` (作成済み・実装完了)

## 実装完了内容

### Phase 0: SQLite移行（データレイヤー統合） ✅
- src/web/models.py: SQLAlchemy ORM モデル定義
- src/web/dao.py: DAO層実装（CRUD操作・一括削除対応）
- scripts/migrate_json_to_sqlite.py: JSON → SQLite マイグレーション
- src/web/scheduled_post_store_sqlite.py: SQLite互換実装

### Phase 1: 一括削除API実装 ✅
- POST /api/scheduled-posts/batch-delete エンドポイント
- 複数IDを受け取り一括削除
- トランザクション処理・ロールバック対応

### Phase 2: フロントエンド一括削除UI実装 ✅
- チェックボックス列（各テーブル行）
- 「すべて選択」チェックボックス（ヘッダ）
- 「選択したものを削除」ボタン
- 「すべて選択」「すべて解除」ボタン
- 選択件数表示
- 確認ダイアログ
- 動的テーブル更新時の再初期化対応

## 主な改善点

### 削除機能
- ✅ 個別削除（既存）+ 一括削除（新規）
- ✅ 複数件を効率的に削除可能
- ✅ 確認ダイアログで誤削除防止

### UX改善
- ✅ チェックボックスで複数選択
- ✅ 操作ボタン（削除・全選択・全解除）
- ✅ 選択件数のリアルタイム表示
- ✅ 選択があるときのみボタン表示

### パフォーマンス
- ✅ JSONファイル全読み込みから SQL WHERE IN() へ
- ✅ ページネーション対応済み（Phase 3で活用可）
- ✅ インデックス最適化

## 今後の拡張
- Phase 3: ページネーション実装（10/20/50件選択可能）
- Phase 4: ステータス・SNS別フィルター実装

## 使用方法

### マイグレーション実行
```bash
python scripts/migrate_json_to_sqlite.py
# または
python scripts/migrate_json_to_sqlite.py --json-path data/scheduled_posts.json --db-path data/scheduled_posts.db
```

### Web UI 使用方法
1. 予約投稿一覧表示
2. 各行のチェックボックスで選択（複数可）
3. 「選択したものを削除」ボタンで一括削除
4. 確認ダイアログで確認
