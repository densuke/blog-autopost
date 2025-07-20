#!/usr/bin/env python
# -*- coding: utf-8 -*-

import argparse
from .config_manager import ConfigManager, load_config
from .article_manager import ArticleManager
from .plugin_loader import load_plugins


def handle_list_sns(config_manager):
    """
    登録されているSNSアカウントの一覧を表示します
    
    Args:
        config_manager: 設定管理インスタンス
    """
    print("=== 登録されているSNSアカウント一覧 ===")
    
    sns_configs = config_manager.get_all_sns_configs()
    
    if not sns_configs:
        print("SNSアカウントが設定されていません。")
        print("config.ymlを確認してください。")
        return
    
    # 配列形式の場合
    if isinstance(sns_configs, list):
        print(f"設定形式: 配列形式（複数アカウント対応）")
        print(f"登録アカウント数: {len(sns_configs)}")
        print()
        
        for i, sns_config in enumerate(sns_configs, 1):
            sns_type = sns_config.get('type', 'unknown')
            name = sns_config.get('name', f'{sns_type}-{i}')
            
            print(f"{i}. {name}")
            print(f"   SNS種別: {sns_type}")
            
            # SNS別の詳細情報
            if sns_type == 'x':
                has_credentials = all(key in sns_config for key in ['consumer_key', 'consumer_secret', 'access_token', 'access_token_secret'])
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            elif sns_type == 'bluesky':
                has_credentials = all(key in sns_config for key in ['identifier', 'password'])
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            elif sns_type in ['mastodon', 'misskey']:
                has_credentials = all(key in sns_config for key in ['instance_url', 'access_token'])
                instance_url = sns_config.get('instance_url', 'N/A')
                print(f"   インスタンス: {instance_url}")
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            print()
    
    # オブジェクト形式の場合（後方互換性）
    elif isinstance(sns_configs, dict):
        print(f"設定形式: オブジェクト形式（従来形式）")
        print(f"登録SNS数: {len(sns_configs)}")
        print()
        
        for i, (sns_name, sns_config) in enumerate(sns_configs.items(), 1):
            print(f"{i}. {sns_name}")
            print(f"   SNS種別: {sns_name}")
            
            # SNS別の詳細情報
            if sns_name == 'x':
                has_credentials = all(key in sns_config for key in ['consumer_key', 'consumer_secret', 'access_token', 'access_token_secret'])
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            elif sns_name == 'bluesky':
                has_credentials = all(key in sns_config for key in ['identifier', 'password'])
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            elif sns_name in ['mastodon', 'misskey']:
                has_credentials = all(key in sns_config for key in ['instance_url', 'access_token'])
                instance_url = sns_config.get('instance_url', 'N/A')
                print(f"   インスタンス: {instance_url}")
                print(f"   認証情報: {'設定済み' if has_credentials else '不完全'}")
            print()
    
    print("注意: --sns オプションでは上記の名前またはSNS種別を指定できます。")


def handle_direct_text_post(args, config_manager):
    """
    直接テキスト投稿を処理します
    
    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    original_text = args.text
    target_sns = None
    media_files = args.media or []
    
    # メディア添付機能のバリデーション
    if media_files:
        from .media_validator import validate_media_for_posting, ValidationError
        from .media_converter import create_media_converter, ConversionError, is_ffmpeg_available
        
        print(f"添付メディア: {len(media_files)}件")
        for i, media_path in enumerate(media_files, 1):
            print(f"  {i}. {media_path}")
    
    # --optimizeオプションが指定された場合はTextOptimizerを使用
    if args.optimize:
        from .text_optimizer import TextOptimizer
        text_optimizer = TextOptimizer(config_manager.config)
    else:
        text_optimizer = None
    
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
    
    # メディアファイルの事前検証
    if media_files:
        try:
            # 対象SNSのリストを作成
            if plugins:
                target_sns_types = []
                for plugin_name, plugin_instance in plugins.items():
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    if sns_type not in target_sns_types:
                        target_sns_types.append(sns_type)
            else:
                # ドライラン時は全SNSをチェック
                target_sns_types = ['x', 'bluesky', 'mastodon', 'misskey']
            
            # メディア検証実行
            validation_results = validate_media_for_posting(media_files, target_sns_types)
            
            # 検証結果の表示
            has_errors = False
            converted_files = {}
            
            for sns_type, result in validation_results.items():
                if result.errors:
                    has_errors = True
                    print(f"❌ {sns_type.upper()}: {', '.join(result.errors)}")
                elif result.warnings:
                    print(f"⚠️  {sns_type.upper()}: {', '.join(result.warnings)}")
                else:
                    print(f"✅ {sns_type.upper()}: 投稿可能")
                
                # 変換情報を収集
                converted_files.update(result.converted_files)
            
            # エラーがある場合は処理を停止
            if has_errors and not args.dry_run:
                print("\n投稿を中止しました。上記のエラーを解決してから再実行してください。")
                return
            
            # 音声ファイルの変換処理（X向け）
            if any('x' in validation_results and 
                   any('MP4に変換されます' in warning for warning in validation_results['x'].warnings) 
                   for _ in [None]):  # 条件チェック用のダミーループ
                
                if not is_ffmpeg_available():
                    print("❌ ffmpegが見つかりません。音声変換にはffmpegが必要です。")
                    if not args.dry_run:
                        return
                else:
                    print("🔄 音声ファイルをMP4に変換しています...")
                    converter = create_media_converter()
                    
                    # 音声ファイルのみを変換
                    for i, media_path in enumerate(media_files):
                        if media_path.lower().endswith('.m4a'):
                            try:
                                if not args.dry_run:
                                    converted_path = converter.convert_m4a_to_mp4(media_path)
                                    media_files[i] = converted_path
                                    print(f"✅ 変換完了: {media_path} → {converted_path}")
                                else:
                                    print(f"[ドライラン] 変換予定: {media_path}")
                            except ConversionError as e:
                                print(f"❌ 変換失敗: {media_path} - {e}")
                                if not args.dry_run:
                                    return
        
        except ValidationError as e:
            print(f"❌ メディア検証エラー: {e}")
            return
        except Exception as e:
            print(f"❌ メディア処理中にエラーが発生しました: {e}")
            if args.debug:
                import traceback
                traceback.print_exc()
            return
    
    print(f"投稿テキスト: {original_text}")
    print(f"文字数: {len(original_text)}")
    
    if args.optimize:
        print("テキスト最適化が有効です。")
    
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
    
    # 警告表示（最適化なしの場合のみ）
    if not args.optimize:
        for plugin_name, plugin_instance in plugins_for_warning.items():
            sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
            limit = character_limits.get(sns_type, 500)
            if len(original_text) > limit:
                print(f"⚠️  警告: {plugin_name} の文字数制限 ({limit}文字) を超えています")
    
    # 投稿実行
    if not args.dry_run:
        print("以下のSNSに投稿しています:")
        for plugin_name, plugin_instance in plugins.items():
            try:
                print(f"- {plugin_name}: 投稿中...")
                
                # 最適化が有効な場合はSNS別に最適化されたテキストを使用
                if args.optimize and text_optimizer:
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    # URLが含まれている場合のみ最適化を適用（タイトルとして空文字、リンクとして全文を扱う）
                    import re
                    url_pattern = r'https?://[^\s]+'
                    urls = re.findall(url_pattern, original_text)
                    
                    if urls:
                        # URLを含む場合：URL以外の部分をタイトルとして扱う
                        url = urls[-1]  # 最後のURLを使用
                        title_part = original_text.replace(url, '').strip()
                        optimized_text = text_optimizer.optimize_text(title_part, url, sns_type, force_optimize=True)
                    else:
                        # URLを含まない場合：そのまま投稿
                        optimized_text = original_text
                    
                    print(f"  最適化後: {optimized_text} ({len(optimized_text)}文字)")
                    plugin_instance.post(optimized_text, media_files if media_files else None)
                else:
                    plugin_instance.post(original_text, media_files if media_files else None)
                
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
    parser.add_argument("--list-sns", action="store_true", help="登録されているSNSアカウントの一覧を表示します。")
    parser.add_argument("--optimize", action="store_true", help="直接投稿時にもテキスト最適化（URL短縮など）を適用します。")
    parser.add_argument("--media", action="append", help="投稿にメディアファイルを添付します（複数回指定可能）。")
    args = parser.parse_args()

    config_data = load_config(args.config)
    config_manager = ConfigManager(config_data)
    
    # SNS一覧表示モードかどうかチェック
    if args.list_sns:
        handle_list_sns(config_manager)
        return
    
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