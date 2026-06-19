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

# デバッグ付きドライラン (Rust版は現在 --debug 未実装のため run-dry-run と同様)
debug-dry-run *args='':
    cargo run -- run --dry-run {{args}}

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
    @echo "  just test-cov                      # カバレッジ付きテスト"
    @echo ""
    @echo "環境構築:"
    @echo "  just sync                          # 依存関係の同期"
    @echo ""
    @echo "メンテナンス:"
    @echo "  just touch-rss-posted              # 全RSSフィードを既読にする"
