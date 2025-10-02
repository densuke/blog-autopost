# blog-autopost用justfile

# ブログ更新チェックと自動投稿
blog-check *args='':
    uv run -m src.main {{args}}

# テキストの直接投稿
post-text text *args='':
    uv run -m src.main --text "{{text}}" {{args}}

# 利用可能なSNS設定名の一覧表示
list-sns:
    uv run -m src.main --list-sns

# 登録されているフィードの一覧表示
list-feeds:
    uv run -m src.main --list-feeds

# ドライラン（テスト実行）
dry-run *args='':
    uv run -m src.main --dry-run {{args}}

# デバッグ付きドライラン
debug-dry-run *args='':
    uv run -m src.main --debug --dry-run {{args}}

# テスト実行
test:
    uv run pytest

# カバレッジ付きテスト実行  
test-cov:
    uv run pytest --cov=src

# 依存関係の同期
sync:
    uv sync

# Webサーバーの起動
run-web:
    uv run uvicorn src.web.main_web:app --reload

# メンテナンスコマンド
touch-rss-posted *args='':
    uv run -m src.main touch-rss-posted {{args}}

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
