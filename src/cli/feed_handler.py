#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
フィード処理ハンドラー

RSS/Atomフィードの一覧表示・監視・処理を担当
"""

from typing import Any, Dict, List

from ..article_manager import ArticleManager, MultiArticleManager
from ..config_manager import ConfigManager
from ..plugin_loader import load_plugins


def handle_list_feeds(config_manager: ConfigManager) -> None:
    """
    登録されているフィードの一覧を表示します

    Args:
        config_manager: 設定管理インスタンス
    """
    print("=== 登録されているフィード一覧 ===")

    feed_configs = config_manager.get_all_feed_configs()

    if not feed_configs:
        print("フィードが設定されていません。")
        print("config.ymlを確認してください。")
        return

    print(f"登録フィード数: {len(feed_configs)}")
    print()

    for i, feed_config in enumerate(feed_configs, 1):
        name = feed_config.get('name', f'フィード{i}')
        feed_url = feed_config.get('feed_url', 'N/A')

        print(f"{i}. {name}")
        print(f"   フィードURL: {feed_url}")

        # 画像設定の確認
        image_settings = feed_config.get('image_settings')
        if image_settings:
            enable_link_cards = image_settings.get('enable_link_cards', False)
            print(f"   リンクカード機能: {'有効' if enable_link_cards else '無効'}")
        print()

    print("注意: --feed オプションでは上記の名前を指定できます。")


def handle_touch_rss_posted(args, config_manager: ConfigManager) -> None:
    """
    登録されたRSSフィードのアイテムをすべて投稿済みとしてマークします

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    if args.dry_run:
        print("[ドライラン] RSSフィードのアイテムをすべて投稿済みとしてマークします。")
        return

    print("RSSフィードのアイテムをすべて投稿済みとしてマークします。")
    article_manager = ArticleManager(config_manager)
    result = article_manager.force_mark_all_as_posted()
    if result.get('status') == 'success':
        print(f"処理が完了しました。処理された記事数: {result.get('processed_count', 0)}")
    else:
        print(f"エラーが発生しました: {result.get('message', '不明なエラー')}")


def process_rss_feeds(args, config_manager: ConfigManager) -> None:
    """
    RSS/Atomフィードを監視し、新着記事を処理します

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    # 複数フィード対応の確認
    feed_configs = config_manager.get_all_feed_configs()

    if len(feed_configs) > 1 or (len(feed_configs) == 1 and feed_configs[0].get('name') != 'default'):
        # 複数フィード処理
        _process_multi_feeds(args, config_manager, feed_configs)
    else:
        # 単一フィード処理（従来通り）
        _process_single_feed(args, config_manager)


def _process_multi_feeds(args, config_manager: ConfigManager, feed_configs: List[Dict[str, Any]]) -> None:
    """
    複数フィードの処理を行います

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
        feed_configs: フィード設定リスト
    """
    multi_article_manager = MultiArticleManager(config_manager)

    # フィード限定オプションの処理
    feed_filter = None
    if args.feed:
        feed_filter = [feed.strip() for feed in args.feed.split(',')]
        if args.debug:
            print(f"処理対象フィード: {feed_filter}")

    # 全フィードから新着記事を取得
    all_new_articles_data = multi_article_manager.get_all_new_articles(
        debug=args.debug, limit=args.limit, feed_filter=feed_filter
    )

    if all_new_articles_data:
        if args.limit:
            print(f"各フィードから直近の{args.limit}個の記事のみを処理します。")

        print("新しい記事が見つかりました:")

        # プラグインを読み込み
        if not args.dry_run:
            all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None, dry_run=args.dry_run)

            # SNS限定がある場合はフィルタリング
            if args.sns:
                target_sns = [sns.strip() for sns in args.sns.split(',')]
                plugins = {}
                for plugin_name, plugin_instance in all_plugins.items():
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    if plugin_name in target_sns or sns_type in target_sns:
                        plugins[plugin_name] = plugin_instance

                if not plugins:
                    print(f"指定されたSNS ({args.sns}) が見つかりませんでした。")
                    print(f"利用可能なSNS: {', '.join(all_plugins.keys())}")
                    return
                else:
                    if args.debug:
                        print(f"投稿対象SNS: {', '.join(plugins.keys())}")
            else:
                plugins = all_plugins
        else:
            plugins = {}

        # フィード別に記事を処理
        for feed_name, data in all_new_articles_data.items():
            articles = data['articles']

            if args.debug:
                print(f"\n--- フィード: {feed_name} ({len(articles)}件) ---")

            # フィード別のArticleManagerを作成（投稿テキスト生成用）
            feed_article_manager = ArticleManager(config_manager, feed_name)

            for article in articles:
                if not args.dry_run:
                    for plugin_name, plugin_instance in plugins.items():
                        try:
                            sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                            optimized_text = feed_article_manager.create_post_text(article['title'], article['link'], sns_type)
                            if args.debug:
                                print(f"{plugin_name}投稿内容: {optimized_text}")

                            # リッチコンテンツをサポートするSNSの場合は記事データも渡す
                            if hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                                if args.debug:
                                    print(f"[DEBUG] {sns_type}投稿: リンクカード機能対応")
                                plugin_instance.post(optimized_text, article_data=article, debug=args.debug)
                            else:
                                plugin_instance.post(optimized_text, debug=args.debug)
                        except Exception as e:
                            print(f"{plugin_name}への投稿中にエラー: {e}")
                else:
                    # ドライラン時は代表的なSNSで投稿内容を表示
                    sample_text = feed_article_manager.create_post_text(article['title'], article['link'], 'x')
                    print(f"投稿内容例 (X): {sample_text}")
                    print("[ドライラン] SNSに投稿しました。")

        if not args.dry_run:
            multi_article_manager.save_all_articles(all_new_articles_data)
            print("新しい記事リストを保存しました。")
    else:
        print("新しい記事はありませんでした。")


def _process_single_feed(args, config_manager: ConfigManager) -> None:
    """
    単一フィードの処理を行います（従来通り）

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    article_manager = ArticleManager(config_manager)

    if args.debug:
        print(f"フィードURL: {article_manager.feed_url}")

    latest_articles = article_manager.get_latest_articles(args.debug)
    saved_articles = article_manager.load_saved_articles()
    new_articles = article_manager.get_new_articles(latest_articles, saved_articles, args.debug, args.limit)

    if new_articles:
        if args.limit:
            print(f"直近の{args.limit}個の記事のみを処理します。")

        print("新しい記事が見つかりました:")

        # プラグインを読み込み
        if not args.dry_run:
            all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None, dry_run=args.dry_run)

            # SNS限定がある場合はフィルタリング
            if args.sns:
                target_sns = [sns.strip() for sns in args.sns.split(',')]
                plugins = {}
                for plugin_name, plugin_instance in all_plugins.items():
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    if plugin_name in target_sns or sns_type in target_sns:
                        plugins[plugin_name] = plugin_instance

                if not plugins:
                    print(f"指定されたSNS ({args.sns}) が見つかりませんでした。")
                    print(f"利用可能なSNS: {', '.join(all_plugins.keys())}")
                    return
                else:
                    if args.debug:
                        print(f"投稿対象SNS: {', '.join(plugins.keys())}")
            else:
                plugins = all_plugins
        else:
            plugins = {}

        for article in new_articles:
            if not args.dry_run:
                for plugin_name, plugin_instance in plugins.items():
                    try:
                        sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                        optimized_text = article_manager.create_post_text(article['title'], article['link'], sns_type)
                        if args.debug:
                            print(f"{plugin_name}投稿内容: {optimized_text}")

                        # リッチコンテンツをサポートするSNSの場合は記事データも渡す
                        if hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                            if args.debug:
                                print(f"[DEBUG] {sns_type}投稿: リンクカード機能対応")
                            plugin_instance.post(optimized_text, article_data=article, debug=args.debug)
                        else:
                            plugin_instance.post(optimized_text, debug=args.debug)
                    except Exception as e:
                        print(f"{plugin_name}への投稿中にエラー: {e}")
            else:
                # ドライラン時は代表的なSNSで投稿内容を表示
                sample_text = article_manager.create_post_text(article['title'], article['link'], 'x')
                print(f"投稿内容例 (X): {sample_text}")
                print("[ドライラン] SNSに投稿しました。")

        if not args.dry_run:
            article_manager.save_articles(latest_articles)
            print("新しい記事リストを保存しました。")
    else:
        print("新しい記事はありませんでした。")
