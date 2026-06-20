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

### 🔑 外部APIキー認証
外部のスクリプトやツールから直接 Web API を操作して予約投稿を管理する場合、`web_auth` 内の `secret_key` を APIキーとして利用できます。

```yaml
web_auth:
  username: "admin"
  password: "changeme"
  secret_key: "your-api-secret-token" # 外部連携用のAPIキー
```

HTTPリクエストのヘッダーに以下を付与することで、セッションCookieなしで認証を通過できます。
* `Authorization: Bearer your-api-secret-token`
* または `X-Api-Key: your-api-secret-token`

例:
```bash
curl -H "Authorization: Bearer your-api-secret-token" http://localhost:8080/api/schedules
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

### 5. `schedule` (予約投稿のCLI管理)
ブラウザやWeb UIを使わずに、CLIから直接予約投稿の一覧、追加、削除、変更を行えます。
```bash
# 予約投稿の一覧を表示（予定時刻順）
cargo run -- schedule list

# 投稿ステータス（予約済み, 投稿済み, 失敗）でフィルタして表示
cargo run -- schedule list --status "予約済み"

# 特定の日時に予約を追加（添付画像・リンク対応）
cargo run -- schedule add --text "テスト投稿" --at "2026-06-20T18:00:00+09:00" --sns "bluesky" --media "/path/to/img.png" --link "https://example.com"

# 次の投稿可能空き枠（タイミング設定）を自動計算して予約を追加
cargo run -- schedule add --text "自動枠予約のテスト" --auto-slot --sns "mastodon"

# IDを指定して予約投稿を変更（テキスト、予定時間、ステータス等）
cargo run -- schedule update post-1234567890 --text "変更後のテキスト" --status "予約済み"

# IDを指定して予約投稿を削除
cargo run -- schedule delete post-1234567890
```
*(※ `--media` オプションでローカルの画像を予約に添付した際、元のファイルが削除されても送信時にエラーにならないよう、自動的に `data/uploads/` ディレクトリへ安全にコピー退避されます)*

---

## 🔄 Python版からのデータ移行（移行マニュアル）

既存の Python 版 `blog-autopost` からデータ（既読記事データ・予約投稿データ・添付メディア）を Rust版へ一括で移行するためのスクリプト `scripts/migrate.py` が用意されています。

### 移行手順

1. **Python版のプロセス（デーモ等）を停止する**
   移行中の書き込み競合を防ぐため、稼働中の Python 版スケジューラを停止してください。
2. **移行スクリプトを実行する**
   Rust版プロジェクトのルートディレクトリで、`python3` を使用して移行スクリプトを実行します。
   ```bash
   # --python-dir に Python版のプロジェクトディレクトリパスを指定します（デフォルト: ../blog-autopost）
   python3 scripts/migrate.py --python-dir ../blog-autopost
   ```
3. **移行されるデータ**
   * **既読記事データ**: Python版の複数の JSON ファイル（`data/articles_*.json`）から読み込まれ、自動的に重複排除の上で `data/articles.json` にマージされます。
   * **予約投稿データ**: SQLite データベース（`data/scheduled_posts.db`）からすべての予約（履歴を含む）が読み込まれ、Rust版の `data/scheduled_posts.json` に変換されます。
   * **添付メディア**: 予約に添付されている画像ファイル（`data/scheduled_media/` 内）は、自動的に Rust版の `data/uploads/` にコピー退避され、データベース内の参照パスも自動的に書き換わります。
4. **Rust版の起動**
   移行完了後、Rust版を起動し、Web UI または CLI (`cargo run -- schedule list`) でデータが正常に表示されるか確認してください。

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
