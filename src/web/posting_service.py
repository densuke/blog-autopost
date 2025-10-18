#!/usr/bin/env python
# -*- coding: utf-8 -*-

import asyncio
import os
import shutil

from .. import plugin_loader
from ..config_manager import ConfigManager
from ..image_resizer import ImageResizer
from ..media_validator import validate_media_for_posting
from ..text_optimizer import TextOptimizer


class PostingService:
    def __init__(self, config_manager: ConfigManager,
                 image_resizer: ImageResizer,
                 text_optimizer: TextOptimizer):
        self.config_manager = config_manager
        self.image_resizer = image_resizer
        self.text_optimizer = text_optimizer

    def post_now(self, post_data: dict, debug: bool = False) -> dict:
        """SNS投稿を並列実行する（非同期対応版）"""
        results = {}
        sns_targets = post_data.get('sns_targets', [])
        media_files = post_data.get('media_files', [])
        original_text = post_data.get('text', '')
        url = post_data.get('url', '')

        # 1. プラグインの読み込み
        plugins = plugin_loader.load_plugins(self.config_manager, sns_names=sns_targets)
        if not plugins:
            return {'error': 'No valid SNS targets found or loaded.'}

        # 2. メディアの検証とリサイズ
        resized_media_cache: dict[str, list[str]] = {}
        unique_sns_types = set(plugin.sns_type for plugin in plugins.values())

        for sns_type in unique_sns_types:
            resized_media_cache[sns_type] = []
            for media_file_path in media_files:
                if media_file_path.lower().endswith(('.jpg', '.jpeg', '.png', '.gif', '.webp')):
                    resized_path = self.image_resizer.resize_image_file(media_file_path, sns_type)
                    resized_media_cache[sns_type].append(resized_path)
                else:
                    resized_media_cache[sns_type].append(media_file_path)

        # 3. 各SNS投稿を並列実行
        async def post_to_sns(name: str, plugin) -> tuple[str, dict]:
            """個別SNSへの投稿（非同期）"""
            try:
                current_plugin_media_files = resized_media_cache.get(plugin.sns_type, media_files)

                # メディア検証
                validation_results = validate_media_for_posting(current_plugin_media_files, [plugin.sns_type])
                plugin_validation_result = validation_results.get(plugin.sns_type)

                if plugin_validation_result and not plugin_validation_result.is_valid:
                    all_errors = ", ".join(plugin_validation_result.errors)
                    return name, {'success': False, 'message': f'Media validation failed for {plugin.sns_type.upper()}: {all_errors}'}

                # テキスト最適化
                article_data = None
                text_to_optimize = original_text

                if url:
                    if hasattr(plugin, 'supports_rich_content') and plugin.supports_rich_content():
                        article_data = {'title': original_text, 'link': url, 'description': original_text}
                    else:
                        text_to_optimize = f"{original_text} {url}".strip()

                optimized_text = self.text_optimizer.optimize_text(text_to_optimize, url, plugin.sns_type)

                # 投稿実行（スレッドプール経由で非ブロッキング化）
                loop = asyncio.get_event_loop()
                await loop.run_in_executor(
                    None,
                    plugin.post,
                    optimized_text,
                    current_plugin_media_files,
                    article_data,
                    debug
                )

                return name, {'success': True, 'message': 'Posted successfully.'}

            except Exception as e:
                return name, {'success': False, 'message': str(e)}

        # 並列実行（asyncio.gather）
        try:
            # イベントループを取得またはjoy作成
            try:
                loop = asyncio.get_running_loop()
            except RuntimeError:
                # ループが実行中でない場合は新規作成
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                run_in_new_loop = True
            else:
                run_in_new_loop = False

            # 並列タスクを作成
            tasks = [post_to_sns(name, plugin) for name, plugin in plugins.items()]

            # asyncio.gather で並列実行
            if run_in_new_loop:
                task_results = loop.run_until_complete(asyncio.gather(*tasks))
                loop.close()
            else:
                # 既存ループ内なら run_until_complete は使用できない
                # この場合は create_task で登録するか、同期実行に戻す
                task_results = loop.run_until_complete(asyncio.gather(*tasks))

            # 結果をまとめる
            for name, result in task_results:
                results[name] = result

        except RuntimeError as e:
            # asyncio 実行エラーの場合は従来の同期実行にフォールバック
            for name, plugin in plugins.items():
                try:
                    current_plugin_media_files = resized_media_cache.get(plugin.sns_type, media_files)
                    validation_results = validate_media_for_posting(current_plugin_media_files, [plugin.sns_type])
                    plugin_validation_result = validation_results.get(plugin.sns_type)

                    if plugin_validation_result and not plugin_validation_result.is_valid:
                        all_errors = ", ".join(plugin_validation_result.errors)
                        results[name] = {'success': False, 'message': f'Media validation failed: {all_errors}'}
                        continue

                    article_data = None
                    text_to_optimize = original_text

                    if url:
                        if hasattr(plugin, 'supports_rich_content') and plugin.supports_rich_content():
                            article_data = {'title': original_text, 'link': url, 'description': original_text}
                        else:
                            text_to_optimize = f"{original_text} {url}".strip()

                    optimized_text = self.text_optimizer.optimize_text(text_to_optimize, url, plugin.sns_type)
                    plugin.post(optimized_text, current_plugin_media_files, article_data=article_data, debug=debug)
                    results[name] = {'success': True, 'message': 'Posted successfully.'}

                except Exception as e:
                    results[name] = {'success': False, 'message': str(e)}

        return results

    def post_now_and_cleanup(self, post_data: dict, debug: bool = False) -> dict:
        """
        post_nowを呼び出し、その後メディアファイルを保存したディレクトリをクリーンアップします。
        APSchedulerから呼び出されることを想定しています。
        """
        results = self.post_now(post_data, debug)

        job_media_dir = post_data.get('job_media_dir')
        if job_media_dir and os.path.exists(job_media_dir):
            try:
                shutil.rmtree(job_media_dir)
                if debug:
                    print(f"Scheduled media directory cleaned up: {job_media_dir}")
            except Exception as e:
                if debug:
                    print(f"Error cleaning up scheduled media directory {job_media_dir}: {e}")
        return results
