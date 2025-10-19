#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
投稿処理ハンドラー

SNS投稿・テキスト最適化・プラグイン管理などの投稿関連処理を担当
"""

import re
from typing import Any, Dict, List, Optional, Tuple

from ..config_manager import ConfigManager
from ..plugin_loader import load_plugins
from .media_handler import extract_image_from_url, process_media_files


def handle_list_sns(config_manager: ConfigManager) -> None:
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
        print("設定形式: 配列形式（複数アカウント対応）")
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
        print("設定形式: オブジェクト形式（従来形式）")
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


def setup_text_optimization(args, config_manager: ConfigManager) -> Optional[Any]:
    """
    テキスト最適化の設定を行います

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス

    Returns:
        TextOptimizer or None: テキスト最適化オブジェクト
    """
    if args.optimize:
        from ..text_optimizer import TextOptimizer
        return TextOptimizer(config_manager.config)
    return None


def validate_and_filter_plugins(args, config_manager: ConfigManager) -> Optional[Tuple[Dict[str, Any], Optional[List[str]]]]:
    """
    プラグインの読み込み、フィルタリング、バリデーションを行います

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス

    Returns:
        tuple: (plugins_dict, target_sns_list) または None（エラー時）
    """
    target_sns = None

    # SNS限定オプションの処理
    if args.sns:
        target_sns = [sns.strip() for sns in args.sns.split(',')]
        if args.debug:
            print(f"投稿対象SNS: {target_sns}")

    # プラグインを読み込み
    if not args.dry_run:
        all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None, dry_run=args.dry_run)

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
                return None
        else:
            plugins = all_plugins
    else:
        plugins = {}

    return plugins, target_sns


def execute_sns_posting(original_text: str, media_files: Optional[List[str]],
                        plugins: Dict[str, Any], target_sns: Optional[List[str]],
                        text_optimizer: Optional[Any], args, config_manager: ConfigManager) -> None:
    """
    SNS投稿処理を実行します

    Args:
        original_text: 元の投稿テキスト
        media_files: 添付メディアファイル
        plugins: SNSプラグイン辞書
        target_sns: 対象SNSリスト
        text_optimizer: テキスト最適化オブジェクト
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    # 文字数制限の警告表示
    character_limits = config_manager.config.get('character_limits', {
        'x': 280,
        'bluesky': 300,
        'mastodon': 500,
        'misskey': 3000
    })

    # ドライラン時は警告用に仮のプラグイン情報を作成
    if args.dry_run and target_sns:
        all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None, dry_run=args.dry_run)
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
                url_pattern = r'https?://[^\s]+'
                urls = re.findall(url_pattern, original_text)

                # リンクカード対応プラグインのために簡易article_dataを作成
                if urls and hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                    url = urls[-1]  # 最後のURLを使用
                    title_part = original_text.replace(url, '').strip()

                    # URLから画像を取得
                    image_url = extract_image_from_url(url, debug=args.debug)

                # 最適化が有効な場合はSNS別に最適化されたテキストを使用
                optimized_text_to_post = original_text
                if args.optimize and text_optimizer:
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    if urls:
                        url = urls[-1]
                        title_part = original_text.replace(url, '').strip()
                        optimized_text_to_post = text_optimizer.optimize_text(title_part, url, sns_type, force_optimize=True)
                    else:
                        optimized_text_to_post = original_text # URLを含まない場合はそのまま

                    if args.debug:
                        print(f"  最適化後: {optimized_text_to_post} ({len(optimized_text_to_post)}文字)")

                # リンクカード対応プラグインのために簡易article_dataを作成
                article_data_to_post = None
                if urls and hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                    url = urls[-1]
                    title_part = original_text.replace(url, '').strip()
                    image_url = extract_image_from_url(url, debug=args.debug)
                    article_data_to_post = {
                        'title': title_part if title_part else 'ブログ記事',
                        'link': url,
                        'description': title_part,
                        'image': image_url if image_url else None
                    }

                # 投稿実行
                plugin_instance.post(optimized_text_to_post, media_files if media_files else None, article_data=article_data_to_post, debug=args.debug)

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
            all_plugins = load_plugins(config_manager, force_sensitive=args.sensitive if hasattr(args, 'sensitive') else None, dry_run=args.dry_run)
            print(f"- 投稿対象: {', '.join(all_plugins.keys())}")
        print("[ドライラン] 直接投稿をシミュレートしました。")


def handle_direct_text_post(args, config_manager: ConfigManager) -> None:
    """
    直接テキスト投稿を処理します

    Args:
        args: コマンドライン引数
        config_manager: 設定管理インスタンス
    """
    original_text = args.text
    target_sns = None
    media_files = args.media or []

    # メディアファイルの処理
    processed_media_files = process_media_files(media_files, args)
    if processed_media_files is None:  # エラーが発生した場合
        return
    media_files = processed_media_files

    # テキスト最適化の設定
    text_optimizer = setup_text_optimization(args, config_manager)

    # プラグインのバリデーション・フィルタリング
    plugin_result = validate_and_filter_plugins(args, config_manager)
    if plugin_result is None:  # エラーが発生した場合
        return
    plugins, target_sns = plugin_result


    if args.debug:
        print(f"投稿テキスト: {original_text}")
        print(f"文字数: {len(original_text)}")

    if args.optimize and args.debug:
        print("テキスト最適化が有効です。")

    # SNS投稿処理の実行
    execute_sns_posting(original_text, media_files, plugins, target_sns, text_optimizer, args, config_manager)
