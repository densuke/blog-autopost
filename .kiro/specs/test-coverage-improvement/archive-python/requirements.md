# Requirements Document

## Introduction

Blog AutoPost CLIの全機能に対する信頼性の高い自動テストを構築し、コードカバレッジを向上させることで、バグの早期発見と安全なリファクタリングを実現する。現在のカバレッジ 49% から80% 以上への向上を目指し、特に重要なモジュールの信頼性を確保する。

## Requirements

### Requirement 1: 既存テストの修正と現代化

**Objective:** 現在失敗・エラーが発生しているテストを修正し、最新のコード実装に対応させる。また、テストの構造を最適化して保守性を高める。

#### Acceptance Criteria

1. WHEN テスト実行時に `AttributeError: 'ScheduledPostStoreSQLite' object has no attribute 'file_path'` エラーが発生する場合 THEN テストコード内のフィクスチャを ScheduledPostStoreSQLite に対応させるべき
2. WHEN テストで JsonStore から SQLite への移行に対応する場合 THEN 関連する全テストフィクスチャが正しくモック化されるべき
3. WHEN test_scheduled_post_api.py の 30 個のエラーをクリアする場合 THEN テストセットアップと実装の不一致を解決するべき
4. WHEN test_article_manager.py::test_force_mark_all_as_posted_writes_to_file テストが失敗する場合 THEN 一時ファイルの取り扱いを適切に実装するべき
5. WHEN test_web_app.py::test_access_root_unauthenticated テストが失敗する場合 THEN 認証フロー実装と期待値のズレを修正するべき

---

### Requirement 2: 低カバレッジモジュールのテスト拡充

**Objective:** カバレッジが 50% 以下の主要モジュールに対して、包括的なテストを実装する。特に CLI エントリーポイント、記事管理、メディア処理に焦点を当てる。

#### Acceptance Criteria

1. WHEN src/main.py（カバレッジ 25%）のテストを強化する場合 THEN コマンドライン引数のパース、各実行モード（dry-run、debug、limit など）に対するテストを追加するべき
2. WHEN src/article_manager.py（カバレッジ 27%）のテストを強化する場合 THEN RSS/Atom フィード解析、新着記事検出、重複排除ロジックのテストを追加するべき
3. WHEN src/image_resizer.py（カバレッジ 26%）のテストを強化する場合 THEN 画像リサイズ、ファイル形式変換、エラーハンドリングのテストを追加するべき
4. WHEN src/web/core_posting_logic.py（カバレッジ 16%）のテストを強化する場合 THEN コア投稿ロジック、エラーケース、フォールバック処理のテストを追加するべき
5. WHEN src/web/scheduler_service.py（カバレッジ 35%）のテストを強化する場合 THEN スケジューラー起動・停止、定期実行ロジックのテストを追加するべき

---

### Requirement 3: プラグインのカバレッジ向上

**Objective:** 各 SNS プラグインのテストカバレッジを向上させ、投稿ロジック、エラー処理、API レスポンスハンドリングの信頼性を高める。

#### Acceptance Criteria

1. WHEN src/plugins/bluesky.py（カバレッジ 43%）のテストを強化する場合 THEN リンクカード生成、OGP 抽出、API 呼び出しエラーハンドリングのテストを追加するべき
2. WHEN src/plugins/mastodon.py（カバレッジ 40%）と src/plugins/misskey.py（カバレッジ 45%）のテストを強化する場合 THEN API 認証、投稿、メディア添付のテストを追加するべき
3. WHEN src/plugins/x.py（カバレッジ 61%）のテストを強化する場合 THEN レート制限、メディア検証、ツイート長制限のテストを追加するべき
4. WHEN src/plugins/threads.py（カバレッジ 68%）のテストを強化する場合 THEN Thread API の特異的な要件（メディア形式など）に対応したテストを追加するべき

---

### Requirement 4: Web API とスケジューラー機能のテスト整備

**Objective:** Web UI 関連の API エンドポイント、スケジューラー統合、予約投稿管理機能の包括的なテストを構築する。

#### Acceptance Criteria

1. WHEN src/web/main_web.py（カバレッジ 64%）のテストを強化する場合 THEN 全エンドポイント（GET、POST、PUT、DELETE）に対するテストを追加するべき
2. WHEN src/web/scheduled_post_store_sqlite.py（カバレッジ 40%）のテストを強化する場合 THEN CRUD 操作、トランザクション、エラーハンドリングのテストを追加するべき
3. WHEN 予約投稿 API テスト（現在 30 エラー）を修正する場合 THEN SQLite ストアへの移行に合わせてテストコードを更新するべき
4. WHEN src/web/posting_service.py（カバレッジ 67%）のテストを強化する場合 THEN 投稿実行、リトライロジック、複数 SNS 投稿のテストを追加するべき

---

### Requirement 5: エッジケースとエラーハンドリングのテスト

**Objective:** 各モジュールのエラーハンドリング、例外処理、境界条件に対する包括的なテストを実装する。

#### Acceptance Criteria

1. WHEN ネットワークエラー、API タイムアウト、メディアファイルエラーをテストする場合 THEN 各リカバリーメカニズムが正しく動作することを確認するべき
2. WHEN 無効な入力（不正な URL、サポートされていない SNS、不正なメディア形式など）をテストする場合 THEN 適切なバリデーション実装と エラーメッセージが確認されるべき
3. WHEN キャラクターセット、言語対応をテストする場合 THEN 日本語、絵文字、特殊文字を含むテストケースを追加するべき
4. WHERE メディアファイルサイズ制限のテストを実施する場合 THE 各 SNS の制限値（ファイルサイズ、個数）に対応したテストを追加するべき

---

### Requirement 6: テスト品質とメンテナンス

**Objective:** テストコードの品質を高め、長期的な保守性を確保する。テスト構造を統一化し、ドキュメント化する。

#### Acceptance Criteria

1. WHEN テストコードを整理する場合 THEN テストケースの命名規則、フィクスチャの構造を統一するべき
2. WHEN テストのドキュメンテーションを追加する場合 THEN 各テストの目的、テストケース、期待結果を明確に記述するべき
3. WHEN パフォーマンステストが必要な場合 THEN 重い処理（API 呼び出し、ファイル I/O）のモック化とテスト時間最適化を実施するべき
4. WHEN CI/CD パイプラインでテストを実行する場合 THEN 全テストが安定して合格（PASS）し、カバレッジレポートが自動生成されるべき

---

### Requirement 7: テストカバレッジの目標設定と追跡

\*\*Objective:\*\* 段階的にカバレッジを向上させ、最終的に 80% 以上を達成する。各フェーズでの進捗を可視化する。

#### Acceptance Criteria

1. WHEN テストカバレッジの目標を設定する場合 THEN 現在 49% から段階的に 60%、70%、80% へ到達するべき
2. WHEN カバレッジレポートを実行する場合 THEN 各モジュール単位でのカバレッジ率が確認できるべき
3. WHEN カバレッジが向上しない箇所を特定する場合 THEN テスト追加が不可能な理由（コードの設計課題など）を記録するべき
4. WHILE テスト拡充を進める場合 THE 既存の合格テストが引き続き合格し続けるべき（リグレッション防止）

---

### Requirement 8: コード品質向上と型安全性（ruff/mypy）

**Objective:** ruff と mypy を uv 経由で実行し、コード品質を向上させ、型安全性を確保する。既存コードの現代化と Python の慣例への準拠を達成する。

#### Acceptance Criteria

1. WHEN `uv run ruff check src/ tests/` でコードをチェックする場合 THEN 全ての lint 警告が解決され、コードが PEP 8 に準拠するべき
2. WHEN `uv run ruff format src/ tests/` でコードを自動整形する場合 THEN 全ソースコードが統一されたフォーマットに整形されるべき
3. WHEN `uv run mypy src/` で型チェックを実行する場合 THEN 型エラーが 0 になるまで型ヒントを追加するべき
4. WHEN 古い Python 構文（例：`str.format()` → f-string）を検出する場合 THEN 最新の Python 3.12+ 慣例に更新するべき
5. WHEN 型ヒントが不足しているモジュールを特定する場合 THEN 関数シグネチャに型ヒント（引数、戻り値）を追加するべき
6. WHERE コード品質ツール（ruff/mypy）の実行が CI/CD パイプラインに統合される場合 THE すべてのチェックが自動的に実行され、失敗時はビルドが停止されるべき

#### 実行コマンド

- **ruff チェック**: `uv run ruff check src/ tests/`
- **ruff 自動整形**: `uv run ruff format src/ tests/`
- **mypy 型チェック**: `uv run mypy src/`
- **ruff ルール詳細**: `uv run ruff rule <RULE_CODE>`

#### 注記

ruff と mypy は uv 経由で実行されます。既存の pyproject.toml に設定されているツール設定に準拠します。

---

## Current Status

- **全テスト数**: 156
- **合格**: 124
- **失敗**: 2
- **エラー**: 30
- **全体カバレッジ**: 49%
- **主要な問題**:
  - test_scheduled_post_api.py の 27 個のエラー（ScheduledPostStoreSQLite への未対応）
  - test_scheduler_service.py の 3 個のエラー
  - 主要モジュールの低いカバレッジ（main.py 25%、article_manager.py 27%）

---

## Success Criteria

1. ✅ 全テストが合格（エラー・失敗 0）
2. ✅ 全体カバレッジ 80% 以上を達成
3. ✅ 主要モジュール（main.py、article_manager.py、plugins）のカバレッジが最低 70% 以上
4. ✅ テストドキュメントの整備
5. ✅ CI/CD パイプラインでの自動テスト実行と報告
6. ✅ `uv run ruff check` で警告 0 を達成
7. ✅ `uv run mypy src/` で型エラー 0 を達成
8. ✅ 全ソースコードが最新の Python 3.12+ 慣例に準拠

## 現状 (2026-07-22 棚卸し)

**この spec は陳腐化している。** 記述内容が Python 版(pytest / ScheduledPostStoreSQLite / カバレッジ49%)
を前提としており、Rust へ移植された現行の `main` には適用できない。
Rust 版として requirements から作り直しが必要。

関連Issue: #59
