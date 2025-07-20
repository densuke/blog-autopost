from typing import Dict, Any
from .url_shortener import URLShortener


class TextOptimizer:
    """
    SNS投稿用テキスト最適化クラス
    
    文字数制限に応じてURL短縮やタイトルトリミングを行います。
    """
    
    # SNS別デフォルト文字数制限
    DEFAULT_CHARACTER_LIMITS = {
        'x': 280,
        'bluesky': 300,
        'mastodon': 500,
        'misskey': 3000
    }
    
    def __init__(self, config: Dict[str, Any]):
        """
        TextOptimizerを初期化します
        
        Args:
            config (Dict[str, Any]): 設定辞書
        """
        self.config = config
        
        # URL短縮設定
        url_shortening_config = config.get('url_shortening', {})
        self.url_shortening_enabled = url_shortening_config.get('enabled', True)
        
        # 文字数制限設定
        custom_limits = config.get('character_limits', {})
        self.character_limits = self.DEFAULT_CHARACTER_LIMITS.copy()
        self.character_limits.update(custom_limits)
        
        # URL短縮インスタンス
        if self.url_shortening_enabled:
            self.url_shortener = URLShortener()
        else:
            self.url_shortener = None
    
    def get_character_limit(self, sns_type: str) -> int:
        """
        指定されたSNSの文字数制限を取得します
        
        Args:
            sns_type (str): SNS種別
            
        Returns:
            int: 文字数制限
        """
        return self.character_limits.get(sns_type, 500)  # デフォルト500文字
    
    def optimize_text(self, title: str, link: str, sns_type: str) -> str:
        """
        SNSの文字数制限に合わせてテキストを最適化します
        
        Args:
            title (str): 記事タイトル
            link (str): 記事URL
            sns_type (str): SNS種別
            
        Returns:
            str: 最適化されたテキスト
        """
        character_limit = self.get_character_limit(sns_type)
        
        # 基本形式: "{title} {link}"
        original_text = f"{title} {link}"
        
        # 文字数制限内の場合はそのまま返す
        if len(original_text) <= character_limit:
            return original_text
        
        # URL短縮を試行
        if self.url_shortening_enabled and self.url_shortener:
            shortened_link = self.url_shortener.shorten(link)
            shortened_text = f"{title} {shortened_link}"
            
            # 短縮後に制限内に収まる場合
            if len(shortened_text) <= character_limit:
                return shortened_text
            
            # まだ長い場合はタイトルをトリミング
            return self._trim_title_with_link(title, shortened_link, character_limit)
        else:
            # URL短縮無効の場合、タイトルをトリミング
            return self._trim_title_with_link(title, link, character_limit)
    
    def _trim_title_with_link(self, title: str, link: str, character_limit: int) -> str:
        """
        タイトルをトリミングしてリンクと組み合わせます
        
        Args:
            title (str): 記事タイトル
            link (str): リンクURL（短縮済み可能性あり）
            character_limit (int): 文字数制限
            
        Returns:
            str: トリミングされたテキスト
        """
        # "... " と link の分を確保
        ellipsis = "..."
        space = " "
        reserved_length = len(ellipsis) + len(space) + len(link)
        
        # タイトルに使える文字数を計算
        available_title_length = character_limit - reserved_length
        
        # タイトルが使える文字数より短い場合はそのまま
        if len(title) <= available_title_length:
            return f"{title} {link}"
        
        # タイトルをトリミング
        if available_title_length > 0:
            trimmed_title = title[:available_title_length]
            return f"{trimmed_title}{ellipsis} {link}"
        else:
            # 極端に短い制限の場合、リンクのみ
            return link