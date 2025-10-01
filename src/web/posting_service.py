#!/usr/bin/env python
# -*- coding: utf-8 -*-

import shutil # shutilモジュールをインポート
import os # osモジュールをインポート

from ..config_manager import ConfigManager
from .. import plugin_loader
from ..media_validator import validate_media_for_posting
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer

class PostingService:
    def __init__(self, config_manager: ConfigManager, 
                 image_resizer: ImageResizer, 
                 text_optimizer: TextOptimizer):
        self.config_manager = config_manager
        self.image_resizer = image_resizer
        self.text_optimizer = text_optimizer

    def post_now(self, post_data: dict, debug: bool = False) -> dict:
        results = {}
        sns_targets = post_data.get('sns_targets', [])
        media_files = post_data.get('media_files', [])
        original_text = post_data.get('text', '')
        url = post_data.get('url', '')

        # 1. プラグインの読み込み
        plugins = plugin_loader.load_plugins(self.config_manager, sns_names=sns_targets)
        if not plugins:
            return {'error': 'No valid SNS targets found or loaded.'}

        # 2. メディアの検証とリサイズ、投稿
        for name, plugin in plugins.items():
            try:
                # 2.1. プラグイン固有のリサイズ処理
                current_plugin_media_files = []
                for media_file_path in media_files:
                    if media_file_path.lower().endswith(('.jpg', '.jpeg', '.png', '.gif', '.webp')):
                        # 画像ファイルの場合のみリサイズ
                        resized_path = self.image_resizer.resize_image_file(media_file_path, plugin.sns_type)
                        current_plugin_media_files.append(resized_path)
                    else:
                        current_plugin_media_files.append(media_file_path)

                # 2.2. リサイズ後のメディアで検証
                validation_results = validate_media_for_posting(current_plugin_media_files, [plugin.sns_type])
                plugin_validation_result = validation_results.get(plugin.sns_type)

                if plugin_validation_result and not plugin_validation_result.is_valid:
                    all_errors = ", ".join(plugin_validation_result.errors)
                    results[name] = {'success': False, 'message': f'Media validation failed for {plugin.sns_type.upper()}: {all_errors}'}
                    continue # このSNSへの投稿はスキップ

                # 2.3. テキストの最適化
                article_data = None
                text_to_optimize = original_text

                if url:
                    if hasattr(plugin, 'supports_rich_content') and plugin.supports_rich_content():
                        # リッチコンテンツ対応プラグインの場合、URLはテキストに含めずarticle_dataに渡す
                        article_data = {'title': original_text, 'link': url, 'description': original_text}
                        text_to_optimize = original_text  # URLを含めない
                    else:
                        # リッチコンテンツ非対応の場合、URLをテキストに含める
                        text_to_optimize = f"{original_text} {url}".strip()
                optimized_text = self.text_optimizer.optimize_text(text_to_optimize, url, plugin.sns_type)

                # 2.4. 投稿実行
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
