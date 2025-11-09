# Requirements Document

## Introduction

本機能は、既存の予約投稿機能を拡張し、各SNSアカウントに対して複数の「投稿可能タイミング」を曜日別・時刻別に設定できるようにする機能です。Bufferのような「次の空きスロット」自動選択機能を実装し、SNS運用における投稿タイミングの最適化と運用負荷の軽減を実現します。

### ビジネス価値
- SNS投稿の時間帯最適化により、より多くのフォロワーにリーチできる
- 「次のタイミングで投稿」機能により、投稿ごとに時間を指定する手間を削減
- 曜日ごとに異なる投稿時間を設定可能
- 複数SNSへの投稿時、各SNSの最適タイミングで自動的に分散投稿
- グローバル設定により、全SNS共通のスロットを一括管理

### 参考サービス
本機能は、Bufferの投稿スケジュール・キュー管理システムを参考にしています。

## Requirements

### Requirement 1: SNS投稿タイミング設定の管理
**Objective:** システム管理者として、各SNSアカウントに曜日別・時刻別の投稿可能タイミングを設定したい。これにより、SNSごとの最適な投稿時間を事前定義できる。

#### Acceptance Criteria

1. WHEN config.ymlファイルでグローバル設定を記述する場合 THEN Blog AutoPost CLIは`default_allowed_timings`という配列フィールドを認識できること
2. WHEN config.ymlファイルでSNS設定を記述する場合 THEN Blog AutoPost CLIは各SNSエントリに`allowed_timings`という配列フィールドを認識できること
3. WHERE `default_allowed_timings`または`allowed_timings`配列内 THE Blog AutoPost CLIは曜日と時刻を以下の形式で受け入れること:
   - リスト構造: `[曜日指定, [時刻リスト]]`
   - 曜日指定: `"Monday"`, `"Tuesday"`, `"Wednesday"`, `"Thursday"`, `"Friday"`, `"Saturday"`, `"Sunday"`, `"*"`, `"Weekday"`, `"Weekend"`
   - 時刻形式: `"HH:MM"` (例: `"09:00"`, `"15:30"`)
   - 設定例: `[["Monday", ["09:00", "12:00"]], ["Wednesday", ["09:00", "15:30"]], ["*", ["18:00"]]]`
4. WHERE ワイルドカード指定 THE Blog AutoPost CLIは以下のように解釈すること:
   - `"*"`: 全曜日(月〜日)
   - `"Weekday"`: 平日(月〜金)
   - `"Weekend"`: 週末(土日)
5. WHEN SNSに`allowed_timings`と`default_allowed_timings`の両方が適用される場合 THEN Blog AutoPost CLIは両方の設定を統合(和集合)して利用可能スロットとすること
6. WHEN `allowed_timings`が設定されておらず`default_allowed_timings`のみの場合 THEN Blog AutoPost CLIはグローバル設定のみを投稿可能タイミングとすること
7. WHEN 両方とも設定されていない場合 THEN Blog AutoPost CLIは従来通り任意の時刻での予約投稿を許可すること
8. IF `allowed_timings`または`default_allowed_timings`に無効な時刻フォーマット(例: "25:00", "12:60")が含まれる場合 THEN Blog AutoPost CLIは起動時に設定エラーを表示し、該当エントリを無効化すること
9. IF 無効な曜日指定が含まれる場合 THEN Blog AutoPost CLIは起動時に設定エラーを表示し、該当エントリを無効化すること
10. WHERE ConfigManagerクラス THE Blog AutoPost CLIは`default_allowed_timings`と各SNSの`allowed_timings`を読み込み、統合済みの投稿可能タイミング情報をSNS設定に含めて提供すること

### Requirement 2: 次の空きスロット自動選択機能
**Objective:** コンテンツ投稿者として、「次のタイミングで投稿」ボタンで各SNSの次の空きスロットに自動予約したい。これにより、時刻を手動で選択する手間を削減できる。

#### Acceptance Criteria

1. WHEN Web UI予約投稿作成画面を表示する場合 THEN Blog AutoPost CLIは「次のタイミングで投稿」ボタンを表示すること
2. WHEN ユーザーが「次のタイミングで投稿」ボタンをクリックした場合 THEN Blog AutoPost CLIは選択された各SNSについて独立して次の空きスロットを検索すること
3. WHERE 次の空きスロット検索 THE Blog AutoPost CLIは以下のロジックで検索すること:
   - 現在時刻以降の投稿可能タイミングを時系列順にチェック
   - 各タイミングで既存の予約投稿が存在するかチェック
   - 既存予約がない最初のスロットを選択
   - 当日にスロットがない場合は翌日以降を検索
   - 最大7日先まで検索し、見つからない場合はエラー
4. WHEN 複数のSNSを選択している場合 THEN Blog AutoPost CLIは各SNSごとに独立して次の空きスロットを見つけ、個別の予約投稿として作成すること
5. IF あるSNSで7日以内に空きスロットが見つからない場合 THEN Blog AutoPost CLIはそのSNSのみエラーメッセージを表示し、他のSNSの予約は正常に作成すること
6. WHERE 予約投稿作成結果 THE Blog AutoPost CLIは各SNSの予約日時を明示的に表示すること(例: "X: 2025-11-10 09:00, Bluesky: 2025-11-10 10:00")
7. WHEN 同じ時刻に複数の予約を作成しようとした場合 THEN Blog AutoPost CLIは競合を検出し、2つ目以降の予約は次のスロットに自動的に配置すること

### Requirement 3: 手動時刻選択機能との併用
**Objective:** コンテンツ投稿者として、「次のタイミング」自動選択と手動時刻選択の両方を利用したい。これにより、状況に応じて柔軟に予約方法を選択できる。

#### Acceptance Criteria

1. WHEN Web UI予約投稿作成画面を表示する場合 THEN Blog AutoPost CLIは「次のタイミングで投稿」ボタンと「手動で日時指定」オプションの両方を提供すること
2. WHERE 手動日時指定モード THE Blog AutoPost CLIは日付選択フィールドと時刻選択ドロップダウンを表示すること
3. IF 選択されたすべてのSNSに投稿可能タイミング設定がある場合 THEN Blog AutoPost CLIは時刻ドロップダウンに利用可能な時刻のみをリストアップすること
4. IF 選択されたSNS間で共通の投稿可能タイミングが存在しない場合 THEN Blog AutoPost CLIは各SNSの利用可能時刻の和集合を表示し、選択時に警告を表示すること
5. IF 一部のSNSに投稿可能タイミング設定がない場合 THEN Blog AutoPost CLIは自由な時刻入力フィールドを表示すること
6. WHERE 日付選択フィールド THE Blog AutoPost CLIは本日以降の日付のみを選択可能にすること
7. WHEN 選択された日時が現在時刻より過去の場合 THEN Blog AutoPost CLIはバリデーションエラーを表示し、投稿の作成を拒否すること
8. WHEN 手動指定時刻が投稿可能タイミングの範囲外の場合 THEN Blog AutoPost CLIは警告メッセージを表示し、ユーザーに確認を求めること

### Requirement 4: 予約投稿実行時のタイミング検証
**Objective:** システム管理者として、設定された投稿可能タイミングに準拠した投稿のみが実行されることを保証したい。これにより、意図しない時間帯での投稿を防止できる。

#### Acceptance Criteria

1. WHEN 予約投稿の実行時刻が到来した場合 THEN Post Executorは投稿対象SNSの投稿可能タイミング設定を確認すること
2. IF 対象SNSに投稿可能タイミング設定があり、かつ実行時刻がその範囲内にない場合 THEN Post Executorは投稿を実行せず、ステータスを「スキップ」に更新すること
3. WHERE ステータスが「スキップ」に更新された投稿 THE Post Executorは理由を`error_message`フィールドに記録すること(例: "投稿時刻が許可されたタイミングの範囲外のため実行をスキップしました")
4. IF 対象SNSに投稿可能タイミング設定がない場合 THEN Post Executorは従来通り予約時刻に投稿を実行すること
5. WHEN 複数のSNSに同時投稿する予約投稿で、一部のSNSのみがタイミング制約に違反する場合 THEN Post Executorは制約を満たすSNSのみに投稿を実行し、違反したSNSをスキップすること

### Requirement 5: 投稿可能タイミングの時刻許容範囲
**Objective:** システム運用者として、設定された投稿可能タイミングに若干の時刻のずれを許容したい。これにより、スケジューラーの実行タイミングのずれによる投稿漏れを防止できる。

#### Acceptance Criteria

1. WHEN Post Executorが投稿可能タイミングをチェックする場合 THEN Blog AutoPost CLIは設定された時刻の前後5分を許容範囲とすること
2. IF 実行時刻が設定時刻±5分の範囲内である場合 THEN Post Executorは投稿を実行すること
3. WHERE config.yml設定 THE Blog AutoPost CLIは`allowed_timings_tolerance_minutes`という設定項目で許容時間を変更可能にすること(デフォルト: 5)
4. IF `allowed_timings_tolerance_minutes`に0が設定されている場合 THEN Post Executorは厳密な時刻一致のみを許可すること
5. WHEN 許容範囲内に複数の設定時刻が含まれる場合 THEN Post Executorは最も近い設定時刻を基準として判定すること

### Requirement 6: Web UI上での投稿タイミング設定の表示
**Objective:** コンテンツ投稿者として、Web UIのダッシュボードで各SNSの投稿可能タイミングを確認したい。これにより、どの時刻に投稿可能かを把握できる。

#### Acceptance Criteria

1. WHEN Web UIダッシュボードを表示する場合 THEN Blog AutoPost CLIは設定されている全SNSアカウントのリストを表示すること
2. WHERE 各SNSアカウントの表示エリア THE Blog AutoPost CLIは投稿可能タイミングが設定されている場合、曜日別にグループ化した時刻リストを表示すること
3. WHERE 投稿可能タイミング表示 THE Blog AutoPost CLIは以下の形式で表示すること:
   - グローバル設定由来のスロットには「(共通)」マークを付ける
   - SNS固有設定由来のスロットには「(固有)」マークを付ける
   - 時刻は`HH:MM`形式で、時系列順にソート
4. IF SNSに投稿可能タイミング設定がない場合 THEN Blog AutoPost CLIは「制限なし」または同等のメッセージを表示すること
5. WHERE ダッシュボード THE Blog AutoPost CLIは各SNSの次の空きスロット情報も表示すること(例: "次の空き: 11/10 09:00")

### Requirement 7: スロット競合管理
**Objective:** システム管理者として、同じ時刻に複数の予約が競合しないよう管理したい。これにより、投稿の重複や漏れを防止できる。

#### Acceptance Criteria

1. WHEN 予約投稿を作成する際 THEN Blog AutoPost CLIは指定された日時・SNSで既存の予約投稿が存在するかチェックすること
2. IF 同じSNS・同じ時刻に既存の予約が存在する場合 THEN Blog AutoPost CLIはスロット競合として検出すること
3. WHERE 「次のタイミングで投稿」機能使用時 THE Blog AutoPost CLIは競合するスロットを自動的にスキップし、次の空きスロットを選択すること
4. WHERE 手動時刻指定時 THE Blog AutoPost CLIは競合警告を表示し、ユーザーに以下の選択肢を提供すること:
   - 次の空きスロットを使用
   - 別の時刻を手動選択
   - キャンセル
5. WHEN スケジューラーがスロット検索を行う際 THEN Blog AutoPost CLIは最大7日先まで検索し、各曜日の投稿可能タイミングを考慮すること
6. IF 7日以内に空きスロットが見つからない場合 THEN Blog AutoPost CLIはエラーメッセージを表示し、予約作成を拒否すること

### Requirement 8: 既存予約投稿との後方互換性
**Objective:** システム管理者として、既存の予約投稿データと機能が新機能導入後も正常に動作することを保証したい。これにより、データ移行やユーザー混乱を回避できる。

#### Acceptance Criteria

1. WHEN 既存の予約投稿データ(scheduled_posts.db)を読み込む場合 THEN Blog AutoPost CLIは投稿可能タイミング設定の有無に関わらず、既存データを正常に読み込むこと
2. IF 既存の予約投稿が投稿可能タイミング未設定のSNSに対するものである場合 THEN Post Executorは従来通り予約時刻に投稿を実行すること
3. WHEN 投稿可能タイミング設定が追加される前に作成された予約投稿がある場合 THEN Blog AutoPost CLIはそれらの投稿を「制限なし」として扱うこと
4. WHERE ScheduledPostモデル THE Blog AutoPost CLIは新しいフィールドを追加せず、既存のスキーマを維持すること

### Requirement 9: エラーハンドリングとログ出力
**Objective:** システム運用者として、投稿タイミング機能に関連するエラーとイベントを追跡したい。これにより、問題の診断と解決を迅速に行える。

#### Acceptance Criteria

1. WHEN 投稿がタイミング制約によりスキップされた場合 THEN Blog AutoPost CLIはINFOレベルでログに記録すること
2. WHERE ログメッセージ THE Blog AutoPost CLIは投稿ID、対象SNS、設定時刻、実行時刻、スキップ理由を含めること
3. IF config.yml内の投稿可能タイミング設定にエラーがある場合 THEN Blog AutoPost CLIはERRORレベルでログに記録し、該当SNSの設定詳細を含めること
4. WHEN Web UIで投稿可能タイミングの計算中にエラーが発生した場合 THEN Blog AutoPost CLIはユーザーに分かりやすいエラーメッセージを表示し、ログに詳細を記録すること
5. WHERE Post Executorの実行ログ THE Blog AutoPost CLIは各投稿に対するタイミングチェック結果(許可/拒否)を記録すること
6. WHEN 「次のタイミングで投稿」機能で空きスロット検索を行う際 THEN Blog AutoPost CLIは検索プロセス(検索範囲、チェックしたスロット数、見つかったスロット)をDEBUGレベルでログに記録すること
