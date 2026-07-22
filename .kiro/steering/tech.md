# 技術スタック (tech.md)

## 1. アーキテクチャ

Rust (edition 2024) 単一クレート `blog-autopost-rs` として実装されており、
1つのバイナリがCLIとWebサーバの両方を提供します。旧Python/FastAPI実装は
`python` ブランチにのみ存在し、`main` には含まれません。

主要なコンポーネントは以下の通りです。

- **CLI (`src/cli.rs`, `src/commands/`)**: `clap` によるサブコマンド定義と、
  その実処理。`main.rs` は設定読込とディスパッチのみを行います。
- **フィード処理 (`src/article/`)**: `feed-rs` によるRSS/Atom解析、
  アイキャッチ抽出、投稿済み記事のJSON永続化。
- **投稿パイプライン (`src/runner.rs`, `src/text/`)**: 新着記事の抽出、
  テンプレート適用、SNSごとの文字数制限に合わせたテキスト最適化。
- **SNSクライアント (`src/sns/`)**: `SnsClient` トレイトと
  X / Bluesky / Misskey / Mastodon の各実装。
- **予約投稿 (`src/scheduled/`)**: JSONファイルによるストアと実行器。
- **タイミング管理 (`src/timing.rs`)**: 曜日と時刻の許可リストから
  次に投稿可能な枠を算出します。
- **Web UI / API (`src/web/`, `static/`)**: `axum` によるHTTPサーバ。
  画面は `static/index.html` と `static/login.html` を直接配信します。
- **デーモン (`src/commands/daemon.rs`)**: `tokio-cron-scheduler` により
  フィード監視と予約投稿の実行を毎分起動します。

## 2. 使用技術

### 2.1. 言語とツールチェイン

- **Rust (edition 2024)**。ツールチェインは stable を前提とします。
- 依存関係は `Cargo.toml` / `Cargo.lock` で管理します。

### 2.2. 主要クレート

バージョンは `Cargo.toml` を参照してください(ここには記載しません)。

- **非同期ランタイム**: `tokio`, `async-trait`, `tokio-stream`
- **CLI**: `clap` (derive, env)
- **HTTPサーバ**: `axum` (multipart), `tower-http` (fs, cors)
- **HTTPクライアント**: `reqwest` (json, multipart, rustls)
- **フィード解析**: `feed-rs`
- **シリアライズ**: `serde`, `serde_json`, `serde_yaml`
- **スケジューリング**: `tokio-cron-scheduler`, `chrono`
- **認証・排他**: `bcrypt` (パスワードハッシュ), `fd-lock` (ファイルロック)
- **メディア**: `image` (リサイズ), `tempfile`
- **その他**: `anyhow` (エラー), `regex`, `urlencoding`,
  `oauth1-request` (X API の署名)
- **開発用**: `wiremock` (HTTPスタブ), `tower` (Router のテスト)

### 2.3. 永続化

データベースは使用しません。以下のJSONファイルで完結します。

- `data/articles.json`: 投稿済み記事の記録
- `data/scheduled_posts.json`: 予約投稿
- `data/uploads/`: Web UI からのアップロードメディア

書き込みは一時ファイルと rename による原子的な置き換えで行い、
予約投稿ストアはプロセス内 `Mutex` とプロセス間 `fd-lock` で排他します。

## 3. 開発環境

### 3.1. 必須ツール

- **Rust ツールチェイン** (stable。`rustfmt`, `clippy` を含む)
- **`git`**
- **`just`**: コマンドランナー(推奨)
- **`cargo-llvm-cov`**: カバレッジ計測
- **`cross` と Docker**: x86_64 Linux 向けのクロスビルド時のみ

### 3.2. セットアップ

```bash
git clone <repository_url>
cd blog-autopost
just sync                       # cargo fetch
cp config.yml.template config.yml
```

## 4. 主要なコマンド

`justfile` に定義されています。`just help` で一覧を表示できます。

- フィードのチェックと投稿: `cargo run -- check`
- デーモン起動: `cargo run -- run`
- ドライラン: `just dry-run` / `just debug-dry-run`
- 手動投稿: `just post-text 'テキスト内容'`
- 予約投稿の操作: `cargo run -- schedule list` / `schedule add` など
- 全フィードを既読化: `just touch-rss-posted`
- Web UI 起動: `just run-web` (`cargo run -- serve`、既定ポート 8080)
- テスト: `just test` (`cargo test`)
- カバレッジ: `just cov` / `just test-cov` / `just cov-check`
- リリースビルド: `just build-x86` / `just dist`

コミット前には `cargo fmt` と `cargo clippy` を実行します。

## 5. CI

`.github/workflows/ci.yml` が main への push と PR で以下を実行します。

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo llvm-cov` によるテストとカバレッジ計測

カバレッジ閾値は `coverage-threshold.txt` で管理し、CIが
`--fail-under-regions` で判定します。**閾値は引き下げないこと**。
テスト追加で上回ったら同じPR内で引き上げるラチェット方式を取ります。

## 6. 設定と環境変数

設定は `config.yml` に集約します(`config.yml.template` からコピーして作成)。
主な項目は監視フィード(`blog`)、SNS認証情報(`sns`)、投稿テンプレート
(`templates`)、投稿タイミング(`default_allowed_timings` /
`allowed_timings`)、Web認証(`web_auth`)です。

環境変数は一部のCLIオプションの上書きに使えます(`clap` の `env` 属性。
例: `SNS_URL`, `SNS_TOKEN`)。秘密情報を含む `config.yml` はgitに含めません。

`web_auth.password` は平文を検出すると起動時に bcrypt ハッシュへ
自動移行します。

## 7. ポート

Web UI は `cargo run -- serve --port <番号>` で起動します。既定は 8080 で、
`0.0.0.0` で待ち受けます。外部公開する場合は前段のリバースプロキシや
ファイアウォールで制御してください。CLIのみを利用する場合はポートを
使用しません。
