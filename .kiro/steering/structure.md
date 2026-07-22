# プロジェクト構造 (structure.md)

現行実装は Rust (edition 2024) 単一クレート `blog-autopost-rs` です。
旧Python/FastAPI実装は `python` ブランチにのみ存在し、`main` には含まれません。

## 1. ルートディレクトリ構成

```
/
├── .env.example         # 環境変数のテンプレート
├── .github/workflows/   # CI(書式検査・clippy・テスト・カバレッジ)とリリース
├── .kiro/               # steering(本ファイル群)と specs
├── CLAUDE.md            # Claude Code 向けのガイド
├── Cargo.toml           # クレート定義と依存関係
├── Cargo.lock           # 依存関係のロック
├── config.yml.template  # 設定ファイルのテンプレート
├── coverage-threshold.txt # カバレッジ閾値(CIが判定。引き下げ禁止)
├── justfile             # just コマンドランナーの定義
├── README.md            # 利用者向けドキュメント
├── docs/                # 補足ドキュメント
├── src/                 # ソースコード
├── static/              # Web UI の実体(index.html, login.html)
└── data/                # 実行時に生成されるデータ(git管理外)
```

`tests/` ディレクトリは存在しません。テストはすべて各モジュール内の
`#[cfg(test)] mod tests` に置く方針です。

## 2. `src/` の構成

```
src/
├── main.rs              # エントリポイント。設定読込とディスパッチのみ
├── lib.rs               # ライブラリのモジュール宣言
├── cli.rs               # clap によるCLI定義
├── commands.rs          # サブコマンドのディスパッチ
├── commands/            # 各サブコマンドの実処理
├── runner.rs            # フィード取得から投稿までの一連の流れ
├── config.rs            # config.yml の読み込み
├── timing.rs            # 投稿タイミングの算出
├── image_resizer.rs     # 添付画像のリサイズ
├── article/             # 記事の取得・抽出・永続化
├── sns/                 # SNSクライアント
├── scheduled/           # 予約投稿
├── text/                # 投稿テキストの生成
└── web/                 # Web UI / API サーバ
```

### `src/commands/` - サブコマンドの実処理

`commands.rs` は入力を振り分けるだけで、実処理は以下へ分割しています。

- `check.rs`: フィードを一度だけチェックして投稿する
- `daemon.rs`: `tokio-cron-scheduler` でフィード監視と予約投稿実行を定期起動する
- `post.rs`: 任意テキストの手動投稿
- `schedule.rs`: 予約投稿の一覧・追加・削除・変更
- `touch.rs`: 現在のフィードをすべて既読として記録する
- `list.rs`: SNS一覧とフィード一覧の表示
- `sns_clients.rs`: 設定から `SnsClient` 群を構築する
- `sns_selector.rs`: `--sns` 指定によるフィルタ(`-名前`で除外、`all`で全件)
- `length_check.rs`: 文字数超過の検査

### `src/article/` - 記事の取得と永続化

- `feed_fetcher.rs`: `feed-rs` によるRSS/Atomの取得と解析
- `image_extractor.rs`: og:image 等からのアイキャッチ抽出
- `store.rs`: `JsonArticleStore`。一時ファイルと rename で原子的に書き換える
- `traits.rs`: `ArticleStore` トレイト
- `models.rs`: `Article`

### `src/sns/` - SNS投稿

- `traits.rs`: `SnsClient` トレイト(`name` / `account_name` / `post` /
  `max_characters` / `url_char_weight`)
- `x.rs` / `bluesky.rs` / `misskey.rs` / `mastodon.rs`: 各SNSの実装
- `models.rs`: `PostContent` と `PostResult`
- `mod.rs`: 画像ダウンロードと画像形式判定の共通処理

### `src/scheduled/` - 予約投稿

- `models.rs`: `ScheduledPost`
- `store.rs`: `JsonScheduledPostStore`。プロセス内は `tokio::sync::Mutex`、
  プロセス間は `fd-lock` によるファイルロックで排他する
- `executor.rs`: 実行時刻を過ぎた予約を取り出して投稿する

### `src/text/` - 投稿テキストの生成

- `traits.rs`: `TextOptimizer` トレイト
- `optimizer.rs`: SNSごとの文字数制限に合わせた最適化
- `tags.rs`: ハッシュタグ抽出

### `src/web/` - Web UI と API

- `mod.rs`: axum の `Router` 構築、`AppState`、認証ミドルウェア、サーバ起動
- `routes.rs`: 各ハンドラ(設定取得、手動投稿、メディアアップロード、
  予約のCRUD、次スロット取得、ログインとログアウト、MCP用SSE)

HTMLは `static/index.html` と `static/login.html` を直接配信します
(`ServeDir` によるフォールバック)。テンプレートエンジンは使いません。
かつて存在した `src/web/templates/` は死にコードとして削除済みです。

### `data/` - 実行時データ

- `data/articles.json`: 投稿済み記事の記録
- `data/scheduled_posts.json`: 予約投稿
- `data/uploads/`: Web UI からアップロードされたメディア

データベースは使用しません。永続化はすべてJSONファイルです。

## 3. コード構成パターン

- **トレイトによる抽象化**: `SnsClient` / `ArticleStore` / `TextOptimizer` を
  境界に置き、実装の差し替えとテスト時のモック化を可能にしています。
  新しいSNSは `src/sns/` にモジュールを追加し、`sns/mod.rs` と
  `commands/sns_clients.rs` に登録します。動的プラグイン機構はありません。
- **設定駆動**: 監視フィード、SNS認証情報、投稿タイミング、Web認証を
  `config.yml` に集約しています。
- **薄いエントリポイント**: `main.rs` と `commands.rs` は振り分けのみを担い、
  実処理は各モジュールへ寄せます。
- **CLIとWebでロジックを共有**: 双方が同じ `SnsClient` 実装と
  `JsonScheduledPostStore` を利用します。
- **エラーの局所化**: 各SNSへの投稿失敗は個別に捕捉し、他SNSへの投稿を
  継続します。エラー型は `anyhow` を使用します。

## 4. 命名規則

- モジュールとファイル: スネークケース(例: `image_resizer.rs`)
- 型とトレイト: アッパーキャメルケース(例: `JsonArticleStore`)
- 定数: 大文字スネークケース
- テスト関数: `test_` プレフィックス(例: `test_feed_targets_skips_empty_url`)

## 5. ドキュメンテーション

公開API・構造体・トレイトには doc comment (`///`) を必ず記述します。
自明でない設計判断(排他制御の方式、SIGPIPEの扱いなど)は、その理由を
コメントとして残します。

## 6. 主要な設計原則

- **拡張性**: SNSの追加はトレイト実装1つとその登録で完結します。
- **堅牢性**: JSONの書き込みは原子的に行い、ファイルロックで多重起動に備えます。
- **安全な確認手段**: `--dry-run` / `--debug` / `--verbose` / `--list-sns` /
  `--list-feeds` により、実投稿の前に挙動を確認できます。
- **テストの近接配置**: テストは対象モジュール内に置き、カバレッジは
  `coverage-threshold.txt` の閾値をCIで判定します。
