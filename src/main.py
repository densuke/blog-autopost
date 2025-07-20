#!/usr/bin/env python
# -*- coding: utf-8 -*-

import argparse
from .config_manager import ConfigManager, load_config
from .article_manager import ArticleManager
from .plugin_loader import load_plugins


def handle_direct_text_post(args, config_manager):
    """
    直接テキスト投稿を処理します
    
    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    text = args.text
    target_sns = None
    
    # SNS限定オプションの処理
    if args.sns:
        target_sns = [sns.strip() for sns in args.sns.split(',')]
        if args.debug:
            print(f"投稿対象SNS: {target_sns}")
    
    # プラグインを読み込み
    if not args.dry_run:
        all_plugins = load_plugins(config_manager)
        
        # SNS限定がある場合はフィルタリング
        if target_sns:
            plugins = {}
            for plugin_name, plugin_instance in all_plugins.items():
                # プラグイン名またはSNS typeが対象リストに含まれるかチェック
                sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                if plugin_name in target_sns or sns_type in target_sns:
                    plugins[plugin_name] = plugin_instance
            
            if not plugins:
                print(f"指定されたSNS ({args.sns}) が見つかりませんでした。")
                print(f"利用可能なSNS: {', '.join(all_plugins.keys())}")
                return
        else:
            plugins = all_plugins
    else:
        plugins = {}
    
    print(f"投稿テキスト: {text}")
    print(f"文字数: {len(text)}")
    
    # 文字数制限の警告表示
    character_limits = {'x': 280, 'bluesky': 300, 'mastodon': 500, 'misskey': 3000}
    
    # ドライラン時は警告用に仮のプラグイン情報を作成
    if args.dry_run and target_sns:
        all_plugins = load_plugins(config_manager)
        plugins_for_warning = {}
        for plugin_name, plugin_instance in all_plugins.items():
            sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
            if plugin_name in target_sns or sns_type in target_sns:
                plugins_for_warning[plugin_name] = plugin_instance
    elif args.dry_run:
        plugins_for_warning = load_plugins(config_manager)
    else:
        plugins_for_warning = plugins
    
    # 警告表示
    for plugin_name, plugin_instance in plugins_for_warning.items():
        sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
        limit = character_limits.get(sns_type, 500)
        if len(text) > limit:
            print(f"⚠️  警告: {plugin_name} の文字数制限 ({limit}文字) を超えています")
    
    # 投稿実行
    if not args.dry_run:
        print("以下のSNSに投稿しています:")
        for plugin_name, plugin_instance in plugins.items():
            try:
                print(f"- {plugin_name}: 投稿中...")
                plugin_instance.post(text)
                print(f"- {plugin_name}: 投稿完了")
            except Exception as e:
                print(f"- {plugin_name}: 投稿失敗 - {e}")
        print("直接投稿が完了しました。")
    else:
        print("[ドライラン] 以下のSNSに投稿予定:")
        if target_sns:
            print(f"- 投稿対象: {', '.join(target_sns)}")
        else:
            all_plugins = load_plugins(config_manager)
            print(f"- 投稿対象: {', '.join(all_plugins.keys())}")
        print("[ドライラン] 直接投稿をシミュレートしました。")

def main():
    parser = argparse.ArgumentParser(description="ブログの更新をチェックし、SNSにポストします。")
    parser.add_argument("--config", type=str, default="config.yml", help="設定ファイルのパス")
    parser.add_argument("--dry-run", action="store_true", help="ドライランを実行します。")
    parser.add_argument("--limit", type=int, help="処理する記事数を制限します。")
    parser.add_argument("--debug", action="store_true", help="デバッグ情報を表示します。")
    parser.add_argument("--text", type=str, help="指定したテキストを直接SNSに投稿します。")
    parser.add_argument("--sns", type=str, help="投稿するSNSを限定します（カンマ区切りで複数指定可能）。")
    args = parser.parse_args()

    config_data = load_config(args.config)
    config_manager = ConfigManager(config_data)
    
    # 直接テキスト投稿モードかどうかチェック
    if args.text:
        handle_direct_text_post(args, config_manager)
        return
    
    # 通常のRSS監視モード
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