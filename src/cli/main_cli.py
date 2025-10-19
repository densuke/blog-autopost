#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
メインCLI統合モジュール

各ハンドラーを統合し、コマンドライン実行を制御
"""

from ..config_manager import ConfigManager, load_config
from .argument_parser import create_argument_parser
from .feed_handler import handle_list_feeds, handle_touch_rss_posted, process_rss_feeds
from .post_handler import handle_direct_text_post, handle_list_sns


def main() -> None:
    """
    メインエントリーポイント

    コマンドライン引数をパースし、適切なハンドラーを実行します
    """
    # 引数パーサーを作成（ハンドラーを注入）
    parser = create_argument_parser(touch_rss_posted_handler=handle_touch_rss_posted)
    args = parser.parse_args()

    # 設定ファイル読み込み
    config_data = load_config(args.config)
    config_manager = ConfigManager(config_data)

    # サブコマンドの処理
    if hasattr(args, 'func'):
        # dry-run引数がない場合に備えてデフォルト値を設定
        if not hasattr(args, 'dry_run'):
            args.dry_run = False
        args.func(args, config_manager)
        return

    # SNS一覧表示モードかどうかチェック
    if args.list_sns:
        handle_list_sns(config_manager)
        return

    # フィード一覧表示モードかどうかチェック
    if args.list_feeds:
        handle_list_feeds(config_manager)
        return

    # 直接テキスト投稿モードかどうかチェック
    if args.text:
        handle_direct_text_post(args, config_manager)
        return

    # 通常のRSS監視モード
    process_rss_feeds(args, config_manager)


if __name__ == "__main__":
    main()
