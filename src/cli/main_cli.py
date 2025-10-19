#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
メインCLI統合モジュール

各ハンドラーを統合し、コマンドライン実行を制御
"""

import src.config_manager
import src.cli.feed_handler
import src.cli.post_handler
from .argument_parser import create_argument_parser


def main() -> None:
    """
    メインエントリーポイント

    コマンドライン引数をパースし、適切なハンドラーを実行します
    """
    # 引数パーサーを作成（ハンドラーを注入）
    parser = create_argument_parser(touch_rss_posted_handler=src.cli.feed_handler.handle_touch_rss_posted)
    args = parser.parse_args()

    # 設定ファイル読み込み
    config_data = src.config_manager.load_config(args.config)
    config_manager = src.config_manager.ConfigManager(config_data)

    # サブコマンドの処理
    if hasattr(args, 'func'):
        # dry-run引数がない場合に備えてデフォルト値を設定
        if not hasattr(args, 'dry_run'):
            args.dry_run = False
        args.func(args, config_manager)
        return

    # SNS一覧表示モードかどうかチェック
    if args.list_sns:
        src.cli.post_handler.handle_list_sns(config_manager)
        return

    # フィード一覧表示モードかどうかチェック
    if args.list_feeds:
        src.cli.feed_handler.handle_list_feeds(config_manager)
        return

    # 直接テキスト投稿モードかどうかチェック
    if args.text:
        src.cli.post_handler.handle_direct_text_post(args, config_manager)
        return

    # 通常のRSS監視モード
    src.cli.feed_handler.process_rss_feeds(args, config_manager)


if __name__ == "__main__":
    main()
