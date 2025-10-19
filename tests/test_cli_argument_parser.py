#!/usr/bin/env python
# -*- coding: utf-8 -*-

import pytest

from src.cli.argument_parser import create_argument_parser


class TestCreateArgumentParser:
    """create_argument_parser関数のテスト"""

    def test_parser_creation(self):
        """パーサーが正常に作成されることを確認"""
        parser = create_argument_parser()
        assert parser is not None
        assert parser.description == "ブログの更新をチェックし、SNSにポストします。"

    def test_default_arguments(self):
        """デフォルト引数が正しく設定されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args([])

        assert args.config == "config.yml"
        assert args.dry_run is False
        assert args.limit is None
        assert args.debug is False
        assert args.text is None
        assert args.sns is None
        assert args.list_sns is False
        assert args.list_feeds is False
        assert args.feed is None
        assert args.optimize is False
        assert args.media is None
        assert args.sensitive is False

    def test_config_argument(self):
        """--configオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--config", "custom.yml"])
        assert args.config == "custom.yml"

    def test_dry_run_argument(self):
        """--dry-runオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--dry-run"])
        assert args.dry_run is True

    def test_limit_argument(self):
        """--limitオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--limit", "5"])
        assert args.limit == 5

    def test_debug_argument(self):
        """--debugオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--debug"])
        assert args.debug is True

    def test_text_argument(self):
        """--textオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--text", "テスト投稿"])
        assert args.text == "テスト投稿"

    def test_sns_argument(self):
        """--snsオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--sns", "x,bluesky"])
        assert args.sns == "x,bluesky"

    def test_list_sns_argument(self):
        """--list-snsオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--list-sns"])
        assert args.list_sns is True

    def test_list_feeds_argument(self):
        """--list-feedsオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--list-feeds"])
        assert args.list_feeds is True

    def test_feed_argument(self):
        """--feedオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--feed", "blog1,blog2"])
        assert args.feed == "blog1,blog2"

    def test_optimize_argument(self):
        """--optimizeオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--optimize"])
        assert args.optimize is True

    def test_media_argument_single(self):
        """--mediaオプション(単一)が正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--media", "image.jpg"])
        assert args.media == ["image.jpg"]

    def test_media_argument_multiple(self):
        """--mediaオプション(複数)が正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--media", "image1.jpg", "--media", "image2.png"])
        assert args.media == ["image1.jpg", "image2.png"]

    def test_sensitive_argument(self):
        """--sensitiveオプションが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["--sensitive"])
        assert args.sensitive is True

    def test_subcommand_touch_rss_posted(self):
        """touch-rss-postedサブコマンドが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["touch-rss-posted"])
        assert args.command == "touch-rss-posted"
        # ハンドラーが設定されていない場合はfunc属性が存在しない
        # これは正常な動作

    def test_subcommand_touch_rss_posted_with_handler(self):
        """touch-rss-postedサブコマンド + ハンドラーが正しく処理されることを確認"""
        def dummy_handler(args, config_manager):
            pass

        parser = create_argument_parser(touch_rss_posted_handler=dummy_handler)
        args = parser.parse_args(["touch-rss-posted"])
        assert args.command == "touch-rss-posted"
        assert hasattr(args, 'func')
        assert args.func == dummy_handler

    def test_subcommand_touch_rss_posted_with_dry_run(self):
        """touch-rss-postedサブコマンド + --dry-runが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args(["touch-rss-posted", "--dry-run"])
        assert args.command == "touch-rss-posted"
        assert args.dry_run is True

    def test_combined_arguments(self):
        """複数オプション組み合わせが正しく処理されることを確認"""
        parser = create_argument_parser()
        args = parser.parse_args([
            "--config", "test.yml",
            "--debug",
            "--dry-run",
            "--limit", "3",
            "--sns", "x",
            "--optimize"
        ])
        assert args.config == "test.yml"
        assert args.debug is True
        assert args.dry_run is True
        assert args.limit == 3
        assert args.sns == "x"
        assert args.optimize is True
