# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

このファイルを参照するときは、グラウンドルールとして ~/.claude/CLAUDE.md を参照してください。
ワークスペースなど隔離空間で作業しているために読み込めない場合は、利用者にその旨をお知らせください。読めそうな場所に持ち込むよう努力します。

## やりとりをする言語について

このファイルを参照できる場所にある環境では、日本語での会話を原則としてください。
ただ、プログラミング上で発生するエラーメッセージなどは英語のままで表記し、かっこ付けで日本語訳を付けるようにしてください。


## プロジェクト概要

Blog AutoPost は、ブログのRSS/Atomフィードを監視し、新しい記事が投稿された際に各種SNS（X、Bluesky、Misskey、Mastodon）へ自動投稿するツールです。CLIに加えてWeb UIも備えており、手動投稿と予約投稿を管理できます。

実装言語は Rust (edition 2024)、クレート名は `blog-autopost-rs` です。旧Python実装は `python` ブランチに残っており、`main` はRust版のみを対象とします。

## 開発環境とコマンド

`justfile` に主要な操作をまとめてあります。`just help` で一覧を表示できます。

### セットアップ
```bash
just sync  # cargo fetch による依存関係の取得
```

### 実行コマンド
```bash
# デーモンとして起動しスケジュール実行
cargo run -- run

# ドライラン（実際の投稿とデータ保存を行わない）
just dry-run

# デバッグ情報付きドライラン
just debug-dry-run

# フィードを一度だけチェックして投稿
cargo run -- check

# 投稿先SNSを限定（'-名前'で除外、'all'で全件）
cargo run -- check --sns bluesky

# テキストの手動投稿
just post-text 'テキスト内容'

# Web UIの起動（既定ポート 8080）
just run-web

# 予約投稿の管理
cargo run -- schedule list
cargo run -- schedule add --text 'テキスト' --at '2026-08-01T12:00:00+09:00'

# 全フィードを既読にする
just touch-rss-posted

# カスタム設定ファイル使用
cargo run -- --config custom_config.yml run
```

### テスト実行
```bash
# 全テスト実行
just test          # cargo test

# カバレッジ付きテスト（HTMLレポート生成）
just test-cov      # cargo tarpaulin --out Html

# 特定テストのみ実行
cargo test article::store
```

### リリースビルド
```bash
just build-x86  # x86_64 Linux (musl/静的リンク) 向け。要 cross + Docker
just dist       # 配布用tar.gz作成（バイナリ + static/ + 設定テンプレート）
```

## アーキテクチャ概要

### エントリポイント
- `src/main.rs` - 起動処理
- `src/cli.rs` - clap によるCLI定義（サブコマンド: `run` / `check` / `post` / `touch` / `serve` / `schedule`）
- `src/commands.rs`, `src/commands/` - 各サブコマンドの実処理
- `src/runner.rs` - フィードチェックから投稿までの一連の流れ

### コアモジュール
**`src/article/`** - 記事の取得と永続化
- `feed_fetcher.rs` - feed-rs によるRSS/Atomの取得と解析
- `image_extractor.rs` - og:image 等からのアイキャッチ抽出
- `store.rs` - `JsonArticleStore`（JSONファイルへの永続化、既投稿記事の追跡）
- `traits.rs` - `ArticleStore` トレイト

**`src/sns/`** - SNS投稿
- `traits.rs` - `SnsClient` トレイト（`post()`, `max_characters()`, `url_char_weight()`）
- `x.rs` / `bluesky.rs` / `misskey.rs` / `mastodon.rs` - 各SNSの実装

**`src/scheduled/`** - 予約投稿
- `models.rs` - `ScheduledPost`
- `store.rs` - `JsonScheduledPostStore`（Mutexとファイルロックによる排他制御）
- `executor.rs` - 予約投稿の実行

**`src/text/`** - 投稿テキストの生成
- `optimizer.rs` - SNSごとの文字数制限に合わせた最適化
- `tags.rs` - タグ抽出（YouTube概要欄など）

**`src/timing.rs`** - 投稿タイミングの算出（複数タイミング設定に対応）

**`src/web/`** - Web UI
- `mod.rs` / `routes.rs` - axum によるHTTPサーバとAPI
- `templates/index.html`, `templates/login.html` - UI本体（CSS/JSインライン）
- `static/` - 配布時に参照される静的ファイル

**`src/config.rs`** - `config.yml` の読み込み（serde_yaml）
**`src/image_resizer.rs`** - 添付画像のリサイズ

### データ管理
- 記事データ: 既定 `data/articles.json` - 既投稿記事の追跡用
- 予約投稿: JSONファイルへ永続化
- 設定: `config.yml` - SNS認証情報とフィード設定

## 重要な設計パターン

### トレイトによるSNS抽象化
Python版のプラグイン動的読み込みは廃止され、`SnsClient` トレイトによる静的な抽象化になっています。新しいSNSを追加する場合は `src/sns/` にモジュールを作り `SnsClient` を実装し、`src/sns/mod.rs` に登録します。

### 記事の重複排除
`ArticleStore` が前回実行時の記事リストと最新記事を比較し、新着記事のみを抽出します。初回実行時はすべての記事が検出されるため、`--dry-run` での事前確認を推奨します。

### 文字数制限の扱い
X と Mastodon はURLを実長に関わらず23文字として数えるため、`url_char_weight()` を override して吸収しています。

### エラーハンドリング
各SNSへの投稿エラーは個別にキャッチされ、他のSNSへの投稿継続を保証します。エラー型は anyhow を使用します。

## Active Specifications

`.kiro/specs/` 配下で管理しています。

| spec | 状態 |
|---|---|
| `scheduled-post-timing-extension` | 完了 - 各SNSの投稿タイミング設定を複数化し予約投稿機能を拡張 |
| `responsive-design-layout` | 完了 - 広い画面での2カラムレイアウト対応 |
| `unified-post-form-ui` | 完了 - すぐに投稿・時間指定予約・次のタイミング投稿を1つのフォームに統合 |
| `dark-mode-system-integration` | 一部実装 - システム連動(`prefers-color-scheme`)のみ。手動切替UIは未実装 |
| `reserved-post-sns-selection-fix` | 未着手 - 予約投稿編集時にSNSを全て選び直せないバグ |
| `test-coverage-improvement` | 仕様策定済み - カバレッジ80%維持のCI基盤整備と段階的なテスト拡充 (#59 #61 #62 #63) |

## 注意事項

- 設定ファイル`config.yml`はテンプレート（`config.yml.template`）からコピーして作成
- APIキーなどの秘密情報はgitに含めないよう注意
- 応答は日本語で必ず行うこと
- MCPとしてのSerena, Context7の利用を積極的に行うこと
- TDD(テスト開発駆動)を常に意識し、実装前にテストコードを作製して実装をはっきりさせること
- テストファーストとする
- 1トピック1コミットとする(cherry-pick可能なレベル)
- コミットメッセージも日本語で行うこと
- 公開API・構造体・トレイトには doc comment (`///`) を必ず記述すること
- コードカバレッジは全体で70%以上、新規コードは90%以上を目指すこと
    - カバレッジ対応はタスク内のサブタスク時は不要、タスク終了時にまとめてチェックする
- コミットの際は `cargo fmt` と `cargo clippy` による検査を行うこと
- 微小な修正でなければ必ずブランチを切って作業を行うこと、mainへのマージは勝手に行わず勝手に問い合わせること(PRにする場合もありえます)

