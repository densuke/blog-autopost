#!/usr/bin/env python
# -*- coding: utf-8 -*-

"""
メディア処理ハンドラー

画像抽出・リサイズ・変換などのメディア関連処理を担当
"""

import os
from typing import List, Optional
from urllib.parse import urljoin

import requests
from bs4 import BeautifulSoup

from ..utils.html_utils import decode_html_content


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

        # 文字コードの自動検出
        html_content = decode_html_content(response)
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
            element = soup.find(selector_type, attrs)  # type: ignore
            if element and element.get('content'):  # type: ignore
                image_url = element['content'].strip()  # type: ignore
                if image_url:
                    # 相対URLを絶対URLに変換
                    absolute_url = urljoin(url, image_url)
                    if debug:
                        print(f"[DEBUG] 画像発見: {absolute_url} (ソース: {attrs})")
                    return absolute_url

        if debug:
            print("[DEBUG] 画像が見つかりませんでした")
        return ''

    except Exception as e:
        if debug:
            print(f"[DEBUG] 画像取得エラー: {e}")
        return ''


def process_media_files(media_files: List[str], args) -> Optional[List[str]]:
    """
    メディアファイルの処理（リサイズ、バリデーション、変換）を行います

    Args:
        media_files: 処理するメディアファイルのリスト
        args: コマンドライン引数

    Returns:
        list: 処理済みメディアファイルのリスト、エラー時はNone
    """
    if not media_files:
        return []

    from ..image_resizer import create_image_resizer
    from ..media_converter import (
        ConversionError,
        create_media_converter,
        is_ffmpeg_available,
    )
    from ..media_validator import ValidationError, validate_media_for_posting

    if args.debug:
        print(f"添付メディア: {len(media_files)}件")
        for i, media_path in enumerate(media_files, 1):
            print(f"  {i}. {media_path}")

    # 画像リサイズの前処理
    if args.debug:
        print("画像リサイズ処理を実行中...")
    resizer = create_image_resizer(debug=args.debug)

    resized_media_files = []
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

    # メディアファイルの事前検証
    try:
        # 対象SNSのリストを作成
        if args.sns:
            target_sns_list = [sns.strip() for sns in args.sns.split(',')]
        else:
            # ドライラン時は全SNSをチェック
            target_sns_types = ['x', 'bluesky', 'mastodon', 'misskey']

        # メディア検証実行
        validation_results = validate_media_for_posting(resized_media_files, target_sns_types)

        # 検証結果の表示
        has_errors = False
        for sns_type, result in validation_results.items():
            if result.errors:
                has_errors = True
                print(f"❌ {sns_type.upper()}: {', '.join(result.errors)}")
            elif result.warnings and args.debug:
                print(f"⚠️  {sns_type.upper()}: {', '.join(result.warnings)}")
            elif args.debug:
                print(f"✅ {sns_type.upper()}: 投稿可能")

        # エラーがある場合は処理を停止
        if has_errors and not args.dry_run:
            print("\n投稿を中止しました。上記のエラーを解決してから再実行してください。")
            return None

        # 音声ファイルの変換処理（X向け）
        if any('x' in validation_results and
               any('MP4に変換されます' in warning for warning in validation_results['x'].warnings)
               for _ in [None]):  # 条件チェック用のダミーループ

            if not is_ffmpeg_available():
                print("❌ ffmpegが見つかりません。音声変換にはffmpegが必要です。")
                if not args.dry_run:
                    return None
            else:
                if args.debug:
                    print("🔄 音声ファイルをMP4に変換しています...")
                converter = create_media_converter()

                # 音声ファイルのみを変換
                for i, media_path in enumerate(resized_media_files):
                    if media_path.lower().endswith('.m4a'):
                        try:
                            if not args.dry_run:
                                converted_path = converter.convert_m4a_to_mp4(media_path)
                                resized_media_files[i] = converted_path
                                if args.debug:
                                    print(f"✅ 変換完了: {media_path} → {converted_path}")
                            else:
                                if args.debug:
                                    print(f"[ドライラン] 変換予定: {media_path}")
                        except ConversionError as e:
                            print(f"❌ 変換失敗: {media_path} - {e}")
                            if not args.dry_run:
                                return None

    except ValidationError as e:
        print(f"❌ メディア検証エラー: {e}")
        return None
    except Exception as e:
        print(f"❌ メディア処理中にエラーが発生しました: {e}")
        if args.debug:
            import traceback
            traceback.print_exc()
        return None

    return resized_media_files
