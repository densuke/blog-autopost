#!/usr/bin/env python
# -*- coding: utf-8 -*-

from ..config_manager import ConfigManager
from .. import plugin_loader
from ..media_validator import MediaValidator
from ..image_resizer import ImageResizer
from ..text_optimizer import TextOptimizer

class PostingService:
    def __init__(self, config_manager: ConfigManager, 
                 media_validator: MediaValidator, image_resizer: ImageResizer, 
                 text_optimizer: TextOptimizer):
        self.config_manager = config_manager
        self.media_validator = media_validator
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

        # 2. メディアの検証
        try:
            self.media_validator.validate_media_for_posting(media_files, list(plugins.keys()))
        except Exception as e:
            return {'error': f'Media validation failed: {e}'}

        # 3. 投稿処理
        for name, plugin in plugins.items():
            try:
                # 4. テキストの最適化
                # Note: URLをテキストに含めるかどうかは要件による。ここでは含める。
                text_to_optimize = f"{original_text} {url}".strip()
                optimized_text = self.text_optimizer.optimize_text(text_to_optimize, url, plugin.sns_type)

                # 5. 投稿実行
                # Note: メディアのリサイズはここで呼び出す想定だが、
                # ImageResizerのインターフェースに合わせて調整が必要。
                # ここでは簡略化のため、リサイズ済みファイルを渡すことを想定。
                plugin.post(optimized_text, media_files, article_data=None, debug=debug)
                results[name] = {'success': True, 'message': 'Posted successfully.'}

            except Exception as e:
                results[name] = {'success': False, 'message': str(e)}
        
        return results
