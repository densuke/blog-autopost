#!/usr/bin/env python
# -*- coding: utf-8 -*-

import argparse
import os
import requests
from bs4 import BeautifulSoup
from urllib.parse import urljoin
from .config_manager import ConfigManager, load_config
from .article_manager import ArticleManager, MultiArticleManager
from .plugin_loader import load_plugins


def extract_image_from_url(url: str, debug: bool = False) -> str:
    """
    URLから画像を抽出します（OGP画像、Twitter Card画像など）
    
    Args:
        url: 記事URL
        debug: デバッグモード
        
    Returns:
        str: 画像URL（見つからない場合は空文字）
    """
    try:
        if debug:
            print(f"[DEBUG] 画像取得開始: {url}")
            
        response = requests.get(url, timeout=10, headers={
            'User-Agent': 'Mozilla/5.0 (compatible; Blog-AutoPost/1.0)'
        })
        response.raise_for_status()
        
        # 文字コードの自動検出（bluesky.pyと同様の処理）
        html_content = _decode_html_content(response)
        soup = BeautifulSoup(html_content, 'html.parser')
        
        # 画像取得の優先順位
        image_selectors = [
            # OGP画像
            ('meta', {'property': 'og:image'}),
            ('meta', {'property': 'og:image:url'}),
            # Twitter Card画像
            ('meta', {'name': 'twitter:image'}),
            ('meta', {'name': 'twitter:image:src'}),
            # 他のメタタグ
            ('meta', {'name': 'image'}),
            ('meta', {'itemprop': 'image'}),
        ]
        
        for selector_type, attrs in image_selectors:
            element = soup.find(selector_type, attrs)
            if element and element.get('content'):
                image_url = element['content'].strip()
                if image_url:
                    # 相対URLを絶対URLに変換
                    absolute_url = urljoin(url, image_url)
                    if debug:
                        print(f"[DEBUG] 画像発見: {absolute_url} (ソース: {attrs})")
                    return absolute_url
        
        if debug:
            print(f"[DEBUG] 画像が見つかりませんでした")
        return ''
        
    except Exception as e:
        if debug:
            print(f"[DEBUG] 画像取得エラー: {e}")
        return ''


def _decode_html_content(response) -> str:
    """
    HTMLコンテンツを適切な文字コードでデコードします
    （bluesky.pyと同じ処理）
    """
    import re
    
    # Content-Typeヘッダーからcharsetを取得
    content_type = response.headers.get('content-type', '').lower()
    
    # HTMLのmeta charsetタグから文字コードを検出
    html_bytes = response.content
    html_preview = html_bytes[:2048].decode('utf-8', errors='ignore').lower()
    
    # meta charset検出のパターン
    charset_patterns = [
        r'<meta[^>]+charset=["\']?([^"\'>\s]+)',
        r'<meta[^>]+content=["\'][^"\']*charset=([^"\'>\s]+)',
    ]
    
    detected_charset = None
    for pattern in charset_patterns:
        match = re.search(pattern, html_preview)
        if match:
            detected_charset = match.group(1).strip()
            break
    
    # 文字コード優先順位: meta charset > Content-Type > 自動検出
    encodings_to_try = []
    
    if detected_charset:
        encodings_to_try.append(detected_charset)
    
    if 'charset=' in content_type:
        charset_from_header = content_type.split('charset=')[1].split(';')[0].strip()
        if charset_from_header not in encodings_to_try:
            encodings_to_try.append(charset_from_header)
    
    # 日本語サイトでよく使われる文字コードを追加
    common_encodings = ['utf-8', 'shift_jis', 'euc-jp', 'iso-2022-jp', 'cp932']
    for encoding in common_encodings:
        if encoding not in encodings_to_try:
            encodings_to_try.append(encoding)
    
    # 各文字コードを順番に試行
    for encoding in encodings_to_try:
        try:
            return html_bytes.decode(encoding)
        except (UnicodeDecodeError, LookupError):
            continue
    
    # すべて失敗した場合はUTF-8でエラーを無視してデコード
    return html_bytes.decode('utf-8', errors='ignore')


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


def handle_list_feeds(config_manager):
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
    resized_media_files = []
    if media_files:
        from .media_validator import validate_media_for_posting, ValidationError
        from .media_converter import create_media_converter, ConversionError, is_ffmpeg_available
        from .image_resizer import create_image_resizer
        
        if args.debug:
            print(f"添付メディア: {len(media_files)}件")
            for i, media_path in enumerate(media_files, 1):
                print(f"  {i}. {media_path}")
        
        # 画像リサイズの前処理
        if args.debug:
            print("画像リサイズ処理を実行中...")
        resizer = create_image_resizer(debug=args.debug)
        
        for media_path in media_files:
            try:
                # 画像ファイルかどうかチェック
                if media_path.lower().endswith(('.jpg', '.jpeg', '.png', '.gif', '.webp')):
                    # SNS別の制限を考慮（複数SNSがある場合は最小値を使用）
                    if args.sns:
                        target_sns_list = [sns.strip() for sns in args.sns.split(',')]
                        # 最初のSNSの制限を使用（複数対応は後で改良可能）
                        sns_type = target_sns_list[0] if target_sns_list else 'bluesky'
                    else:
                        sns_type = 'bluesky'  # デフォルト
                    
                    # 画像をリサイズ
                    resized_path = resizer.resize_image_file(media_path, sns_type)
                    resized_media_files.append(resized_path)
                    
                    if args.debug:
                        original_size = os.path.getsize(media_path)
                        resized_size = os.path.getsize(resized_path)
                        print(f"[DEBUG] リサイズ: {media_path} ({original_size} bytes) → {resized_path} ({resized_size} bytes)")
                else:
                    # 画像以外はそのまま
                    resized_media_files.append(media_path)
            except Exception as e:
                if args.debug:
                    print(f"画像リサイズエラー: {media_path} - {e}")
                resized_media_files.append(media_path)  # エラー時は元ファイルを使用
        
        # リサイズ後のファイルでバリデーション実行
        media_files = resized_media_files
    
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
        all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
        
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
                elif result.warnings and args.debug:
                    print(f"⚠️  {sns_type.upper()}: {', '.join(result.warnings)}")
                elif args.debug:
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
                    if args.debug:
                        print("🔄 音声ファイルをMP4に変換しています...")
                    converter = create_media_converter()
                    
                    # 音声ファイルのみを変換
                    for i, media_path in enumerate(media_files):
                        if media_path.lower().endswith('.m4a'):
                            try:
                                if not args.dry_run:
                                    converted_path = converter.convert_m4a_to_mp4(media_path)
                                    media_files[i] = converted_path
                                    if args.debug:
                                        print(f"✅ 変換完了: {media_path} → {converted_path}")
                                else:
                                    if args.debug:
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
    
    if args.debug:
        print(f"投稿テキスト: {original_text}")
        print(f"文字数: {len(original_text)}")
    
    if args.optimize and args.debug:
        print("テキスト最適化が有効です。")
    
    # 文字数制限の警告表示
    character_limits = {'x': 280, 'bluesky': 300, 'mastodon': 500, 'misskey': 3000}
    
    # ドライラン時は警告用に仮のプラグイン情報を作成
    if args.dry_run and target_sns:
        all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
        plugins_for_warning = {}
        for plugin_name, plugin_instance in all_plugins.items():
            sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
            if plugin_name in target_sns or sns_type in target_sns:
                plugins_for_warning[plugin_name] = plugin_instance
    elif args.dry_run:
        plugins_for_warning = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
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
        if args.debug:
            print("以下のSNSに投稿しています:")
        for plugin_name, plugin_instance in plugins.items():
            try:
                if args.debug:
                    print(f"- {plugin_name}: 投稿中...")
                
                # URLを抽出（リンクカード対応用）
                import re
                url_pattern = r'https?://[^\s]+'
                urls = re.findall(url_pattern, original_text)
                
                # リンクカード対応プラグインのために簡易article_dataを作成
                article_data = None
                if urls and hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                    url = urls[-1]  # 最後のURLを使用
                    title_part = original_text.replace(url, '').strip()
                    
                    # URLから画像を取得
                    image_url = extract_image_from_url(url, debug=args.debug)
                    
                    article_data = {
                        'title': title_part if title_part else 'ブログ記事', 
                        'link': url,
                        'description': title_part,
                        'image': image_url if image_url else None
                    }
                
                # 最適化が有効な場合はSNS別に最適化されたテキストを使用
                if args.optimize and text_optimizer:
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    # URLが含まれている場合のみ最適化を適用（タイトルとして空文字、リンクとして全文を扱う）
                    
                    if urls:
                        # URLを含む場合：URL以外の部分をタイトルとして扱う
                        url = urls[-1]  # 最後のURLを使用
                        title_part = original_text.replace(url, '').strip()
                        optimized_text = text_optimizer.optimize_text(title_part, url, sns_type, force_optimize=True)
                    else:
                        # URLを含まない場合：そのまま投稿
                        optimized_text = original_text
                    
                    if args.debug:
                        print(f"  最適化後: {optimized_text} ({len(optimized_text)}文字)")
                    
                    # リンクカード対応プラグインの場合はarticle_dataを渡す
                    if article_data:
                        plugin_instance.post(optimized_text, media_files if media_files else None, article_data=article_data, debug=args.debug)
                    else:
                        plugin_instance.post(optimized_text, media_files if media_files else None, debug=args.debug)
                else:
                    # リンクカード対応プラグインの場合はarticle_dataを渡す
                    if article_data:
                        plugin_instance.post(original_text, media_files if media_files else None, article_data=article_data, debug=args.debug)
                    else:
                        plugin_instance.post(original_text, media_files if media_files else None, debug=args.debug)
                
                if args.debug:
                    print(f"- {plugin_name}: 投稿完了")
            except Exception as e:
                print(f"- {plugin_name}: 投稿失敗 - {e}")
        print("直接投稿が完了しました。")
    else:
        print("[ドライラン] 以下のSNSに投稿予定:")
        if target_sns:
            print(f"- 投稿対象: {', '.join(target_sns)}")
        else:
            all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
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
    parser.add_argument("--list-feeds", action="store_true", help="登録されているフィードの一覧を表示します。")
    parser.add_argument("--feed", type=str, help="処理するフィードを限定します（カンマ区切りで複数指定可能）。")
    parser.add_argument("--optimize", action="store_true", help="直接投稿時にもテキスト最適化（URL短縮など）を適用します。")
    parser.add_argument("--media", action="append", help="投稿にメディアファイルを添付します（複数回指定可能）。")
    parser.add_argument("--sensitive", action="store_true", help="Misskeyでメディアファイルをセンシティブコンテンツとしてマークします。")
    args = parser.parse_args()

    config_data = load_config(args.config)
    config_manager = ConfigManager(config_data)
    
    # SNS一覧表示モードかどうかチェック
    if args.list_sns:
        handle_list_sns(config_manager)
        return
    
    # フィード一覧表示モードかどうかチェック
    if args.list_feeds:
        handle_list_feeds(config_manager)
        return
    
    # 直接テキスト投稿モードかどうかチェック
    if args.text:
        handle_direct_text_post(args, config_manager)
        return
    
    # 通常のRSS監視モード
    # 複数フィード対応の確認
    feed_configs = config_manager.get_all_feed_configs()
    
    if len(feed_configs) > 1 or (len(feed_configs) == 1 and feed_configs[0].get('name') != 'default'):
        # 複数フィード処理
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
                all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
                
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
                feed_config = data['feed_config']
                
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
                        print(f"[ドライラン] SNSに投稿しました。")
            
            if not args.dry_run:
                multi_article_manager.save_all_articles(all_new_articles_data)
                print("新しい記事リストを保存しました。")
        else:
            print("新しい記事はありませんでした。")
    else:
        # 単一フィード処理（従来通り）
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
                all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None)
                
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
                    print(f"[ドライラン] SNSに投稿しました。")
            
            if not args.dry_run:
                article_manager.save_articles(latest_articles)
                print("新しい記事リストを保存しました。")
        else:
            print("新しい記事はありませんでした。")

if __name__ == "__main__":
    main()