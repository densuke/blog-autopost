# Tasks: test-coverage-improvement (Rust版)

各フェーズは独立してマージ可能な単位とする。1トピック1コミットを守ること。

## Phase 0: 計測基盤の整備 (Issue #63, #61, #62)

閾値の初期値は現状値 42% を割り込ませないための下限として 40 とする。
この段階ではテストを追加しないため、カバレッジの数値は変わらない。

### Task 0.1: justfile のカバレッジレシピを修正

- [x] `test-cov` を `cargo llvm-cov --html` へ変更
- [x] サマリのみ表示する `cov` レシピを追加
- [x] `help` レシピの記述を更新
- [x] ローカルで `just cov` が動作することを確認

### Task 0.2: 閾値ファイルの追加

- [x] リポジトリルートに `coverage-threshold.txt` を作成し `40` と記述
- [x] 数値のみを記述し、コメント行を含めないこと(CIが `cat` でそのまま読むため)

### Task 0.3: CIワークフローの追加

- [x] `.github/workflows/ci.yml` を新規作成
- [x] トリガーは `pull_request` (main) / `push` (main) / `workflow_dispatch`
- [x] runner は `ubuntu-24.04`
- [x] `cargo fmt --all -- --check` を実行
- [x] `cargo clippy --all-targets --all-features -- -D warnings` を実行
- [x] `taiki-e/install-action@cargo-llvm-cov` で計測ツールを導入
- [x] `coverage-threshold.txt` を読んで `--fail-under-regions` に渡す
- [x] 計測結果を `GITHUB_STEP_SUMMARY` へ出力
- [x] `Swatinem/rust-cache@v2` によるキャッシュを設定

### Task 0.4: 動作確認

- [x] PRを作成し、CIが起動することを確認 — PR #65 で成功
- [x] 閾値を一時的に 99 にして、CIが失敗することを確認(閾値が効いていることの検証) — ローカルで検証済み(閾値99でexit=1、40でexit=0)
- [x] 閾値を 40 に戻してCIが成功することを確認
- [x] CI実行時間がキャッシュ有効時に10分以内であることを確認 — キャッシュなしの初回で3分4秒

### Task 0.5: ドキュメント更新

- [x] `README.md` のカバレッジ節を cargo-llvm-cov ベースへ統一
- [x] `CLAUDE.md` のカバレッジ目標(現在70%)を本specの80%と整合させる
- [x] `cargo tarpaulin` を前提とする記述が残っていないことを確認

**Phase 0 完了条件:** CIが動き、閾値40%で成功する。カバレッジの数値自体は未変化。

---

## Phase 1: SNS層と予約コマンドのテスト (閾値 40% → 55%)

対象と現状カバー率:

| ファイル | リージョン | 現状 |
|---|---|---|
| `sns/x.rs` | 94 | 0.00% |
| `sns/misskey.rs` | 81 | 0.00% |
| `sns/mastodon.rs` | 100 | 39.00% |
| `sns/mod.rs` | 50 | 34.00% |
| `commands/schedule.rs` | 348 | 0.00% |

### Task 1.1: SNSクライアントのURL注入可能化

- [ ] 各クライアントがインスタンスURLを外部から注入できるか確認
- [ ] 注入できない構造のものはコンストラクタを追加してリファクタリング
- [ ] `sns/bluesky.rs` の既存テストを参考実装として確認
- [ ] リファクタリング後も既存テストが通ることを確認

### Task 1.2: `sns/misskey.rs` のテスト追加

- [ ] `wiremock` で投稿エンドポイントをモック
- [ ] 正常系: 投稿成功時に `PostResult` が期待通り
- [ ] 異常系: 401応答時に `Err`
- [ ] 異常系: 500応答時に `Err`
- [ ] `max_characters()` の返値を検証
- [ ] 実ネットワークへ接続していないことを確認

### Task 1.3: `sns/x.rs` のテスト追加

- [ ] `wiremock` で投稿エンドポイントをモック
- [ ] 正常系・401・500 を検証
- [ ] `url_char_weight()` が23を返すことを検証
- [ ] `max_characters()` の返値を検証

### Task 1.4: `sns/mastodon.rs` のテスト追加

- [ ] 未カバーの61リージョンを埋めるテストを追加
- [ ] `url_char_weight()` が23を返すことを検証
- [ ] エラー応答時の挙動を検証

### Task 1.5: `sns/mod.rs` のテスト追加

- [ ] `download_image()` のテストを `wiremock` で追加
- [ ] 画像でないコンテンツ(HTML等)を受け取った場合の挙動を検証
- [ ] ネットワークエラー時に `Err` を返すことを検証

### Task 1.6: `commands/schedule.rs` のテスト追加

- [ ] `tempfile::TempDir` を使ったフィクスチャを用意
- [ ] `list` の検証(空・複数件・ステータスフィルタ)
- [ ] `add` の検証(日時指定・`--auto-slot`・SNS指定)
- [ ] `update` の検証(テキスト・日時・SNS・ステータスの変更)
- [ ] `delete` の検証(存在するID・存在しないID)
- [ ] 不正な日時形式を渡した場合にエラーとなることを検証

### Task 1.7: 閾値の引き上げ

- [ ] `just cov` で実測値を確認
- [ ] `coverage-threshold.txt` を「実測値 - 2」へ更新(目標55以上)
- [ ] CIが成功することを確認

**Phase 1 完了条件:** 閾値が55以上に引き上がり、CIが成功する。

---

## Phase 2: commands.rs のテスト (閾値 55% → 65%)

対象: `commands.rs` (1091リージョン、現状19.98%、未カバー873)

### Task 2.1: テスト可能性の調査

- [ ] `commands.rs` 内の関数を列挙し、外部依存(ネットワーク・ファイル・時刻)を洗い出す
- [ ] 純粋関数として切り出せる部分を特定する
- [ ] 800行を超えているため、責務ごとのモジュール分割を検討する

### Task 2.2: 依存の注入可能化

- [ ] ネットワーク依存を持つ処理は注入可能な形へリファクタリング
- [ ] ファイルパス依存は `tempfile` で差し替え可能にする
- [ ] リファクタリング後も既存の9件のテストが通ることを確認

### Task 2.3: テストの追加

- [ ] 各サブコマンドの正常系を検証
- [ ] 設定不足・不正な引数などの異常系を検証
- [ ] ドライラン時に実際の投稿と保存が行われないことを検証

### Task 2.4: 閾値の引き上げ

- [ ] `just cov` で実測値を確認
- [ ] `coverage-threshold.txt` を「実測値 - 2」へ更新(目標65以上)
- [ ] CIが成功することを確認

**Phase 2 完了条件:** 閾値が65以上に引き上がり、CIが成功する。

---

## Phase 3: web/routes.rs のテスト (閾値 65% → 80%)

対象: `web/routes.rs` (1719リージョン、現状0.00%)

design.md 第2節の通り、テストを書く前にルータ構築の抽出が必要である。

### Task 3.1: `build_router()` の抽出

- [ ] `src/web/mod.rs` の `start_server()` からルータ構築部分を `build_router(state: Arc<AppState>) -> Router` として切り出す
- [ ] 抽出はコードの移動のみとし、ロジックを変更しない
- [ ] `start_server()` が `build_router()` を呼ぶ形に書き換える
- [ ] doc comment を付与する
- [ ] 既存の認証ミドルウェアテストが通ることを確認(回帰確認)
- [ ] `cargo run -- serve` で実際にWeb UIが動作することを確認

### Task 3.2: テストフィクスチャの整備

- [ ] `setup_test_router()` がダミーハンドラではなく `build_router()` を使うよう変更
- [ ] ストアのパスを `data/test_scheduled_posts.json` から `tempfile::TempDir` 配下へ変更
- [ ] `TempDir` の生存期間がテスト終了まで保たれるよう戻り値で保持する
- [ ] 変更後に `routes.rs` のカバレッジが0%から上昇することを `just cov` で確認

### Task 3.3: 認証まわりのテスト

- [ ] `GET /login` がログインページを返すことを検証
- [ ] `POST /login` の正常系(正しい資格情報)を検証
- [ ] `POST /login` の異常系(誤った資格情報)を検証
- [ ] `GET /logout` でセッションが破棄されることを検証
- [ ] 未認証で `/api/*` へアクセスした場合に認証エラーとなることを検証

### Task 3.4: 設定・スロット系APIのテスト

- [ ] `GET /api/config` の正常系を検証
- [ ] `GET /api/next-slots` が投稿枠を返すことを検証

### Task 3.5: 予約投稿API のテスト

- [ ] `GET /api/schedules` の正常系(空・複数件)を検証
- [ ] `PUT /api/schedules/{id}` の正常系と存在しないID の場合を検証
- [ ] `DELETE /api/schedules/{id}` の正常系と存在しないID の場合を検証
- [ ] `POST /api/schedules/{id}/post-now` の挙動を検証

### Task 3.6: 投稿・アップロードAPI のテスト

- [ ] `POST /api/post` の正常系を検証(SNS送信はモックすること)
- [ ] `POST /api/upload` のマルチパート送信を検証
- [ ] ボディサイズ上限(10MB)を超えた場合の挙動を検証

### Task 3.7: MCPエンドポイントのテスト

- [ ] `GET /api/mcp/sse` の接続確立を検証
- [ ] `POST /api/mcp/message` の正常系を検証

### Task 3.8: 閾値の80%への引き上げ

- [ ] `just cov` で実測値を確認し80%以上であることを確認
- [ ] `coverage-threshold.txt` を `80` へ更新
- [ ] CIが成功することを確認

**Phase 3 完了条件:** 閾値が80となり、CIが成功する。

---

## Phase 4: 維持

### Task 4.1: 運用ルールの明文化

- [ ] `CLAUDE.md` に「閾値は引き下げない」ルールを明記
- [ ] 新規コードは90%以上を目指す方針との整合を確認
- [ ] カバレッジが閾値を明確に上回った場合の引き上げ手順を記載

### Task 4.2: 中位モジュールの底上げ (任意)

余力があれば以下を改善する。80%維持には必須ではない。

- [ ] `web/mod.rs` (56.07%)
- [ ] `image_resizer.rs` (67.88%)
- [ ] `scheduled/executor.rs` (72.25%)

---

## 検証チェックリスト

全フェーズ完了時に以下を確認する。

- [ ] `cargo llvm-cov --summary-only` のリージョンカバレッジが80%以上
- [ ] CIがPR・pushの両方で起動する
- [ ] 閾値を割り込むとCIが失敗する
- [ ] テストが外部ネットワークへ接続していない
- [ ] テストが `data/` 配下の実ファイルを汚染していない
- [ ] `cargo fmt --check` と `cargo clippy -- -D warnings` が通る
- [ ] `just test-cov` `just cov` がローカルで動作する
- [ ] README / CLAUDE.md の記述が実態と一致している

## 関連

- 要件: `requirements.md`
- 設計: `design.md`
- Issue: #59 #61 #62 #63
