#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
コマンドライン引数のパース処理

Blog AutoPost CLIのコマンドライン引数を定義・パースする
"""

import argparse
from typing import Any, Callable, Optional


def create_argument_parser(
    touch_rss_posted_handler: Optional[Callable[..., Any]] = None
) -> argparse.ArgumentParser:
    """
    コマンドライン引数パーサーを作成します

    Args:
        touch_rss_posted_handler: touch-rss-postedサブコマンドのハンドラー関数（オプション）

    Returns:
        argparse.ArgumentParser: 設定済みの引数パーサー
    """
    parser = argparse.ArgumentParser(description="ブログの更新をチェックし、SNSにポストします。")

    # 基本オプション
    parser.add_argument("--config", type=str, default="config.yml", help="設定ファイルのパス")
    parser.add_argument("--dry-run", action="store_true", help="ドライランを実行します。")
    parser.add_argument("--limit", type=int, help="処理する記事数を制限します。")
    parser.add_argument("--debug", action="store_true", help="デバッグ情報を表示します。")

    # 直接投稿オプション
    parser.add_argument("--text", type=str, help="指定したテキストを直接SNSに投稿します。")
    parser.add_argument("--sns", type=str, help="投稿するSNSを限定します（カンマ区切りで複数指定可能）。")
    parser.add_argument("--optimize", action="store_true", help="直接投稿時にもテキスト最適化（URL短縮など）を適用します。")
    parser.add_argument("--media", action="append", help="投稿にメディアファイルを添付します（複数回指定可能）。")
    parser.add_argument("--sensitive", action="store_true", help="Misskeyでメディアファイルをセンシティブコンテンツとしてマークします。")

    # 情報表示オプション
    parser.add_argument("--list-sns", action="store_true", help="登録されているSNSアカウントの一覧を表示します。")
    parser.add_argument("--list-feeds", action="store_true", help="登録されているフィードの一覧を表示します。")

    # フィード処理オプション
    parser.add_argument("--feed", type=str, help="処理するフィードを限定します（カンマ区切りで複数指定可能）。")

    # サブコマンド定義
    subparsers = parser.add_subparsers(dest='command', help='利用可能なコマンド')

    # touch-rss-posted サブコマンド
    touch_rss_posted_parser = subparsers.add_parser(
        'touch-rss-posted', help='登録されたRSSフィードのアイテムをすべて投稿済みとしてマークします。'
    )
    touch_rss_posted_parser.add_argument("--dry-run", action="store_true", help="ドライランを実行します。")

    # ハンドラーが提供されている場合のみ設定
    if touch_rss_posted_handler is not None:
        touch_rss_posted_parser.set_defaults(func=touch_rss_posted_handler)

    return parser
