# Requirements Document: test-coverage-improvement (Rust版)

## Project Description (Input)

安全に運用するため、コードカバレッジ80%以上を維持する。テスト実行時にカバレッジを
計測し、閾値を下回った場合はCIで検出できるようにする。

## Introduction

`main` は Rust 実装 (`blog-autopost-rs`) へ移植済みだが、テストとカバレッジに関する
CI基盤が移植時に引き継がれていない。本specは以下の3点を達成することを目的とする。

1. すべてのPR・pushでテストが実行される状態にする
2. テスト実行と同時にカバレッジが計測される状態にする
3. カバレッジ80%以上を継続的に維持する仕組みを作る

## 現状の計測結果 (2026-07-22, `cargo llvm-cov`)

### 全体

| 指標 | 値 |
|---|---|
| リージョン | 42.06% (7111中 4120が未カバー) |
| ライン | 44.02% (4096中 2293が未カバー) |
| 関数 | 50.25% (402中 200が未カバー) |

### モジュール別 (リージョン基準)

| ファイル | リージョン数 | カバー率 | 区分 |
|---|---|---|---|
| `web/routes.rs` | 1719 | 0.00% | 最優先 |
| `commands.rs` | 1091 | 19.98% | 最優先 |
| `commands/schedule.rs` | 348 | 0.00% | 最優先 |
| `sns/bluesky.rs` | 375 | 42.67% | 優先 |
| `sns/x.rs` | 94 | 0.00% | 優先 |
| `sns/misskey.rs` | 81 | 0.00% | 優先 |
| `sns/mastodon.rs` | 100 | 39.00% | 優先 |
| `sns/mod.rs` | 50 | 34.00% | 優先 |
| `web/mod.rs` | 478 | 56.07% | 中 |
| `image_resizer.rs` | 330 | 67.88% | 中 |
| `scheduled/executor.rs` | 227 | 72.25% | 中 |
| `timing.rs` | 381 | 82.15% | 達成済み |
| `runner.rs` | 345 | 91.59% | 達成済み |
| `scheduled/store.rs` | 395 | 93.92% | 達成済み |
| `article/store.rs` | 245 | 97.14% | 達成済み |
| `article/image_extractor.rs` | 177 | 99.44% | 達成済み |
| `config.rs` | 52 | 98.08% | 達成済み |
| `article/models.rs` | 58 | 98.28% | 達成済み |
| `text/optimizer.rs` | 167 | 100.00% | 達成済み |
| `text/tags.rs` | 76 | 100.00% | 達成済み |
| `scheduled/models.rs` | 19 | 100.00% | 達成済み |
| `sns/traits.rs` | 5 | 100.00% | 達成済み |
| `main.rs` | 53 | 0.00% | 対象外 |

`main.rs` は起動処理のみのため計測対象から除外する。

### 到達可能性の確認

80%到達には 7111 × 0.8 = 5689 リージョンのカバーが必要。現在のカバー済みは 2991 で、
差分は 2698。最優先・優先区分の未カバー分の合計は 3330 (routes 1719 + commands 873 +
schedule 348 + bluesky 215 + x 94 + misskey 81) であり、これらを重点的にカバーすれば
80%は到達可能である。

## Requirements

EARS表記法 (Event / Actor / Response / System) に基づいて記述する。

### Requirement 1: テスト実行CIの整備

**Objective:** dependabot以外のPRでもテストが実行される状態にする。

#### Acceptance Criteria

1. WHEN 開発者が `main` に対するPull Requestを作成した場合 THEN CIシステムは `cargo test --all` を実行するべき
2. WHEN 開発者が `main` へ直接pushした場合 THEN CIシステムは `cargo test --all` を実行するべき
3. WHEN CIワークフローが実行された場合 THEN CIシステムは `cargo fmt --check` と `cargo clippy --all-targets --all-features -- -D warnings` も実行するべき
4. WHEN いずれかの検査が失敗した場合 THEN CIシステムはジョブを失敗させ、マージ前に検知できるようにするべき
5. WHERE CIワークフローの定義において THE CIシステムは runner に `ubuntu-24.04` を使用し、`workflow_dispatch` による手動実行も可能とするべき

### Requirement 2: カバレッジの計測

**Objective:** テスト実行と同時にカバレッジを計測し、結果を確認できるようにする。

#### Acceptance Criteria

1. WHEN 開発者がローカルで `just test-cov` を実行した場合 THEN システムは `cargo llvm-cov` によりカバレッジを計測しHTMLレポートを生成するべき
2. WHEN 開発者がローカルで `just cov` を実行した場合 THEN システムはカバレッジのサマリのみを標準出力へ表示するべき
3. WHEN CIワークフローがテストを実行した場合 THEN CIシステムは同一実行内でカバレッジを計測するべき
4. WHEN カバレッジ計測が完了した場合 THEN CIシステムは結果のサマリをジョブサマリへ出力し、PR上で参照できるようにするべき
5. IF `cargo llvm-cov` が未導入である場合 THEN CIシステムは `taiki-e/install-action` 等により自動的に導入するべき

### Requirement 3: カバレッジ閾値の強制とラチェット

**Objective:** カバレッジの低下を機械的に防ぎ、段階的に80%へ引き上げる。

#### Acceptance Criteria

1. WHERE カバレッジ閾値の管理において THE システムは閾値をリポジトリ内の単一のファイルで管理し、CIワークフローがそれを参照するべき
2. WHEN CIがカバレッジを計測した場合 THEN CIシステムは計測値が設定された閾値を下回るときにジョブを失敗させるべき
3. WHEN テスト追加によりカバレッジが閾値を明確に上回った場合 THEN 開発者は閾値を新しい水準へ引き上げるべき
4. THE システムは閾値を一度たりとも引き下げてはならない
5. WHERE 段階的な引き上げにおいて THE システムは以下のマイルストーンを経由するべき

    | 段階 | 閾値 (リージョン) | 主な対象 |
    |---|---|---|
    | 初期 | 40% | 現状値を割り込ませないための下限 |
    | 第1段階 | 55% | `sns/` 配下と `commands/schedule.rs` |
    | 第2段階 | 65% | `commands.rs` |
    | 第3段階 | 80% | `web/routes.rs` |
    | 維持 | 80% | 以降は80%を下回らない |

6. IF 閾値の引き上げによりCIが失敗する状態が続く場合 THEN 開発者は閾値を戻すのではなくテストを追加して対応するべき

### Requirement 4: 重点モジュールのテスト整備

**Objective:** カバレッジ0%の大規模モジュールにテストを追加する。

#### Acceptance Criteria

1. WHEN `web/routes.rs` のテストを追加する場合 THEN 開発者は `axum` のルータに対し `tower::ServiceExt::oneshot` を用いたリクエスト単位のテストを記述するべき
2. WHEN 認証を要するエンドポイントをテストする場合 THEN 開発者は未認証時に認証エラーを返すことも検証するべき
3. WHEN SNSクライアント (`sns/x.rs`, `sns/misskey.rs` 等) のテストを追加する場合 THEN 開発者は `wiremock` によりHTTPエンドポイントをモックし、実際の外部SNSへ接続しないようにするべき
4. WHEN SNSクライアントのテストを記述する場合 THEN 開発者は成功系に加えHTTPエラー応答時の異常系も検証するべき
5. WHEN `commands/schedule.rs` のテストを追加する場合 THEN 開発者は一時ディレクトリ (`tempfile`) 上のストアに対して予約の追加・一覧・更新・削除を検証するべき
6. THE テストは外部ネットワークへ接続してはならない

### Requirement 5: ドキュメントの整合

**Objective:** カバレッジに関する記述を実態と一致させる。

#### Acceptance Criteria

1. WHEN カバレッジツールを変更した場合 THEN 開発者は `justfile` `README.md` `CLAUDE.md` の記述を同時に更新するべき
2. WHERE `CLAUDE.md` のカバレッジ目標において THE 記述は本specの80%という目標と矛盾しないべき
3. IF `cargo tarpaulin` を前提とする記述が残っている場合 THEN 開発者はそれを `cargo llvm-cov` へ置き換えるべき

## 非機能要件

- CIの実行時間は、キャッシュ (`Swatinem/rust-cache`) 利用時に10分を超えないこと
- カバレッジ計測はテスト実行とは別ジョブにせず、同一ジョブ内で完結させること(ビルド成果物の再利用のため)

## スコープ外

- `main.rs` のカバレッジ
- Python版 (`python` ブランチ) のカバレッジ
- E2Eテスト・ブラウザテストの導入
- カバレッジのサードパーティサービス(Codecov等)への送信

## 関連Issue

- #59 test-coverage-improvement spec のRust版作り直し
- #61 テスト実行CIの追加
- #62 カバレッジ計測のCI組み込みと80%維持
- #63 `just test-cov` の cargo-llvm-cov 化
