#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
Blog AutoPost CLI メインモジュール（互換レイヤー）

このモジュールは後方互換性のために残されています。
実装は src/cli/ モジュール群に移行されました。
"""

# 外部依存もインポート（テストの互換性のため）
import argparse
import os
import re
from urllib.parse import urljoin

import requests
from bs4 import BeautifulSoup

# 後方互換性のために全てのシンボルを再エクスポート
from .article_manager import ArticleManager, MultiArticleManager
from .cli.argument_parser import create_argument_parser
from .cli.feed_handler import (
    handle_list_feeds,
    handle_touch_rss_posted,
    process_rss_feeds,
)
from .cli.main_cli import main
from .cli.media_handler import extract_image_from_url, process_media_files
from .cli.post_handler import (
    execute_sns_posting,
    handle_direct_text_post,
    handle_list_sns,
    setup_text_optimization,
    validate_and_filter_plugins,
)
from .config_manager import ConfigManager, load_config
from .plugin_loader import load_plugins
from .utils.html_utils import decode_html_content

__all__ = [
    # CLI関連
    'create_argument_parser',
    'main',
    # フィード処理
    'handle_list_feeds',
    'handle_touch_rss_posted',
    'process_rss_feeds',
    # メディア処理
    'extract_image_from_url',
    'process_media_files',
    # 投稿処理
    'execute_sns_posting',
    'handle_direct_text_post',
    'handle_list_sns',
    'setup_text_optimization',
    'validate_and_filter_plugins',
    # 後方互換性のために再エクスポート
    'ArticleManager',
    'MultiArticleManager',
    'ConfigManager',
    'load_config',
    'load_plugins',
    # 外部依存
    'argparse',
    'os',
    're',
    'urljoin',
    'requests',
    'BeautifulSoup',
    'decode_html_content',
]


if __name__ == "__main__":
    main()
