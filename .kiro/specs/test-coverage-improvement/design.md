# Design Document: test-coverage-improvement (Rust版)

## 1. 設計の要点

本specの中心は「カバレッジを測る仕組み」と「測れる形にコードを直す」の2つである。
特に後者について、現状を調査した結果、単にテストを書き足すだけでは `web/routes.rs`
のカバレッジは上がらないことが判明した。以下で詳述する。

## 2. なぜ `web/routes.rs` が0%なのか

`src/web/mod.rs` にはすでに `axum` のテストが存在し、`tower::ServiceExt::oneshot` を
使った認証ミドルウェアのテストが通っている。しかし `routes.rs` のカバレッジは0%のままである。

原因は、テストヘルパ `setup_test_router()` が**本物のハンドラを使っていない**ことにある。

```rust
// src/web/mod.rs のテスト内 (現状)
let api_routes = Router::new()
    .route("/config", get(|| async { "config data" }))   // ← ダミーハンドラ
    .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware));
```

一方、本番のルータは `start_server()` の中でインラインに組み立てられている。

```rust
// src/web/mod.rs の start_server() 内 (現状)
let api_routes = Router::new()
    .route("/config", get(routes::get_config))          // ← 本物のハンドラ
    .route("/post", post(routes::manual_post))
    ...
```

`start_server()` は `TcpListener` のバインドと `axum::serve()` を含むため、テストから
呼び出せない。結果としてルータ定義がテストへ再利用できず、テスト側でダミーを組み直す
しかなくなっている。これがミドルウェアだけが検証され、ハンドラ本体が一度も実行されない
構造を生んでいる。

### 対策: ルータ構築の抽出

`start_server()` からルータ構築部分を純粋な関数として切り出す。

```rust
/// アプリケーションのルータを構築する。
///
/// サーバの起動処理とは分離してあり、テストから `tower::ServiceExt::oneshot`
/// を用いて本物のハンドラを直接検証できる。
pub fn build_router(state: Arc<AppState>) -> Router {
    let api_routes = Router::new()
        .route("/config", get(routes::get_config))
        .route("/post", post(routes::manual_post))
        // ... 既存の定義をそのまま移す
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .nest("/api", api_routes)
        .route("/login", get(routes::get_login_page).post(routes::login_submit))
        .route("/logout", get(routes::logout))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn start_server(config: Config, config_path: String, port: u16) -> anyhow::Result<()> {
    let state = /* 既存の構築処理 */;
    let app = build_router(state);
    let addr = format!("0.0.0.0:{}", port);
    println!("Web UI listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

これにより既存の `setup_test_router()` はダミーを組む必要がなくなり、`build_router()`
を呼ぶだけになる。`routes.rs` の1719リージョンがテストの射程に入る。

この抽出はリファクタリングであり、外部から見た挙動は変わらない。既存の認証ミドルウェア
テストが通り続けることをもって回帰がないことを確認する。

## 3. テスト用フィクスチャの設計

### 3.1 現状の問題

既存のテストヘルパは実ファイルパスを直接使っている。

```rust
let store = Arc::new(JsonScheduledPostStore::new("data/test_scheduled_posts.json"));
```

テスト間で状態が共有され、並行実行時に競合しうる。`tempfile` は既に依存関係へ入って
いるため、これを使う。

### 3.2 共通フィクスチャ

```rust
#[cfg(test)]
mod test_support {
    use tempfile::TempDir;

    /// テスト用のAppStateと、生存期間を束ねるTempDirを返す。
    ///
    /// TempDirは戻り値として保持し続けること。ドロップされると
    /// 一時ディレクトリごと削除される。
    pub async fn test_state(secret_key: Option<String>) -> (Arc<AppState>, TempDir) {
        let dir = TempDir::new().expect("一時ディレクトリの作成に失敗");
        let store_path = dir.path().join("scheduled_posts.json");
        // ... AppState を組み立てる
        (state, dir)
    }
}
```

同じ考え方を `commands/schedule.rs` および `scheduled/store.rs` のテストにも適用する。

## 4. SNSクライアントのテスト設計

`wiremock` が dev-dependencies に入っているため、これを使って外部SNSのAPIをモックする。
`sns/bluesky.rs` は既に4件のテストを持ち42.67%に達しているので、同じ形を
`sns/x.rs` `sns/misskey.rs` `sns/mastodon.rs` へ展開する。

```rust
#[tokio::test]
async fn test_post_returns_error_on_http_500() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/notes/create"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let client = MisskeyClient::new(/* server.uri() を注入 */);
    let result = client.post(&content).await;

    assert!(result.is_err());
}
```

各クライアントについて最低限以下を検証する。

| 観点 | 内容 |
|---|---|
| 正常系 | 投稿が成功し `PostResult` が期待通り |
| 認証エラー | 401応答時に `Err` を返す |
| サーバエラー | 500応答時に `Err` を返す |
| 文字数制限 | `max_characters()` が仕様通りの値を返す |
| URL重み | `url_char_weight()` が X/Mastodon で23を返す |

**ネットワークへ実接続してはならない。** すべて `MockServer::uri()` を注入する形にする。
インスタンスURLを外部から注入できない構造になっているクライアントは、注入可能な形へ
リファクタリングする。

## 5. カバレッジ計測基盤

### 5.1 ツール選定

`cargo-llvm-cov` を採用する。理由は以下の通り。

| 項目 | cargo-llvm-cov | cargo-tarpaulin |
|---|---|---|
| 導入状況 | 環境に導入済み | 未導入・動作しない |
| 対応プラットフォーム | Linux / macOS(arm64含む) | Linuxが主 |
| 閾値指定 | `--fail-under-lines` 等 | `--fail-under` |
| CI導入 | `taiki-e/install-action` で高速導入 | ビルドが必要で低速 |

`justfile` の `test-cov` は現在 tarpaulin 決め打ちで動作しないため、置き換える。

```make
# カバレッジ計測 (HTMLレポート生成)
test-cov:
    cargo llvm-cov --html

# カバレッジのサマリのみ表示
cov:
    cargo llvm-cov --summary-only
```

### 5.2 閾値の管理

閾値をワークフローYAML内へ直接書くと、引き上げのたびにワークフローを触ることになり
変更履歴が追いにくい。リポジトリルートの単一ファイルで管理する。

```
coverage-threshold.txt   # 中身は数値のみ。例: 40
```

CIはこのファイルを読み、`cargo llvm-cov --fail-under-regions "$(cat coverage-threshold.txt)"`
の形で使う。閾値の変更が1行のdiffとして履歴に残る。

### 5.3 ラチェットの運用

閾値は**引き下げてはならない**。テスト追加でカバレッジが閾値を明確に上回ったら、
同じPR内で `coverage-threshold.txt` を引き上げる。「明確に上回る」の目安は
実測値から2ポイント引いた値とし、計測の揺らぎでCIが不安定になるのを避ける。

```
実測 58.3% を達成 → 閾値は 56 へ引き上げる (58 - 2)
```

## 6. CIワークフロー設計

新規に `.github/workflows/ci.yml` を追加する。既存の `dependabot-automerge.yml` は
dependabot専用のまま残し、責務を分ける。

```yaml
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  check:
    runs-on: ubuntu-24.04
    steps:
      - name: ソースの取得
        uses: actions/checkout@v4

      - name: Rustツールチェインのセットアップ
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy, llvm-tools-preview

      - name: ビルドキャッシュ
        uses: Swatinem/rust-cache@v2

      - name: cargo-llvm-cov の導入
        uses: taiki-e/install-action@cargo-llvm-cov

      - name: 書式検査
        run: cargo fmt --all -- --check

      - name: 静的解析
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: テストとカバレッジ計測
        run: |
          threshold=$(cat coverage-threshold.txt)
          echo "カバレッジ閾値: ${threshold}%"
          cargo llvm-cov --all-features --workspace \
            --fail-under-regions "${threshold}" \
            --summary-only | tee coverage-summary.txt

      - name: カバレッジサマリの出力
        if: always()
        run: |
          {
            echo "## カバレッジ計測結果"
            echo ""
            echo '```'
            cat coverage-summary.txt
            echo '```'
          } >> "$GITHUB_STEP_SUMMARY"
```

### 設計上の判断

- **テストとカバレッジを同一ジョブにする**: `cargo llvm-cov` はテストを実行しながら
  計測するため、テスト専用ジョブを別に立てるとビルドとテストが二重に走る。
- **`act` 対応**: 本ワークフローは外部サービスへ送信しないため `ACT` による分岐は
  不要だが、`taiki-e/install-action` が act 上で失敗する場合は
  `if: ${{ !env.ACT }}` によるスキップを検討する。
- **`dependabot-automerge.yml` は変更しない**: 自動マージ側でもテストは走っており、
  そちらへカバレッジ閾値を持ち込むと依存更新が閾値未達で止まるリスクがある。

## 7. 段階的な到達計画

| 段階 | 閾値 | 対象 | 見込み増分 |
|---|---|---|---|
| 初期 | 40% | 基盤整備のみ。現状42%の下限を固定 | - |
| 第1段階 | 55% | `sns/x.rs` `sns/misskey.rs` `sns/mastodon.rs` `sns/mod.rs` `commands/schedule.rs` | +約780リージョン |
| 第2段階 | 65% | `commands.rs` | +約870リージョン |
| 第3段階 | 80% | `web/routes.rs` (`build_router` 抽出後) | +約1700リージョン |

### 到達可能性

全体7111リージョンに対し80%は5689リージョン。現在のカバー済みは2991。
第1〜第3段階の見込み増分の合計は約3350であり、2991 + 3350 = 6341 (89%) となる。
すべてを完全にカバーする必要はなく、余裕を持って80%へ到達できる。

## 8. リスクと対策

| リスク | 対策 |
|---|---|
| `build_router` 抽出で本番挙動が変わる | 既存の認証ミドルウェアテストを回帰テストとして使う。抽出はコードの移動のみに留め、ロジックを変更しない |
| CIの実行時間が延びる | `Swatinem/rust-cache` を利用。`taiki-e/install-action` はビルド済みバイナリを取得するため導入は数秒 |
| 閾値が厳しくCIが常時赤 | 初期閾値を現状値未満の40%から始める。引き上げはテスト追加と同一PR内で行う |
| SNSクライアントのテストが外部へ接続 | `wiremock` の `MockServer::uri()` を注入する構造へ統一。レビュー時に実URLのハードコードがないか確認する |
| 計測値の揺らぎで閾値割れ | 閾値は実測値から2ポイント引いた値に設定する |

## 9. スコープ外

- `main.rs` のカバレッジ (起動処理のみのため計測対象外)
- Codecov等の外部サービス連携
- E2E・ブラウザテスト
- `python` ブランチのカバレッジ

## 10. 関連

- 要件: `requirements.md`
- タスク: `tasks.md`
- Issue: #59 #61 #62 #63
