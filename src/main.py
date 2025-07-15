#!/usr/bin/env python
# -*- coding: utf-8 -*-

import feedparser
import yaml
import json
import os
import importlib
import argparse

DATA_DIR = "data"
ARTICLES_FILE = os.path.join(DATA_DIR, "articles.json")
PLUGINS_DIR = "src.plugins"

def load_config(config_path="config.yml"):
    """
    設定ファイルを読み込みます。

    Args:
        config_path (str): 設定ファイルのパス。

    Returns:
        dict: 設定内容。
    """
    with open(config_path, 'r', encoding='utf-8') as f:
        return yaml.safe_load(f)

def get_latest_articles(feed_url):
    """
    指定されたRSS/Atomフィードから最新の記事リストを取得します。

    Args:
        feed_url (str): RSS/AtomフィードのURL。

    Returns:
        list: 記事のリスト。各記事はタイトルとリンクを持つ辞書です。
    """
    feed = feedparser.parse(feed_url)
    articles = []
    for entry in feed.entries:
        articles.append({
            'title': entry.title,
            'link': entry.link
        })
    return articles

def load_saved_articles():
    """
    保存された記事リストを読み込みます。

    Returns:
        list: 保存された記事のリスト。
    """
    if os.path.exists(ARTICLES_FILE):
        with open(ARTICLES_FILE, 'r', encoding='utf-8') as f:
            return json.load(f)
    return []

def save_articles(articles):
    """
    記事リストを保存します。

    Args:
        articles (list): 保存する記事のリスト。
    """
    with open(ARTICLES_FILE, 'w', encoding='utf-8') as f:
        json.dump(articles, f, ensure_ascii=False, indent=4)

def get_new_articles(latest_articles, saved_articles):
    """
    新しい記事を検出します。

    Args:
        latest_articles (list): 最新の記事リスト。
        saved_articles (list): 保存された記事リスト。

    Returns:
        list: 新しい記事のリスト。
    """
    new_articles = []
    saved_links = {article['link'] for article in saved_articles}
    for article in latest_articles:
        if article['link'] not in saved_links:
            new_articles.append(article)
    return new_articles

def load_plugins(config):
    """
    SNS投稿プラグインを読み込みます。

    Args:
        config (dict): 設定内容。

    Returns:
        dict: 読み込まれたプラグインのインスタンス。
    """
    plugins = {}
    for plugin_name, plugin_config in config.get('sns', {}).items():
        try:
            module = importlib.import_module(f"{PLUGINS_DIR}.{plugin_name}")
            # クラス名をプラグイン名から推測 (例: x -> X)
            class_name = plugin_name.capitalize()
            plugin_class = getattr(module, class_name)
            plugins[plugin_name] = plugin_class(**plugin_config)
        except Exception as e:
            print(f"プラグイン {plugin_name} の読み込み中にエラーが発生しました: {e}")
    return plugins

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
    args = parser.parse_args()

    config = load_config(args.config)
    feed_url = config['blog']['feed_url']
    
    latest_articles = get_latest_articles(feed_url)
    saved_articles = load_saved_articles()
    
    new_articles = get_new_articles(latest_articles, saved_articles)
    
    if new_articles:
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
