# Implementation Tasks

## Overview

本タスクリストは、`scheduled-post-timing-extension`機能の実装を段階的に進めるための作業分解です。TDD原則に従い、各タスクはテストファーストで実装します。

## Task Breakdown

### Phase 1: 基盤コンポーネント実装

#### Task 1.1: TimingManager - ワイルドカード展開機能

**Event**: システム起動時にconfig.ymlのタイミング設定を読み込む時
**Actor**: TimingManager
**Response**: ワイルドカード(`*`, `Weekday`, `Weekend`)を具体的な曜日リストに展開する
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/timing_manager.py`を新規作成
2. `TimingManager.expand_wildcard(day_spec: str) -> List[str]`メソッドを実装
   - `"*"` → `["Monday", ..., "Sunday"]`
   - `"Weekday"` → `["Monday", ..., "Friday"]`
   - `"Weekend"` → `["Saturday", "Sunday"]`
   - 特定曜日はそのまま返す

**テスト**:
- `tests/test_timing_manager.py`を新規作成
- `test_expand_wildcard_asterisk()` - `"*"`の展開
- `test_expand_wildcard_weekday()` - `"Weekday"`の展開
- `test_expand_wildcard_weekend()` - `"Weekend"`の展開
- `test_expand_wildcard_specific_day()` - 特定曜日の処理

**依存関係**: なし

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 1.2: TimingManager - タイミング統合(和集合)機能

**Event**: SNSの投稿可能タイミング情報を取得する時
**Actor**: TimingManager
**Response**: グローバル設定とSNS固有設定を和集合として統合する
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `TimingManager.merge_timings(global_timings, sns_timings) -> Dict[str, List[str]]`メソッドを実装
   - グローバル設定の各エントリをワイルドカード展開して追加
   - SNS固有設定の各エントリをワイルドカード展開して追加
   - 重複時刻は1つにまとめる(set使用)
   - 各曜日の時刻リストを時系列順にソート

**テスト**:
- `test_merge_timings_union()` - 和集合の確認
- `test_merge_timings_duplicate()` - 重複時刻の処理
- `test_merge_timings_empty_global()` - グローバル設定なし
- `test_merge_timings_empty_sns()` - SNS固有設定なし
- `test_merge_timings_wildcard()` - ワイルドカード含む統合

**依存関係**: Task 1.1

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 1.3: TimingManager - 設定バリデーション機能

**Event**: config.ymlを読み込んだ時
**Actor**: TimingManager
**Response**: タイミング設定の妥当性をチェックし、エラーをログ出力する
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `TimingManager.validate_timing_config(timings) -> Tuple[bool, List[str]]`メソッドを実装
   - 曜日指定の妥当性チェック(有効な曜日名 or ワイルドカード)
   - 時刻フォーマットのチェック(`"HH:MM"`, `00:00`〜`23:59`)
   - エラーメッセージリストを返す

**テスト**:
- `test_validate_timing_config_valid()` - 正常な設定
- `test_validate_timing_config_invalid_time()` - 無効時刻(`"25:00"`, `"12:60"`)
- `test_validate_timing_config_invalid_day()` - 無効曜日(`"Moonday"`)
- `test_validate_timing_config_invalid_format()` - フォーマット違反(`"9:00"`, `"09-00"`)

**依存関係**: Task 1.1

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 1.4: TimingManager - ConfigManager連携とキャッシュ

**Event**: アプリケーション起動時
**Actor**: TimingManager
**Response**: ConfigManagerから設定を読み込み、統合済みタイミング情報をキャッシュする
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `TimingManager.__init__(config_manager: ConfigManager)`を実装
2. `TimingManager.get_allowed_timings(sns_name: str) -> Optional[Dict[str, List[str]]]`メソッドを実装
   - 初回呼び出し時にconfig.ymlから`default_allowed_timings`を読み込み
   - 指定されたSNSの`allowed_timings`を読み込み
   - `merge_timings()`で統合
   - 結果をインスタンス変数にキャッシュ
   - 設定なし(両方未定義)の場合は`None`を返す

**テスト**:
- `test_get_allowed_timings_with_both()` - 両方の設定あり
- `test_get_allowed_timings_global_only()` - グローバルのみ
- `test_get_allowed_timings_sns_only()` - SNS固有のみ
- `test_get_allowed_timings_no_config()` - 両方なし → `None`
- `test_get_allowed_timings_cache()` - 2回目以降はキャッシュから取得

**依存関係**: Task 1.2, Task 1.3

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス
- 既存のConfigManagerテストが破綻していないこと

---

#### Task 1.5: ConfigManager - 新設定フィールド読み込み

**Event**: config.ymlを読み込む時
**Actor**: ConfigManager
**Response**: `default_allowed_timings`, `allowed_timings_tolerance_minutes`, SNSごとの`allowed_timings`を読み込む
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/config_manager.py`を拡張
2. グローバル設定として以下を追加:
   - `default_allowed_timings: Optional[List[Tuple[str, List[str]]]]`
   - `allowed_timings_tolerance_minutes: int = 5`
3. SNS設定として以下を追加:
   - `allowed_timings: Optional[List[Tuple[str, List[str]]]]`

**テスト**:
- `tests/test_config_manager.py`に追加
- `test_load_default_allowed_timings()` - グローバル設定読み込み
- `test_load_sns_allowed_timings()` - SNS固有設定読み込み
- `test_load_tolerance_minutes_default()` - デフォルト値5
- `test_load_tolerance_minutes_custom()` - カスタム値
- `test_load_backward_compatibility()` - 新フィールド未設定でも動作

**依存関係**: なし

**完了条件**:
- 全テストがパス
- 既存のConfigManagerテストが破綻していないこと
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

### Phase 2: スロット検索コンポーネント実装

#### Task 2.1: ScheduledPostStoreSQLite - スロット検索用クエリメソッド追加

**Event**: 指定されたSNS・時刻の予約投稿を確認する時
**Actor**: SlotFinder
**Response**: 該当する予約投稿のリストを返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/web/scheduled_post_store_sqlite.py`を拡張
2. `get_posts_by_sns_and_time(sns_name: str, scheduled_at: datetime, tolerance_minutes: int = 0) -> List[ScheduledPost]`メソッドを追加
   - `target_sns`に`sns_name`を含む予約投稿を検索
   - `scheduled_at`が指定時刻±`tolerance_minutes`の範囲内
   - `status`が`"予約済み"`のみ対象

**テスト**:
- `tests/test_scheduled_post_store_sqlite.py`に追加
- `test_get_posts_by_sns_and_time_exact()` - 完全一致
- `test_get_posts_by_sns_and_time_tolerance()` - 許容範囲内
- `test_get_posts_by_sns_and_time_out_of_range()` - 許容範囲外
- `test_get_posts_by_sns_and_time_empty()` - 該当なし
- `test_get_posts_by_sns_and_time_multiple_sns()` - 複数SNS投稿のフィルタリング

**依存関係**: なし

**完了条件**:
- 全テストがパス
- 既存のScheduledPostStoreテストが破綻していないこと
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 2.2: SlotFinder - 候補スロット生成機能

**Event**: 次の空きスロットを検索する時
**Actor**: SlotFinder
**Response**: 指定日数分の候補スロットを時系列順に生成する
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/web/slot_finder.py`を新規作成
2. `SlotFinder.__init__(timing_manager, scheduled_post_store)`を実装
3. `SlotFinder.generate_candidate_slots(sns_name, start_date, days) -> List[datetime]`メソッドを実装
   - `timing_manager.get_allowed_timings(sns_name)`から投稿可能タイミングを取得
   - 指定日数分の日付をループ
   - 各日の曜日に対応する時刻リストから候補を生成
   - `start_date`以前の候補は除外
   - 時系列順にソート

**テスト**:
- `tests/test_slot_finder.py`を新規作成
- `test_generate_candidate_slots_today()` - 当日のスロット生成
- `test_generate_candidate_slots_multiple_days()` - 複数日のスロット生成
- `test_generate_candidate_slots_sorted()` - 時系列順のソート確認
- `test_generate_candidate_slots_no_timings()` - 設定なしの場合空リスト
- `test_generate_candidate_slots_skip_past()` - 過去時刻の除外

**依存関係**: Task 1.4, Task 2.1

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 2.3: SlotFinder - スロット空き状況確認機能

**Event**: 候補スロットが空いているか確認する時
**Actor**: SlotFinder
**Response**: 既存予約の有無を返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `SlotFinder.is_slot_available(sns_name, slot_time) -> bool`メソッドを実装
   - `scheduled_post_store.get_posts_by_sns_and_time()`を呼び出し
   - 既存予約が0件ならTrue、1件以上ならFalse

**テスト**:
- `test_is_slot_available_empty()` - 空きスロット
- `test_is_slot_available_occupied()` - 埋まっているスロット
- `test_is_slot_available_tolerance()` - 許容範囲内の競合検出

**依存関係**: Task 2.1

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 2.4: SlotFinder - 次の空きスロット検索機能

**Event**: 指定されたSNSの次の空きスロットを検索する時
**Actor**: SlotFinder
**Response**: 最初に見つかった空きスロットの日時を返す(見つからない場合None)
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `SlotFinder.find_next_available_slot(sns_name, start_from, max_days) -> Optional[datetime]`メソッドを実装
   - `generate_candidate_slots()`で候補を生成
   - 各候補を時系列順にループ
   - `is_slot_available()`で空き確認
   - 最初の空きスロットを返す
   - 見つからない場合`None`

**テスト**:
- `test_find_next_available_slot_today()` - 当日の空きスロット検索
- `test_find_next_available_slot_next_day()` - 翌日へのロールオーバー
- `test_find_next_available_slot_conflict()` - 競合スロットのスキップ
- `test_find_next_available_slot_no_slot()` - 7日以内に空きなし
- `test_find_next_available_slot_no_config()` - 設定なしの場合`None`

**依存関係**: Task 2.2, Task 2.3

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 2.5: SlotFinder - 複数SNS一括検索機能

**Event**: 複数SNSへの投稿で各SNSの次の空きスロットを検索する時
**Actor**: SlotFinder
**Response**: 各SNSの次の空きスロット(またはNone)を辞書で返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `SlotFinder.find_slots_for_multiple_sns(sns_list) -> Dict[str, Optional[datetime]]`メソッドを実装
   - 各SNSに対して`find_next_available_slot()`を呼び出し
   - 結果を辞書にまとめて返す

**テスト**:
- `test_find_slots_for_multiple_sns_all_success()` - 全SNSで空きあり
- `test_find_slots_for_multiple_sns_partial_failure()` - 一部SNSで空きなし
- `test_find_slots_for_multiple_sns_independent()` - 各SNS独立検索の確認

**依存関係**: Task 2.4

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

### Phase 3: タイミング検証コンポーネント実装

#### Task 3.1: TimingValidator - 曜日取得とタイムゾーン処理

**Event**: 実行時刻の曜日を取得する時
**Actor**: TimingValidator
**Response**: datetimeから曜日名を返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/web/timing_validator.py`を新規作成
2. `TimingValidator.__init__(timing_manager, tolerance_minutes)`を実装
3. `TimingValidator.get_day_of_week(dt: datetime) -> str`メソッドを実装
   - `dt.strftime("%A")`で曜日名を取得(`"Monday"`, `"Tuesday"`, ...)
   - ローカルタイムゾーンに正規化

**テスト**:
- `tests/test_timing_validator.py`を新規作成
- `test_get_day_of_week_monday()` - 月曜日の取得
- `test_get_day_of_week_sunday()` - 日曜日の取得
- `test_get_day_of_week_timezone()` - タイムゾーン境界

**依存関係**: Task 1.4

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 3.2: TimingValidator - 時刻許容範囲チェック

**Event**: 実行時刻が設定時刻の許容範囲内かチェックする時
**Actor**: TimingValidator
**Response**: 許容範囲内ならTrue、範囲外ならFalseを返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `TimingValidator.is_time_within_tolerance(target_time, execution_time, tolerance_minutes) -> bool`メソッドを実装
   - `target_time`(`"HH:MM"`)を`execution_time`と同じ日付のdatetimeに変換
   - `execution_time`との時間差を計算
   - `tolerance_minutes`以内ならTrue

**テスト**:
- `test_is_time_within_tolerance_exact()` - 完全一致
- `test_is_time_within_tolerance_within()` - 許容範囲内(±5分)
- `test_is_time_within_tolerance_out()` - 許容範囲外
- `test_is_time_within_tolerance_zero()` - 許容範囲0分

**依存関係**: Task 3.1

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 3.3: TimingValidator - タイミング検証機能

**Event**: 予約投稿実行時にタイミング制約を確認する時
**Actor**: TimingValidator
**Response**: 許可範囲内ならTrue、範囲外ならFalseとスキップ理由を返す
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `TimingValidator.validate_timing(sns_name, execution_time) -> Tuple[bool, Optional[str]]`メソッドを実装
   - `timing_manager.get_allowed_timings(sns_name)`から設定取得
   - 設定なし(`None`)の場合は`(True, None)`を返す(制限なし)
   - `get_day_of_week()`で実行時刻の曜日を取得
   - 該当曜日の時刻リストをループ
   - `is_time_within_tolerance()`で各時刻をチェック
   - いずれかの時刻が範囲内なら`(True, None)`
   - すべて範囲外なら`(False, "理由メッセージ")`

**テスト**:
- `test_validate_timing_within_range()` - 許可範囲内
- `test_validate_timing_out_of_range()` - 許可範囲外
- `test_validate_timing_tolerance()` - 許容範囲(±5分)での検証
- `test_validate_timing_no_config()` - 設定なしは常にTrue
- `test_validate_timing_wrong_day()` - 設定のない曜日

**依存関係**: Task 3.2

**完了条件**:
- 全テストがパス
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

### Phase 4: API実装

#### Task 4.1: POST /api/scheduled-posts/next - エンドポイント実装

**Event**: ユーザーが「次のタイミングで投稿」ボタンをクリックした時
**Actor**: Web UI
**Response**: 各SNSの次の空きスロットに予約投稿を作成し、結果を返す
**System**: Blog AutoPost CLI (Web API)

**実装内容**:
1. `src/web/routes/scheduled_posts.py`に新規エンドポイントを追加
2. リクエストボディ: `content`, `target_sns`, `media_files`
3. 処理フロー:
   - `slot_finder.find_slots_for_multiple_sns(target_sns)`でスロット検索
   - 各SNSのスロットに対して`scheduled_post_store.create_post()`
   - 成功したSNSと失敗したSNSをレスポンスに含める
4. レスポンス: `created_posts`, `errors`

**テスト**:
- `tests/test_scheduled_posts_api_timing.py`を新規作成
- `test_create_post_next_timing_single_sns()` - 単一SNSへの次のタイミング投稿
- `test_create_post_next_timing_multiple_sns()` - 複数SNSへの次のタイミング投稿
- `test_create_post_next_timing_no_slots()` - 空きスロットなしのエラー処理
- `test_create_post_next_timing_partial_success()` - 一部SNS失敗時の部分成功

**依存関係**: Task 2.5

**完了条件**:
- 全テストがパス
- APIドキュメント(OpenAPI)更新
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 4.2: GET /api/sns-timings - エンドポイント実装

**Event**: Web UIダッシュボード表示時に各SNSのタイミング情報を取得する時
**Actor**: Web UI
**Response**: 全SNSの投稿可能タイミング情報と次の空きスロットを返す
**System**: Blog AutoPost CLI (Web API)

**実装内容**:
1. `src/web/routes/scheduled_posts.py`に新規エンドポイントを追加
2. 処理フロー:
   - 全SNSに対して`timing_manager.get_allowed_timings()`を呼び出し
   - 設定ありの場合、曜日別にグループ化し、ソース(共通/固有)をマーク
   - `slot_finder.find_next_available_slot()`で次の空きスロットを取得
3. レスポンス: `sns_timings`リスト

**テスト**:
- `test_get_sns_timings_all()` - 全SNS情報取得
- `test_get_sns_timings_with_restrictions()` - 制限ありSNS
- `test_get_sns_timings_no_restrictions()` - 制限なしSNS
- `test_get_sns_timings_next_slot()` - 次の空きスロット情報

**依存関係**: Task 1.4, Task 2.4

**完了条件**:
- 全テストがパス
- APIドキュメント(OpenAPI)更新
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 4.3: POST /api/scheduled-posts - 手動時刻指定のバリデーション強化

**Event**: ユーザーが手動で日時を指定して予約投稿を作成する時
**Actor**: Web UI
**Response**: 指定時刻が許可範囲外の場合、400エラーと推奨時刻を返す
**System**: Blog AutoPost CLI (Web API)

**実装内容**:
1. `src/web/routes/scheduled_posts.py`の既存エンドポイントを拡張
2. 処理フロー:
   - リクエストの`scheduled_at`, `target_sns`を取得
   - 各SNSに対して`timing_validator.validate_timing()`を実行
   - 範囲外の場合、`slot_finder.find_next_available_slot()`で推奨時刻を取得
   - 400エラーレスポンスを返す

**テスト**:
- `test_create_post_manual_timing_valid()` - 手動時刻指定(許可範囲内)
- `test_create_post_manual_timing_invalid()` - 手動時刻指定(許可範囲外)
- `test_create_post_manual_timing_suggestion()` - 推奨時刻の提示

**依存関係**: Task 3.3, Task 2.4

**完了条件**:
- 全テストがパス
- 既存の予約投稿作成テストが破綻していないこと
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

#### Task 4.4: DELETE /api/scheduled-posts/{post_id} - 予約取り消しエンドポイント実装

**Event**: ユーザーが予約投稿一覧から取り消しボタンをクリックした時
**Actor**: Web UI
**Response**: 予約投稿を削除(物理削除)し、スロットを解放する
**System**: Blog AutoPost CLI (Web API)

**実装内容**:
1. `src/web/routes/scheduled_posts.py`に新規エンドポイントを追加
2. 処理フロー:
   - `scheduled_post_store.get_post_by_id(post_id)`で予約取得
   - ステータスが`"予約済み"`以外(実行済み、失敗など)なら400エラー
   - 関連するメディアファイルをディスクから削除(あれば)
   - `scheduled_post_store.delete_post(post_id)`で物理削除
   - レスポンスに`freed_slot`情報を含める
3. セキュリティ:
   - CSRF保護(`X-CSRF-Token`ヘッダー必須)
   - 認証チェック(ログインユーザーのみ)

**テスト**:
- `test_cancel_scheduled_post_success()` - 予約取り消し成功(スロット解放)
- `test_cancel_scheduled_post_not_found()` - 存在しない予約の取り消し(404)
- `test_cancel_scheduled_post_already_executed()` - 実行済み投稿の取り消し拒否(400)
- `test_cancel_scheduled_post_slot_freed()` - 取り消し後のスロット再利用確認
- `test_cancel_scheduled_post_media_cleanup()` - メディアファイルの削除確認

**依存関係**: Task 2.1

**完了条件**:
- 全テストがパス
- APIドキュメント(OpenAPI)更新
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

### Phase 5: PostExecutor拡張

#### Task 5.1: PostExecutor - タイミング検証ロジック追加

**Event**: APSchedulerが予約投稿の実行時刻に達した時
**Actor**: PostExecutor
**Response**: 各対象SNSのタイミング検証を行い、範囲外の場合はスキップする
**System**: Blog AutoPost CLI (Backend)

**実装内容**:
1. `src/web/post_executor.py`を拡張
2. `execute_post()`メソッドに以下を追加:
   - 各`target_sns`に対してループ
   - `timing_validator.validate_timing(sns, scheduled_at)`を呼び出し
   - `False`の場合、そのSNSへの投稿をスキップ
   - ステータスを`"スキップ"`に更新
   - `error_message`にスキップ理由を記録
   - ログ出力(INFO): 投稿ID、SNS、設定時刻、実行時刻、スキップ理由

**テスト**:
- `test_post_execution_timing_validation()` - 投稿実行時のタイミング検証
- `test_post_execution_skip_out_of_range()` - 範囲外投稿のスキップ
- `test_post_execution_partial_skip()` - 一部SNSのみスキップ
- `test_post_execution_no_config()` - 設定なしは常に実行

**依存関係**: Task 3.3

**完了条件**:
- 全テストがパス
- 既存のPostExecutorテストが破綻していないこと
- カバレッジ90%以上
- ruff/mypyチェックがパス

---

### Phase 6: UI実装

#### Task 6.1: 予約投稿モーダル - 「次のタイミングで投稿」ボタン追加

**Event**: ユーザーが予約投稿モーダルを開いた時
**Actor**: Web UI
**Response**: 「次のタイミングで投稿」と「手動で日時指定」の2つのボタンを表示
**System**: Blog AutoPost CLI (Web UI)

**実装内容**:
1. `src/web/templates/index.html`の予約投稿モーダルを修正
2. HTML要素追加:
   - `<ion-button id="scheduleNextBtn">次のタイミングで投稿</ion-button>`
   - `<ion-button id="scheduleManualBtn">手動で日時指定</ion-button>`
   - `<div id="manualScheduleForm" style="display: none;">` (手動フォーム)
3. JavaScriptロジック追加:
   - `scheduleNextBtn`クリック → POST /api/scheduled-posts/next
   - `scheduleManualBtn`クリック → 手動フォーム表示

**テスト**:
- 手動UIテスト(ブラウザ確認)
- Playwrightテスト(可能であれば):
  - ボタンの存在確認
  - クリック時の動作確認

**依存関係**: Task 4.1

**完了条件**:
- UIが正常に表示される
- ボタンクリック時に適切なAPIリクエストが送信される
- ruff/mypyチェックがパス(JavaScript除く)

---

#### Task 6.2: 予約投稿モーダル - 投稿結果表示の強化

**Event**: 「次のタイミングで投稿」の結果が返ってきた時
**Actor**: Web UI
**Response**: 各SNSの予約日時を一覧表示し、成功/失敗を視覚的に示す
**System**: Blog AutoPost CLI (Web UI)

**実装内容**:
1. `src/web/templates/index.html`に結果表示セクションを追加
2. `created_posts`をループし、各SNSの予約日時を表示
3. `errors`をループし、失敗したSNSのエラーメッセージを表示
4. Ionic Toastで成功/失敗メッセージを表示

**テスト**:
- 手動UIテスト(ブラウザ確認)
- Playwrightテスト(可能であれば)

**依存関係**: Task 6.1

**完了条件**:
- UIが正常に表示される
- 成功/失敗が視覚的に区別できる
- ruff/mypyチェックがパス(JavaScript除く)

---

#### Task 6.3: ダッシュボード - SNS投稿タイミング情報セクション追加

**Event**: ユーザーがダッシュボードを開いた時
**Actor**: Web UI
**Response**: 各SNSの投稿可能タイミングと次の空きスロットを表示
**System**: Blog AutoPost CLI (Web UI)

**実装内容**:
1. `src/web/templates/index.html`にタイミング情報セクションを追加
2. GET /api/sns-timingsを呼び出し
3. 各SNSの情報を表示:
   - SNS名
   - 投稿可能タイミング(曜日別、時刻ソート済み)
   - 各時刻のソース表示(共通/固有)
   - 次の空きスロット

**テスト**:
- 手動UIテスト(ブラウザ確認)
- Playwrightテスト(可能であれば)

**依存関係**: Task 4.2

**完了条件**:
- UIが正常に表示される
- タイミング情報が正確に表示される
- ruff/mypyチェックがパス(JavaScript除く)

---

#### Task 6.4: 予約投稿一覧 - 取り消しボタン追加

**Event**: ユーザーが予約投稿一覧を表示した時
**Actor**: Web UI
**Response**: 予約済み投稿に取り消しボタンを表示
**System**: Blog AutoPost CLI (Web UI)

**実装内容**:
1. `src/web/templates/index.html`の予約投稿一覧を修正
2. HTML要素追加:
   - 予約済み(`status === "予約済み"`)の投稿のみ表示
   - `<ion-button (click)="cancelScheduledPost(post.id)">` (取り消しボタン)
3. JavaScriptロジック追加:
   - `cancelScheduledPost(postId)` 関数実装
   - 確認ダイアログ表示
   - DELETE /api/scheduled-posts/{post_id}を呼び出し
   - 成功時にToast表示とリスト再読み込み

**テスト**:
- 手動UIテスト(ブラウザ確認)
- Playwrightテスト(可能であれば):
  - ボタンの存在確認
  - 確認ダイアログの動作確認
  - 取り消し後のリスト更新確認

**依存関係**: Task 4.4

**完了条件**:
- UIが正常に表示される
- 取り消し操作が正常に動作する
- 確認ダイアログが表示される
- ruff/mypyチェックがパス(JavaScript除く)

---

### Phase 7: 統合テストとドキュメント

#### Task 7.1: E2Eテスト - 次のタイミングで投稿フロー

**Event**: E2Eテスト実行時
**Actor**: テストスクリプト
**Response**: ユーザーストーリー全体をカバーするテストを実行
**System**: Blog AutoPost CLI (全体)

**実装内容**:
1. Playwrightテスト作成(可能であれば):
   - config.ymlに投稿タイミング設定を追加
   - アプリケーション起動
   - 予約投稿モーダルを開く
   - 「次のタイミングで投稿」ボタンをクリック
   - 予約結果を確認
   - 予約投稿一覧で確認
   - 取り消しボタンで削除
   - スロットが再度空きになることを確認

**テスト**:
- E2Eテストスイート実行

**依存関係**: Task 6.4

**完了条件**:
- E2Eテストがパス
- 全フローが正常に動作

---

#### Task 7.2: ドキュメント更新

**Event**: 実装完了時
**Actor**: 開発者
**Response**: README、APIドキュメント、設定サンプルを更新
**System**: Blog AutoPost CLI (ドキュメント)

**実装内容**:
1. `README.md`に新機能の説明を追加
2. `config.yml.template`に新設定フィールドのサンプルを追加
3. APIドキュメント(OpenAPI)に新エンドポイントを追加
4. 設定ファイルのバリデーションルールをドキュメント化

**依存関係**: 全タスク完了

**完了条件**:
- ドキュメントが最新状態
- サンプル設定が動作する

---

#### Task 7.3: カバレッジ総合確認とリファクタリング

**Event**: 全タスク完了時
**Actor**: 開発者
**Response**: 全体カバレッジを確認し、70%以上を達成
**System**: Blog AutoPost CLI (全体)

**実装内容**:
1. `uv run pytest --cov=src`を実行
2. カバレッジレポートを確認
3. 70%未満の場合、追加テストを作成
4. 必要に応じてリファクタリング
5. ruff/mypyチェック

**依存関係**: 全タスク完了

**完了条件**:
- 全体カバレッジ70%以上
- 新規コードカバレッジ90%以上
- ruff/mypyチェックがパス

---

## タスク実行順序

### Phase 1(Week 1-2): 基盤コンポーネント実装
1. Task 1.1 → Task 1.2 → Task 1.3 → Task 1.4
2. Task 1.5(並行可能)

### Phase 2(Week 2-3): スロット検索コンポーネント実装
1. Task 2.1
2. Task 2.2 → Task 2.3 → Task 2.4 → Task 2.5

### Phase 3(Week 3): タイミング検証コンポーネント実装
1. Task 3.1 → Task 3.2 → Task 3.3

### Phase 4(Week 3-4): API実装
1. Task 4.1, Task 4.2(並行可能)
2. Task 4.3
3. Task 4.4

### Phase 5(Week 4): PostExecutor拡張
1. Task 5.1

### Phase 6(Week 4-5): UI実装
1. Task 6.1 → Task 6.2
2. Task 6.3(並行可能)
3. Task 6.4

### Phase 7(Week 5): 統合テストとドキュメント
1. Task 7.1
2. Task 7.2
3. Task 7.3

## 優先度

**High Priority (P0)**:
- Task 1.1, 1.2, 1.4, 1.5 (基盤コンポーネント)
- Task 2.4 (スロット検索)
- Task 3.3 (タイミング検証)
- Task 4.1 (次のタイミング投稿API)

**Medium Priority (P1)**:
- Task 2.1, 2.2, 2.3, 2.5 (スロット検索周辺)
- Task 3.1, 3.2 (タイミング検証周辺)
- Task 4.2, 4.3 (その他API)
- Task 5.1 (PostExecutor)

**Low Priority (P2)**:
- Task 4.4 (取り消し機能)
- Task 6.x (UI実装)
- Task 7.x (統合テスト・ドキュメント)

## 見積もり

- **Phase 1**: 2週間(Task 1.1〜1.5)
- **Phase 2**: 1週間(Task 2.1〜2.5)
- **Phase 3**: 1週間(Task 3.1〜3.3)
- **Phase 4**: 1.5週間(Task 4.1〜4.4)
- **Phase 5**: 0.5週間(Task 5.1)
- **Phase 6**: 1週間(Task 6.1〜6.4)
- **Phase 7**: 1週間(Task 7.1〜7.3)

**Total**: 8週間(約2ヶ月)
