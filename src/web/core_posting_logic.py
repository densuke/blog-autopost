"""
既存のmain.pyの投稿処理をラップして、Web APIから呼び出せるようにします。
"""
from typing import List, Dict, Optional, Any
import re
from ..config_manager import ConfigManager
from ..plugin_loader import load_plugins
from ..text_optimizer import TextOptimizer
from ..main import extract_image_from_url


class CorePostingLogic:
    """
    既存のCLI投稿ロジックをラップして、プログラマティックに呼び出せるようにするクラス。
    """
    
    def __init__(self, config_manager: ConfigManager):
        self.config_manager = config_manager
        
    def post_to_sns(
        self, 
        content: str, 
        media_files: Optional[List[str]] = None, 
        target_sns: Optional[List[str]] = None,
        optimize: bool = False,
        debug: bool = False
    ) -> Dict[str, Any]:
        """
        指定されたSNSに投稿を行います。
        
        Args:
            content: 投稿内容
            media_files: 添付メディアファイルのパスリスト
            target_sns: 対象SNSのリスト（Noneの場合は全SNS）
            optimize: テキスト最適化を行うか
            debug: デバッグモード
            
        Returns:
            Dict[str, Any]: 投稿結果 {'success': bool, 'results': Dict[str, str], 'errors': Dict[str, str]}
        """
        results = {}
        errors = {}
        
        try:
            # プラグインをロード
            all_plugins = load_plugins(self.config_manager, dry_run=False)
            
            # 対象SNSでフィルタリング
            if target_sns:
                plugins = {}
                for plugin_name, plugin_instance in all_plugins.items():
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    # nameまたはtypeでマッチング
                    if plugin_name in target_sns or sns_type in target_sns:
                        plugins[plugin_name] = plugin_instance
            else:
                plugins = all_plugins
            
            if not plugins:
                return {
                    'success': False,
                    'results': {},
                    'errors': {'general': 'No valid SNS plugins found for the specified targets'}
                }
            
            # テキスト最適化の設定
            text_optimizer = None
            if optimize:
                text_optimizer = TextOptimizer(self.config_manager.config)
            
            # URLを抽出
            url_pattern = r'https?://[^\s]+'
            urls = re.findall(url_pattern, content)
            
            # 文字数制限の取得
            character_limits = self.config_manager.config.get('character_limits', {
                'x': 280, 
                'bluesky': 300, 
                'mastodon': 500, 
                'misskey': 3000
            })
            
            # 各SNSに投稿
            for plugin_name, plugin_instance in plugins.items():
                try:
                    sns_type = getattr(plugin_instance, 'sns_type', None) or plugin_name.split('-')[0]
                    
                    # 最適化が有効な場合はSNS別に最適化されたテキストを使用
                    optimized_text = content
                    if optimize and text_optimizer and urls:
                        url = urls[-1]
                        title_part = content.replace(url, '').strip()
                        optimized_text = text_optimizer.optimize_text(
                            title_part, url, sns_type, force_optimize=True
                        )
                        if debug:
                            print(f"  最適化後 ({plugin_name}): {optimized_text} ({len(optimized_text)}文字)")
                    
                    # 文字数制限チェック（警告のみ）
                    limit = character_limits.get(sns_type, 500)
                    if len(optimized_text) > limit:
                        print(f"⚠️  警告: {plugin_name} の文字数制限 ({limit}文字) を超えています")
                    
                    # リンクカード対応プラグインのためのarticle_data作成
                    article_data = None
                    if urls and hasattr(plugin_instance, 'supports_rich_content') and plugin_instance.supports_rich_content():
                        url = urls[-1]
                        title_part = content.replace(url, '').strip()
                        image_url = extract_image_from_url(url, debug=debug)
                        article_data = {
                            'title': title_part if title_part else 'ブログ記事',
                            'link': url,
                            'description': title_part,
                            'image': image_url if image_url else None
                        }
                    
                    # 投稿実行
                    if debug:
                        print(f"- {plugin_name}: 投稿中...")
                    
                    plugin_instance.post(
                        optimized_text, 
                        media_files if media_files else None, 
                        article_data=article_data, 
                        debug=debug
                    )
                    
                    results[plugin_name] = 'success'
                    
                    if debug:
                        print(f"- {plugin_name}: 投稿完了")
                        
                except Exception as e:
                    error_msg = str(e)
                    errors[plugin_name] = error_msg
                    if debug:
                        print(f"- {plugin_name}: 投稿失敗 - {error_msg}")
            
            # 全体の成功判定
            success = len(results) > 0
            
            return {
                'success': success,
                'results': results,
                'errors': errors
            }
            
        except Exception as e:
            return {
                'success': False,
                'results': {},
                'errors': {'general': str(e)}
            }
