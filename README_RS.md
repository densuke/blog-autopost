# Blog AutoPost CLI (Rust Port)

Blog AutoPost CLIのRust移植版（`blog-autopost-rs`）です。  
本家Python版の主要機能を完全に網羅しつつ、メモリ使用量の削減（KISS原則に基づく軽量化）、高速化、外部ライブラリ依存（Cライブラリなど）の排除、およびクロスコンパイルによるシングルバイナリ配布（x86_64 Linux等への完全静的リンクバイナリ生成）を実現しています。

---

## 🚀 主な特徴（Python版との比較）

1. **ゼロ依存のシングルバイナリ**:
   - `reqwest` と `rustls` を使用しており、SSL/TLS接続も含めて動的共有ライブラリに依存しない静的バイナリを作成できます。
2. **Web UI セキュリティの強化**:
   - `bcrypt`によるパスワード認証とセッションクッキー管理に対応。平文のパスワードでログインすると、初回に自動的に安全な `bcrypt` ハッシュへ `config.yml` を書き換えます（パーミッションも自動的に `600` へ変更し所有者保護を行います）。
3. **Web UI 画像アップロードと自動リサイズ**:
   - ドラッグ＆ドロップ、クリップボードからのペースト、ファイル参照でのアップロードに対応。
   - `src/image_resizer.rs` にて各SNS（X, Bluesky, Mastodon, Misskey）のAPI仕様に合わせた自動リサイズと段階的な品質・スケール圧縮（LANCZOS縮小とJPEG圧縮）が実行され、画像サイズエラーによる投稿失敗を防ぎます。
4. **予約投稿・スケジュール管理のUI完備**:
   - 自動スロット予約のタイミング算出の他、カスタム日時での予約、スケジュールされたポストの一覧表示、編集、個別削除、一括クリーンアップに対応しています。
5. **高度な直接投稿オプション**:
   - `--link` オプションによるリンクカード埋め込み、自動URL短縮（`is.gd`）、および投稿先SNSごとの文字数上限オーバーエラーの事前検知に対応しています。

---

## 🛠 動作要件とビルド

### 動作要件
- **Rust (MSRV 1.75以上)**
- **CMake** (依存する暗号化ライブラリのビルドで必要となる場合があります)
- **just** (コマンドランナー、任意)

### ビルド手順

```bash
# クローン後にディレクトリへ移動
cd blog-autopost-rs

# ビルド（開発用）
cargo build

# リリース用バイナリのビルド (target/release/blog-autopost-rs が生成されます)
cargo build --release
```

### justを使用したコマンド実行
コマンドランナー `just` を導入している場合、定義済みのショートカットが使えます。

```bash
# ヘルプ・利用可能なコマンド一覧を表示
just help
```

---

## 📝 設定 (`config.yml`)

既存のPython版の設定ファイル（`config.yml`）をそのまま使用可能です。Web UI認証を利用するには、`web_auth` セクションを追加してください。

```yaml
# config.yml

# Web UI 認証・セッション設定
web_auth:
  username: "admin"
  password: "changeme"  # 平文で記述してログインすると、自動的にbcryptハッシュに書き換わります

# 投稿タイミングの設定 (allowed_timings)
default_allowed_timings:
  - ["*", ["09:00", "12:00", "18:00"]]
```

---

## 💻 CLIコマンドの使い方

サブコマンドを指定して実行します。

### 1. `run` (ブログ更新チェックと自動投稿)
```bash
# ドライラン（投稿シミュレーション）で実行
cargo run -- run --dry-run

# 直近処理する新規記事の件数を制限して実行
cargo run -- run --limit 2

# 詳細なデバッグログを表示して実行
cargo run -- run --debug
```

### 2. `post` (SNSへのテキスト直接投稿)
```bash
# 全SNSに直接投稿
cargo run -- post -t "これは直接投稿のテストです。"

# 送信先を指定（複数可。-で除外可能）
cargo run -- post -t "テスト" --sns "bluesky,x"

# 画像やリンクカードの添付
cargo run -- post -t "ブログを更新しました" --media "/path/to/image.jpg" --link "https://example.com/article"
```

### 3. `serve` (Web UIダッシュボードの起動)
```bash
# 指定ポート（デフォルト: 8080）で起動
cargo run -- serve --port 9080
```

### 4. `touch` (新着フィードの既読化)
```bash
# 現在のフィード記事をすべて「投稿済み（既読）」としてマーク
cargo run -- touch
```

---

## 🧪 テストとカバレッジ

### テストの実行
```bash
cargo test
# または
just test
```

### カバレッジの測定
カバレッジを測定するには、`cargo-llvm-cov` または `cargo-tarpaulin` の利用を推奨します。

```bash
# cargo-llvm-cov を使用する場合 (インストールの必要あり: cargo install cargo-llvm-cov)
cargo llvm-cov --html

# cargo-tarpaulin を使用する場合
cargo tarpaulin --out Html
```
カバレッジの結果は `tarpaulin-report.html` または `target/llvm-cov/html-details/index.html` として生成され、Webブラウザで詳細を確認できます。
