#!/usr/bin/env python
# -*- coding: utf-8 -*-

import argparse
from .config_manager import ConfigManager, load_config
from .article_manager import ArticleManager
from .plugin_loader import load_plugins

def main():
    parser = argparse.ArgumentParser(description="ブログの更新をチェックし、SNSにポストします。")
    parser.add_argument("--config", type=str, default="config.yml", help="設定ファイルのパス")
    parser.add_argument("--dry-run", action="store_true", help="ドライランを実行します。")
    parser.add_argument("--limit", type=int, help="処理する記事数を制限します。")
    parser.add_argument("--debug", action="store_true", help="デバッグ情報を表示します。")
    args = parser.parse_args()

    config_data = load_config(args.config)
    config_manager = ConfigManager(config_data)
    article_manager = ArticleManager(config_manager)

    if args.debug:
        print(f"フィードURL: {article_manager.feed_url}")

    latest_articles = article_manager.get_latest_articles(args.debug)
    saved_articles = article_manager.load_saved_articles()
    new_articles = article_manager.get_new_articles(latest_articles, saved_articles)

    if new_articles:
        if args.limit:
            new_articles = new_articles[:args.limit]
            print(f"直近の{args.limit}個の記事のみを処理します。")
        
        print("新しい記事が見つかりました:")
        plugins = load_plugins(config_manager) if not args.dry_run else {}

        for article in new_articles:
            if not args.dry_run:
                for plugin_name, plugin_instance in plugins.items():
                    try:
                        # プラグインのtypeを取得（配列形式）またはplugin_name（オブジェクト形式）
                        sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                        optimized_text = article_manager.create_post_text(article['title'], article['link'], sns_type)
                        print(f"{plugin_name}投稿内容: {optimized_text}")
                        plugin_instance.post(optimized_text)
                    except Exception as e:
                        print(f"{plugin_name}への投稿中にエラー: {e}")
            else:
                # ドライラン時は代表的なSNSで投稿内容を表示
                sample_text = article_manager.create_post_text(article['title'], article['link'], 'x')
                print(f"投稿内容例 (X): {sample_text}")
                print(f"[ドライラン] SNSに投稿しました。")
        
        if not args.dry_run:
            article_manager.save_articles(latest_articles)
            print("新しい記事リストを保存しました。")
    else:
        print("新しい記事はありませんでした。")

if __name__ == "__main__":
    main()