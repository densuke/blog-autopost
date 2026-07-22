# blog-autopost用justfile (Rust Port)

# ブログ更新チェックと自動投稿
blog-check *args='':
    cargo run -- run {{args}}

# テキストの直接投稿
post-text text *args='':
    cargo run -- post --text "{{text}}" {{args}}

# 利用可能なSNS設定名の一覧表示
list-sns:
    cargo run -- --list-sns

# 登録されているフィードの一覧表示
list-feeds:
    cargo run -- --list-feeds

# ドライラン（テスト実行）
dry-run *args='':
    cargo run -- run --dry-run {{args}}

# デバッグ付きドライラン (詳細ログ付きシミュレーション)
debug-dry-run *args='':
    cargo run -- run --dry-run --debug {{args}}

# テスト実行
test:
    cargo test

# 依存関係の同期
sync:
    cargo fetch

# Webサーバーの起動
run-web:
    cargo run -- serve

# メンテナンスコマンド
touch-rss-posted *args='':
    cargo run -- touch {{args}}

# カバレッジ付きテスト (cargo-llvm-covによるHTMLレポート生成)
# レポートは target/llvm-cov/html/index.html に出力される。
test-cov:
    cargo llvm-cov --html

# カバレッジのサマリのみ表示
cov:
    cargo llvm-cov --summary-only

# カバレッジが閾値(coverage-threshold.txt)を満たすか検査
# CIと同じ判定をローカルで行う。
cov-check:
    #!/usr/bin/env bash
    set -euo pipefail
    threshold=$(cat coverage-threshold.txt)
    echo "カバレッジ閾値: ${threshold}%"
    cargo llvm-cov --all-features --workspace --fail-under-regions "${threshold}" --summary-only

# x86_64 Linux (musl/静的リンク) 向けリリースビルド
# 要: cross (cargo install cross) と起動中のDocker。
# macOS(arm64)などからはQEMUエミュレーション経由でビルドする。
build-x86:
    @command -v cross >/dev/null 2>&1 || ( echo "error: 'cross' が見つかりません。'cargo install cross' でインストールしてください。" && exit 1 )
    DOCKER_DEFAULT_PLATFORM=linux/amd64 cross build --release --target x86_64-unknown-linux-musl
    @echo ""
    @echo "成果物: target/x86_64-unknown-linux-musl/release/blog-autopost-rs"
    @ls -lh target/x86_64-unknown-linux-musl/release/blog-autopost-rs
    @shasum -a 256 target/x86_64-unknown-linux-musl/release/blog-autopost-rs

# 配布用tar.gzを作成 (x86_64 Linux musl バイナリ + static/ + 設定テンプレート)
# 先に build-x86 を実行してから、サーバーへ転送・展開できる形にまとめる。
# 秘密情報を含む config.yml は同梱せず、config.yml.template を入れる。
dist: build-x86
    #!/usr/bin/env bash
    set -euo pipefail
    stage="target/dist/blog-autopost-rs"
    rm -rf "$stage"
    mkdir -p "$stage"
    cp target/x86_64-unknown-linux-musl/release/blog-autopost-rs "$stage/"
    cp -r static "$stage/"
    cp config.yml.template "$stage/"
    [ -f README_RS.md ] && cp README_RS.md "$stage/"
    ts=$(date +%Y%m%d%H%M%S)
    out="target/dist/blog-autopost-rs-x86_64-linux-musl-${ts}.tar.gz"
    tar -czf "$out" -C target/dist blog-autopost-rs
    echo ""
    echo "配布アーカイブ: $out"
    ls -lh "$out"
    shasum -a 256 "$out"
    echo ""
    echo "展開後、blog-autopost-rs ディレクトリ内で config.yml.template を config.yml にコピーして設定してください。"


# 使用例とヘルプ
help:
    @echo "blog-autopost justfile コマンド一覧:"
    @echo ""
    @echo "基本コマンド:"
    @echo "  just blog-check                    # ブログ更新チェックと自動投稿"
    @echo "  just post-text 'テキスト内容'       # テキストの直接投稿"
    @echo "  just list-sns                      # SNS設定一覧表示"
    @echo "  just list-feeds                    # 登録フィード一覧表示"
    @echo ""
    @echo "オプション付きコマンド例:"
    @echo "  just blog-check --sns bluesky      # Blueskyのみに投稿"
    @echo "  just blog-check --sns 'x,mastodon' # XとMastodonに投稿"
    @echo "  just post-text 'テキスト' --sns bluesky"
    @echo "  just post-text 'テキスト' --media image.jpg"
    @echo "  just post-text 'テキスト' --media image.jpg --media video.mp4"
    @echo ""
    @echo "デバッグ・テスト:"
    @echo "  just dry-run                       # ドライラン実行"
    @echo "  just debug-dry-run                 # デバッグ付きドライラン"
    @echo "  just test                          # テスト実行"
    @echo "  just cov                           # カバレッジのサマリ表示"
    @echo "  just test-cov                      # カバレッジ付きテスト(HTMLレポート)"
    @echo "  just cov-check                     # カバレッジ閾値の検査(CIと同じ判定)"
    @echo ""
    @echo "環境構築:"
    @echo "  just sync                          # 依存関係の同期"
    @echo ""
    @echo "リリースビルド:"
    @echo "  just build-x86                     # x86_64 Linux(musl/静的)向けビルド (要 cross + Docker)"
    @echo "  just dist                          # 配布用tar.gz作成 (バイナリ+static+設定テンプレート)"
    @echo ""
    @echo "メンテナンス:"
    @echo "  just touch-rss-posted              # 全RSSフィードを既読にする"
