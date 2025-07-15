#!/usr/bin/env python
# -*- coding: utf-8 -*-

import argparse

from .config_manager import load_config
from .article_manager import get_latest_articles, load_saved_articles, save_articles, get_new_articles
from .plugin_loader import load_plugins

def main():
    """
    メイン処理
    """
    parser = argparse.ArgumentParser(description="ブログの更新をチェックし、SNSにポストします。")
    parser.add_argument(
        "--config", 
        type=str, 
        default="config.yml", 
        help="設定ファイルのパス (デフォルト: config.yml)"
    )
    parser.add_argument(
        "--dry-run", 
        action="store_true", 
        help="SNSに実際に投稿せず、ドライランを実行します。"
    )
    parser.add_argument(
        "--limit",
        type=int,
        help="直近のN個の記事のみを処理します。"
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="デバッグ情報を表示します。"
    )
    args = parser.parse_args()

    config = load_config(args.config)
    feed_url = config['blog']['feed_url']
    
    if args.debug:
        print(f"フィードURL: {feed_url}")
    
    latest_articles = get_latest_articles(feed_url, args.debug)
    
    if args.debug:
        print(f"フィードから取得した記事数: {len(latest_articles)}")
        if latest_articles:
            print("取得した記事一覧:")
            for i, article in enumerate(latest_articles[:10]):  # 最初の10個を表示
                print(f"  {i+1}. {article['title']}")
                print(f"     URL: {article['link']}")
        else:
            print("フィードから記事を取得できませんでした。")
    
    saved_articles = load_saved_articles()
    
    if args.debug:
        print(f"保存済み記事数: {len(saved_articles)}")
        if saved_articles:
            print("保存済み記事一覧:")
            for i, article in enumerate(saved_articles[:5]):  # 最初の5個を表示
                print(f"  {i+1}. {article['title']}")
    
    new_articles = get_new_articles(latest_articles, saved_articles)
    
    if new_articles:
        if args.limit:
            new_articles = new_articles[:args.limit]
            print(f"直近の{args.limit}個の記事のみを処理します。")
        print("新しい記事が見つかりました:")
        if not args.dry_run:
            plugins = load_plugins(config)
        for article in new_articles:
            print(f"Title: {article['title']}")
            print(f"Link: {article['link']}\n")
            if not args.dry_run:
                for plugin_name, plugin_instance in plugins.items():
                    try:
                        plugin_instance.post(article['title'], article['link'])
                    except Exception as e:
                        print(f"{plugin_name} への投稿中にエラーが発生しました: {e}")
            else:
                print(f"[ドライラン] SNSに投稿: Title: {article['title']}, Link: {article['link']}")
        
        # 新しい記事を保存済みの記事リストに追加し、保存する
        # 最新の記事リストを保存することで、次回実行時に重複投稿を防ぐ
        save_articles(latest_articles)
    else:
        print("新しい記事はありませんでした。")

if __name__ == "__main__":
    main()
