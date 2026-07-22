# Implementation Tasks

## Phase 1: テストエラーの修正（優先度: Critical）

### Task 1.1: ScheduledPostStoreSQLite 対応のフィクスチャ修正

**目的**: test_scheduled_post_api.py の 27 個のエラーを解消し、ScheduledPostStoreSQLite への移行に対応させる

**実装ステップ**:
1. `tests/test_scheduled_post_api.py` のフィクスチャ `clear_scheduled_posts_file` を確認
2. ScheduledPostStoreSQLite の API を調査（file_path 属性の有無）
3. フィクスチャを SQLite 対応に書き換え（DB初期化ロジック）
4. 全27個のテストを個別に実行し、エラー解消を確認

**受け入れ基準**:
- [ ] `clear_scheduled_posts_file` フィクスチャが ScheduledPostStoreSQLite に対応
- [ ] test_scheduled_post_api.py のエラーが 27 個 → 0 個に削減
- [ ] `uv run pytest tests/test_scheduled_post_api.py -v` が全て PASS

**依存関係**: なし

**推定工数**: 4-6 時間

**変更ファイル**:
- `tests/test_scheduled_post_api.py`

**検証コマンド**:
```bash
uv run pytest tests/test_scheduled_post_api.py -v
```

---

### Task 1.2: test_scheduler_service.py のエラー修正

**目的**: test_scheduler_service.py の 3 個のエラーを解消する

**実装ステップ**:
1. エラーメッセージを確認（AttributeError 関連）
2. ScheduledPostStore のモック化を ScheduledPostStoreSQLite に対応
3. テストフィクスチャを更新
4. 3 個のテストケースを個別に実行

**受け入れ基準**:
- [ ] test_scheduler_service.py のエラーが 3 個 → 0 個に削減
- [ ] `uv run pytest tests/test_scheduler_service.py -v` が全て PASS

**依存関係**: Task 1.1

**推定工数**: 2-3 時間

**変更ファイル**:
- `tests/test_scheduler_service.py`

**検証コマンド**:
```bash
uv run pytest tests/test_scheduler_service.py -v
```

---

## Phase 2: 失敗テストの修正（優先度: Critical）

### Task 2.1: test_force_mark_all_as_posted_writes_to_file 失敗修正

**目的**: test_article_manager.py::test_force_mark_all_as_posted_writes_to_file の失敗を解消する

**実装ステップ**:
1. エラーメッセージを確認（FileNotFoundError の原因特定）
2. 一時ファイルパスの取り扱いを修正
3. pytest の `tmp_path` フィクスチャを適切に使用
4. テストを実行し、成功を確認

**受け入れ基準**:
- [ ] test_force_mark_all_as_posted_writes_to_file が PASS
- [ ] 一時ファイルが正しく作成・読み込まれる

**依存関係**: なし

**推定工数**: 1-2 時間

**変更ファイル**:
- `tests/test_article_manager.py`

**検証コマンド**:
```bash
uv run pytest tests/test_article_manager.py::test_force_mark_all_as_posted_writes_to_file -v
```

---

### Task 2.2: test_access_root_unauthenticated 失敗修正

**目的**: test_web_app.py::test_access_root_unauthenticated の失敗を解消する

**実装ステップ**:
1. 認証フローの実装を確認（現在は 200、期待値は 303）
2. 認証なしアクセス時のリダイレクト処理を検証
3. テストの期待値またはコード実装を修正
4. テストを実行し、成功を確認

**受け入れ基準**:
- [ ] test_access_root_unauthenticated が PASS
- [ ] 認証フローの挙動が仕様通り

**依存関係**: なし

**推定工数**: 2-3 時間

**変更ファイル**:
- `tests/test_web_app.py` または `src/web/main_web.py`

**検証コマンド**:
```bash
uv run pytest tests/test_web_app.py::test_access_root_unauthenticated -v
```

---

### Task 2.3: 全テストクリーン確認

**目的**: Phase 1-2 の修正後、全テストが合格することを確認する

**実装ステップ**:
1. 全テストを実行
2. エラー・失敗が 0 であることを確認
3. カバレッジレポートを生成

**受け入れ基準**:
- [ ] 全テストが PASS（エラー 0、失敗 0）
- [ ] `uv run pytest` で 156 テスト全て成功

**依存関係**: Task 1.1, 1.2, 2.1, 2.2

**推定工数**: 1 時間

**検証コマンド**:
```bash
uv run pytest -v
uv run pytest --cov=src --cov-report=term-missing
```

---

## Phase 3: 低カバレッジモジュールのテスト拡充（優先度: High）

### Task 3.1: src/main.py のテスト拡充

**目的**: src/main.py のカバレッジを 25% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認（欠けている部分を特定）
2. CLI 引数パーステストを追加（--dry-run、--debug、--limit、--text など）
3. 各実行モードのテストを追加
4. エラーハンドリングのテストを追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] `--dry-run` モードのテスト
- [ ] `--debug` モードのテスト
- [ ] `--limit N` オプションのテスト
- [ ] `--text` 直接投稿のテスト
- [ ] `--config` カスタム設定ファイルのテスト
- [ ] 無効な引数のエラーハンドリング
- [ ] RSS フィード取得失敗時の処理
- [ ] SNS 投稿失敗時の処理

**受け入れ基準**:
- [ ] src/main.py のカバレッジが 70% 以上
- [ ] 新規テストが 20-30 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 8-12 時間

**変更ファイル**:
- `tests/test_main.py` (既存)
- 新規追加テストファイル（必要に応じて）

**検証コマンド**:
```bash
uv run pytest tests/test_main.py -v
uv run pytest --cov=src/main --cov-report=term-missing
```

---

### Task 3.2: src/article_manager.py のテスト拡充

**目的**: src/article_manager.py のカバレッジを 27% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. RSS/Atom フィード解析テストを追加
3. 新着記事検出ロジックのテスト追加
4. 重複排除メカニズムのテスト追加
5. JSON 永続化のテスト追加
6. カバレッジを計測

**テスト追加項目**:
- [ ] RSS フィード解析の成功ケース
- [ ] Atom フィード解析の成功ケース
- [ ] フィード解析失敗時のエラーハンドリング
- [ ] 新着記事 0 件の場合のテスト
- [ ] 新着記事複数件の場合のテスト
- [ ] 重複記事の検出と排除
- [ ] JSON ファイル読み込み・書き込み
- [ ] 既存記事の更新処理

**受け入れ基準**:
- [ ] src/article_manager.py のカバレッジが 70% 以上
- [ ] 新規テストが 15-20 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 6-8 時間

**変更ファイル**:
- `tests/test_article_manager.py`

**検証コマンド**:
```bash
uv run pytest tests/test_article_manager.py -v
uv run pytest --cov=src/article_manager --cov-report=term-missing
```

---

### Task 3.3: src/image_resizer.py のテスト拡充

**目的**: src/image_resizer.py のカバレッジを 26% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. 画像リサイズの成功・失敗ケーステストを追加
3. ファイル形式変換のテスト追加
4. エラーハンドリングのテスト追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] 画像リサイズ成功（PNG、JPEG、GIF）
- [ ] 画像リサイズ失敗（不正なファイル形式）
- [ ] ファイルサイズ制限チェック
- [ ] アスペクト比保持の検証
- [ ] ファイル形式変換（PNG → JPEG など）
- [ ] Pillow エラーのハンドリング
- [ ] 一時ファイルのクリーンアップ

**受け入れ基準**:
- [ ] src/image_resizer.py のカバレッジが 70% 以上
- [ ] 新規テストが 15-20 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 6-8 時間

**変更ファイル**:
- 新規: `tests/test_image_resizer.py`（存在しない場合）

**検証コマンド**:
```bash
uv run pytest tests/test_image_resizer.py -v
uv run pytest --cov=src/image_resizer --cov-report=term-missing
```

---

### Task 3.4: src/web/core_posting_logic.py のテスト拡充

**目的**: src/web/core_posting_logic.py のカバレッジを 16% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. コア投稿ロジックのテスト追加
3. エラーケースのテスト追加
4. フォールバック処理のテスト追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] 投稿成功ケース（全 SNS）
- [ ] 投稿失敗ケース（API エラー）
- [ ] リトライロジックのテスト
- [ ] 複数 SNS への投稿
- [ ] メディア添付投稿
- [ ] フォールバック処理
- [ ] エラーレスポンスのハンドリング

**受け入れ基準**:
- [ ] src/web/core_posting_logic.py のカバレッジが 70% 以上
- [ ] 新規テストが 20-25 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 8-10 時間

**変更ファイル**:
- 新規: `tests/test_core_posting_logic.py`（存在しない場合）

**検証コマンド**:
```bash
uv run pytest tests/test_core_posting_logic.py -v
uv run pytest --cov=src/web/core_posting_logic --cov-report=term-missing
```

---

### Task 3.5: src/web/scheduler_service.py のテスト拡充

**目的**: src/web/scheduler_service.py のカバレッジを 35% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. スケジューラー起動・停止のテスト追加
3. 定期実行ロジックのテスト追加
4. カバレッジを計測

**テスト追加項目**:
- [ ] スケジューラー起動の成功ケース
- [ ] スケジューラー停止の成功ケース
- [ ] ジョブ登録・削除
- [ ] 定期実行トリガー
- [ ] ジョブ実行失敗時の処理
- [ ] 並列実行制御

**受け入れ基準**:
- [ ] src/web/scheduler_service.py のカバレッジが 70% 以上
- [ ] 新規テストが 15-20 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 1.2, Task 2.3

**推定工数**: 6-8 時間

**変更ファイル**:
- `tests/test_scheduler_service.py`

**検証コマンド**:
```bash
uv run pytest tests/test_scheduler_service.py -v
uv run pytest --cov=src/web/scheduler_service --cov-report=term-missing
```

---

## Phase 4: プラグインテスト拡充（優先度: Medium）

### Task 4.1: src/plugins/bluesky.py のテスト拡充

**目的**: src/plugins/bluesky.py のカバレッジを 43% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. リンクカード生成テストを追加
3. OGP 抽出テストを追加
4. API 呼び出しエラーハンドリングテストを追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] リンクカード生成成功
- [ ] OGP 画像抽出成功
- [ ] OGP 画像抽出失敗時のフォールバック
- [ ] API タイムアウトのエラーハンドリング
- [ ] レート制限エラーのハンドリング
- [ ] メディア添付投稿

**受け入れ基準**:
- [ ] src/plugins/bluesky.py のカバレッジが 70% 以上
- [ ] 新規テストが 10-15 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 5-7 時間

**変更ファイル**:
- `tests/test_bluesky_plugin.py`

**検証コマンド**:
```bash
uv run pytest tests/test_bluesky_plugin.py -v
uv run pytest --cov=src/plugins/bluesky --cov-report=term-missing
```

---

### Task 4.2: src/plugins/mastodon.py と misskey.py のテスト拡充

**目的**: mastodon.py（40%）と misskey.py（45%）のカバレッジを 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. API 認証テストを追加
3. 投稿テストを追加
4. メディア添付テストを追加
5. カバレッジを計測

**テスト追加項目（各プラグイン）**:
- [ ] API 認証成功
- [ ] API 認証失敗のエラーハンドリング
- [ ] テキスト投稿成功
- [ ] メディア添付投稿成功
- [ ] 投稿失敗時のエラーハンドリング

**受け入れ基準**:
- [ ] src/plugins/mastodon.py のカバレッジが 70% 以上
- [ ] src/plugins/misskey.py のカバレッジが 70% 以上
- [ ] 各プラグインで新規テスト 10-15 個追加
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 8-10 時間（両プラグイン合計）

**変更ファイル**:
- `tests/test_mastodon_plugin.py`
- `tests/test_misskey_plugin.py`

**検証コマンド**:
```bash
uv run pytest tests/test_mastodon_plugin.py -v
uv run pytest tests/test_misskey_plugin.py -v
uv run pytest --cov=src/plugins/mastodon --cov=src/plugins/misskey --cov-report=term-missing
```

---

### Task 4.3: src/plugins/x.py のテスト拡充

**目的**: src/plugins/x.py のカバレッジを 61% → 70% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. レート制限テストを追加
3. メディア検証テストを追加
4. ツイート長制限テストを追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] レート制限エラーのハンドリング
- [ ] メディアサイズ検証
- [ ] ツイート長制限チェック
- [ ] ツイート省略処理

**受け入れ基準**:
- [ ] src/plugins/x.py のカバレッジが 70% 以上
- [ ] 新規テストが 5-10 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 3-5 時間

**変更ファイル**:
- `tests/test_x_plugin.py`

**検証コマンド**:
```bash
uv run pytest tests/test_x_plugin.py -v
uv run pytest --cov=src/plugins/x --cov-report=term-missing
```

---

## Phase 5: Web API・スケジューラーテスト拡充（優先度: Medium）

### Task 5.1: src/web/main_web.py のテスト拡充

**目的**: src/web/main_web.py のカバレッジを 64% → 80% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. 全 HTTP メソッド（GET、POST、PUT、DELETE）のテストを追加
3. 認証・認可チェックのテストを追加
4. カバレッジを計測

**テスト追加項目**:
- [ ] GET エンドポイントのテスト
- [ ] POST エンドポイントのテスト
- [ ] PUT エンドポイントのテスト
- [ ] DELETE エンドポイントのテスト
- [ ] 認証必須エンドポイントの認証チェック
- [ ] 認可チェック（権限不足のエラー）

**受け入れ基準**:
- [ ] src/web/main_web.py のカバレッジが 80% 以上
- [ ] 新規テストが 10-15 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 5-7 時間

**変更ファイル**:
- `tests/test_web_app.py`

**検証コマンド**:
```bash
uv run pytest tests/test_web_app.py -v
uv run pytest --cov=src/web/main_web --cov-report=term-missing
```

---

### Task 5.2: src/web/scheduled_post_store_sqlite.py のテスト拡充

**目的**: src/web/scheduled_post_store_sqlite.py のカバレッジを 40% → 80% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. CRUD 操作テストを追加
3. トランザクションテストを追加
4. エラーハンドリングテストを追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] Create 操作の成功ケース
- [ ] Read 操作の成功ケース
- [ ] Update 操作の成功ケース
- [ ] Delete 操作の成功ケース
- [ ] トランザクションのロールバック
- [ ] データベースエラーのハンドリング

**受け入れ基準**:
- [ ] src/web/scheduled_post_store_sqlite.py のカバレッジが 80% 以上
- [ ] 新規テストが 15-20 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 1.1, Task 2.3

**推定工数**: 6-8 時間

**変更ファイル**:
- `tests/test_scheduled_post_store.py` または新規作成

**検証コマンド**:
```bash
uv run pytest tests/test_scheduled_post_store.py -v
uv run pytest --cov=src/web/scheduled_post_store_sqlite --cov-report=term-missing
```

---

### Task 5.3: src/web/posting_service.py のテスト拡充

**目的**: src/web/posting_service.py のカバレッジを 67% → 80% に向上させる

**実装ステップ**:
1. 現在のテストカバレッジを確認
2. 投稿実行テストを追加
3. リトライロジックテストを追加
4. 複数 SNS 投稿テストを追加
5. カバレッジを計測

**テスト追加項目**:
- [ ] 投稿実行成功
- [ ] 投稿実行失敗時のリトライ
- [ ] リトライ回数上限到達時の処理
- [ ] 複数 SNS への同時投稿
- [ ] 一部 SNS 失敗時の処理

**受け入れ基準**:
- [ ] src/web/posting_service.py のカバレッジが 80% 以上
- [ ] 新規テストが 10-15 個追加される
- [ ] 全テストが PASS

**依存関係**: Task 2.3

**推定工数**: 5-7 時間

**変更ファイル**:
- `tests/test_posting_service.py`

**検証コマンド**:
```bash
uv run pytest tests/test_posting_service.py -v
uv run pytest --cov=src/web/posting_service --cov-report=term-missing
```

---

## Phase 6: コード品質向上（ruff/mypy）（優先度: Medium）

### Task 6.1: ruff による lint チェックと自動修正

**目的**: ruff による lint 警告を 0 にする

**実装ステップ**:
1. `uv run ruff check src/ tests/` を実行し、警告リストを取得
2. 自動修正可能な項目は `uv run ruff check --fix src/ tests/` で修正
3. 手動修正が必要な項目を個別に対応
4. 全警告が解消されたことを確認

**対応項目**:
- [ ] PEP 8 違反の修正（インデント、スペース）
- [ ] 未使用インポートの削除
- [ ] 命名規則の統一
- [ ] 行長制限の遵守（100 文字）

**受け入れ基準**:
- [ ] `uv run ruff check src/ tests/` で警告 0
- [ ] コードが PEP 8 に準拠

**依存関係**: なし（並行実施可能）

**推定工数**: 4-6 時間

**変更ファイル**:
- `src/` 配下の全 Python ファイル
- `tests/` 配下の全 Python ファイル

**検証コマンド**:
```bash
uv run ruff check src/ tests/
```

---

### Task 6.2: ruff による自動整形

**目的**: 全ソースコードを統一されたフォーマットに整形する

**実装ステップ**:
1. `uv run ruff format src/ tests/` を実行
2. フォーマット適用結果を確認
3. 全テストが引き続き PASS することを確認

**受け入れ基準**:
- [ ] 全ソースコードが統一フォーマット
- [ ] `uv run ruff format --check src/ tests/` でエラー 0
- [ ] 全テストが引き続き PASS

**依存関係**: Task 6.1

**推定工数**: 1-2 時間

**変更ファイル**:
- `src/` 配下の全 Python ファイル
- `tests/` 配下の全 Python ファイル

**検証コマンド**:
```bash
uv run ruff format --check src/ tests/
uv run pytest -v
```

---

### Task 6.3: mypy による型チェックと型ヒント追加

**目的**: mypy による型エラーを 0 にする

**実装ステップ**:
1. `uv run mypy src/` を実行し、型エラーリストを取得
2. 型ヒント不足の関数に型注釈を追加
3. 型エラーを個別に修正
4. 全型エラーが解消されたことを確認

**対応項目**:
- [ ] 関数シグネチャへの型ヒント追加（引数、戻り値）
- [ ] 変数への型注釈追加
- [ ] `Any` 型の排除（具体的な型に置き換え）
- [ ] 型エラーの修正

**受け入れ基準**:
- [ ] `uv run mypy src/` で型エラー 0
- [ ] 全関数に型ヒントが追加されている

**依存関係**: Task 6.1, 6.2

**推定工数**: 8-12 時間

**変更ファイル**:
- `src/` 配下の全 Python ファイル

**検証コマンド**:
```bash
uv run mypy src/
```

---

### Task 6.4: 古い Python 構文の現代化

**目的**: 古い Python 構文を Python 3.12+ の最新慣例に更新する

**実装ステップ**:
1. `str.format()` を f-string に変換
2. 型ヒントで `List`, `Dict` を `list`, `dict` に変更
3. その他の古い構文を最新版に更新
4. 全テストが引き続き PASS することを確認

**対応項目**:
- [ ] `str.format()` → f-string
- [ ] `typing.List` → `list`
- [ ] `typing.Dict` → `dict`
- [ ] その他の非推奨構文

**受け入れ基準**:
- [ ] 古い構文が全て最新版に更新されている
- [ ] 全テストが引き続き PASS

**依存関係**: Task 6.3

**推定工数**: 4-6 時間

**変更ファイル**:
- `src/` 配下の全 Python ファイル

**検証コマンド**:
```bash
uv run pytest -v
```

---

## Phase 7: 最終検証・CI/CD 統合（優先度: Medium）

### Task 7.1: カバレッジ目標達成の最終確認

**目的**: 全体カバレッジ 80% 以上、主要モジュール 70% 以上を達成していることを確認する

**実装ステップ**:
1. 全テストを実行
2. カバレッジレポートを生成
3. 各モジュールのカバレッジを確認
4. 目標未達のモジュールを特定し、追加テストを検討

**受け入れ基準**:
- [ ] 全体カバレッジ 80% 以上
- [ ] main.py カバレッジ 70% 以上
- [ ] article_manager.py カバレッジ 70% 以上
- [ ] plugins カバレッジ 70% 以上

**依存関係**: Phase 3, 4, 5 の全タスク

**推定工数**: 2-3 時間

**検証コマンド**:
```bash
uv run pytest --cov=src --cov-report=html --cov-report=term-missing
```

---

### Task 7.2: テストドキュメントの整備

**目的**: テストの目的、テストケース、期待結果を明確に記述する

**実装ステップ**:
1. 各テストファイルに docstring を追加
2. テスト関数に説明コメントを追加
3. README にテスト実行方法を追記

**受け入れ基準**:
- [ ] 全テストファイルに docstring が追加されている
- [ ] 各テスト関数に説明コメントが記載されている
- [ ] README にテスト実行方法が記載されている

**依存関係**: Task 7.1

**推定工数**: 3-4 時間

**変更ファイル**:
- `tests/` 配下の全テストファイル
- `README.md`

---

### Task 7.3: CI/CD パイプラインへの統合

**目的**: CI/CD パイプラインでテスト・ruff・mypy が自動実行されるようにする

**実装ステップ**:
1. GitHub Actions ワークフローファイルを作成
2. テスト実行ステップを追加
3. ruff チェックステップを追加
4. mypy チェックステップを追加
5. カバレッジレポート自動生成ステップを追加

**受け入れ基準**:
- [ ] GitHub Actions ワークフローが作成されている
- [ ] テスト、ruff、mypy が自動実行される
- [ ] 失敗時にビルドが停止される
- [ ] カバレッジレポートが自動生成される

**依存関係**: Task 7.1, 7.2

**推定工数**: 3-5 時間

**変更ファイル**:
- 新規: `.github/workflows/test.yml`

**検証方法**:
- GitHub Actions での実行確認

---

## タスク依存関係図

```
Phase 1 (Critical)
├── Task 1.1 (ScheduledPostStoreSQLite 対応)
├── Task 1.2 (scheduler_service エラー修正) [依存: 1.1]

Phase 2 (Critical)
├── Task 2.1 (article_manager 失敗修正)
├── Task 2.2 (web_app 失敗修正)
└── Task 2.3 (全テストクリーン確認) [依存: 1.1, 1.2, 2.1, 2.2]

Phase 3 (High) [依存: 2.3]
├── Task 3.1 (main.py テスト拡充)
├── Task 3.2 (article_manager.py テスト拡充)
├── Task 3.3 (image_resizer.py テスト拡充)
├── Task 3.4 (core_posting_logic.py テスト拡充)
└── Task 3.5 (scheduler_service.py テスト拡充)

Phase 4 (Medium) [依存: 2.3]
├── Task 4.1 (bluesky.py テスト拡充)
├── Task 4.2 (mastodon.py/misskey.py テスト拡充)
└── Task 4.3 (x.py テスト拡充)

Phase 5 (Medium) [依存: 2.3]
├── Task 5.1 (main_web.py テスト拡充)
├── Task 5.2 (scheduled_post_store_sqlite.py テスト拡充)
└── Task 5.3 (posting_service.py テスト拡充)

Phase 6 (Medium) [並行可能]
├── Task 6.1 (ruff lint チェック)
├── Task 6.2 (ruff 自動整形) [依存: 6.1]
├── Task 6.3 (mypy 型チェック) [依存: 6.1, 6.2]
└── Task 6.4 (構文現代化) [依存: 6.3]

Phase 7 (Medium) [依存: Phase 3, 4, 5, 6]
├── Task 7.1 (最終カバレッジ確認)
├── Task 7.2 (テストドキュメント整備) [依存: 7.1]
└── Task 7.3 (CI/CD 統合) [依存: 7.1, 7.2]
```

---

## 推定総工数

| フェーズ | 推定工数 |
|---------|---------|
| Phase 1 | 6-9 時間 |
| Phase 2 | 4-6 時間 |
| Phase 3 | 34-46 時間 |
| Phase 4 | 16-22 時間 |
| Phase 5 | 16-22 時間 |
| Phase 6 | 17-26 時間 |
| Phase 7 | 8-12 時間 |
| **合計** | **101-143 時間** |

---

## 実装推奨順序

1. **Week 1**: Phase 1-2（Critical タスクのクリア）
2. **Week 2-3**: Phase 3（低カバレッジモジュール拡充）
3. **Week 3-4**: Phase 4-5（プラグイン・Web API テスト拡充）
4. **Week 4-5**: Phase 6（コード品質向上）
5. **Week 5**: Phase 7（最終検証・CI/CD 統合）

---

## Success Criteria（再掲）

✅ 全テストが合格（エラー・失敗 0）
✅ 全体カバレッジ 80% 以上
✅ 主要モジュール 70% 以上
✅ ruff 警告 0
✅ mypy エラー 0
✅ Python 3.12+ 慣例準拠
✅ CI/CD パイプライン統合完了
